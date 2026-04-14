use std::collections::HashMap;
use pq_ast::{
    Program,
    expr::{Expr, ExprNode},
    step::{Step, StepKind},
};
use pq_diagnostics::Diagnostic;
use pq_grammar::operators::{Operator, UnaryOp};
use pq_grammar::{lookup_qualified, Type, FunctionType, Param, unify, nullable};
use pq_pipeline::Table;
use pq_types::{ColumnType, coerce_types, coercion::CoercionResult};
use crate::error::TypeError;

pub type CheckResult = Result<(), Vec<Diagnostic>>;

/// Type-checker and type-annotator for a parsed M program.
///
/// After `check` completes on a valid program:
/// - Every `ExprNode.inferred_type` in the program is `Some(...)`.
/// - Every `Step.output_type` is `Some(Vec<(col_name, ColumnType)>)`.
/// - `step_schemas` maps every step name to the column schema it produces,
///   making the propagated types available to downstream passes.
pub struct TypeChecker<'a> {
    table:        &'a Table,
    diagnostics:  Vec<Diagnostic>,
    /// Maps each step name to the column schema it produces.
    /// Populated step-by-step so later steps can resolve column types
    /// from earlier ones.
    pub step_schemas: HashMap<String, Vec<(String, ColumnType)>>,
    /// The type bound to the implicit `_` lambda parameter in the
    /// current expression context.  Set before recursing into a lambda body
    /// and restored afterwards so nesting works correctly.
    lambda_param: Option<ColumnType>,
}

impl<'a> TypeChecker<'a> {
    pub fn new(table: &'a Table) -> Self {
        TypeChecker {
            table,
            diagnostics: vec![],
            step_schemas: HashMap::new(),
            lambda_param: None,
        }
    }

    // ── column lookup ─────────────────────────────────────────────────────

    fn lookup_col(schema: Option<&[(String, ColumnType)]>, name: &str) -> Option<ColumnType> {
        schema?.iter().find(|(n, _)| n == name).map(|(_, t)| t.clone())
    }

    // ── expression annotation ─────────────────────────────────────────────

    /// Annotate every node in `node`'s subtree with its inferred type.
    ///
    /// **Phase 1** – recurse into child nodes mutably (bottom-up), populating
    /// their `inferred_type` fields.
    /// **Phase 2** – compute this node's type from the already-annotated
    /// children (read-only), emitting diagnostics on errors.
    /// **Phase 3** – store the result in `node.inferred_type` and return it.
    fn infer_expr_mut(
        &mut self,
        node:   &mut ExprNode,
        schema: Option<&[(String, ColumnType)]>,
    ) -> Option<ColumnType> {
        // ── Phase 1: recurse into children ──────────────────────────────
        // Scoped block ensures the mutable borrow of node.expr ends before
        // Phase 2 needs to read node as a whole.
        {
            match &mut node.expr {
                Expr::BinaryOp { left, right, .. } => {
                    self.infer_expr_mut(left,  schema);
                    self.infer_expr_mut(right, schema);
                }
                Expr::UnaryOp { operand, .. } => {
                    self.infer_expr_mut(operand, schema);
                }
                Expr::Lambda { body, .. } => {
                    self.infer_expr_mut(body, schema);
                }
                Expr::FunctionCall { name, args } => {
                    let fn_name = name.clone();

                    // Pass 1: infer all non-lambda args so we have their types
                    // available when determining what `_` should be for lambdas.
                    for arg in args.iter_mut() {
                        if !matches!(arg.expr, Expr::Lambda { .. }) {
                            self.infer_expr_mut(arg, schema);
                        }
                    }

                    // Collect the types from pass 1 (None for lambda slots).
                    let pass1_types: Vec<Option<ColumnType>> =
                        args.iter().map(|a| a.inferred_type.clone()).collect();

                    // Pass 2: for each lambda arg, resolve the `_` binding from
                    // the function signature + known arg types, then infer.
                    for (i, arg) in args.iter_mut().enumerate() {
                        if matches!(arg.expr, Expr::Lambda { .. }) {
                            let param_ty =
                                determine_lambda_param_type(&fn_name, i, &pass1_types);
                            let old = self.lambda_param.take();
                            self.lambda_param = param_ty;
                            self.infer_expr_mut(arg, schema);
                            self.lambda_param = old;
                        }
                    }
                }
                Expr::List(items) => {
                    for item in items.iter_mut() {
                        self.infer_expr_mut(item, schema);
                    }
                }
                Expr::Record(fields) => {
                    for (_, val) in fields.iter_mut() {
                        self.infer_expr_mut(val, schema);
                    }
                }
                Expr::FieldAccess { record, .. } => {
                    self.infer_expr_mut(record, schema);
                }
                _ => {} // leaf nodes have no children
            }
        } // ← mutable borrow of node.expr ends here

        // ── Phase 2: compute type from already-annotated children ────────
        let ty = self.compute_node_type(node, schema);

        // ── Phase 3: store and return ─────────────────────────────────────
        node.inferred_type = ty.clone();
        ty
    }

    /// Compute `node`'s type using its already-annotated children.
    /// May emit diagnostics.
    fn compute_node_type(
        &mut self,
        node:   &ExprNode,
        schema: Option<&[(String, ColumnType)]>,
    ) -> Option<ColumnType> {
        match &node.expr {
            // ── literals ─────────────────────────────────────────────────
            Expr::IntLit(_)    => Some(ColumnType::Integer),
            Expr::FloatLit(_)  => Some(ColumnType::Float),
            Expr::BoolLit(_)   => Some(ColumnType::Boolean),
            Expr::StringLit(_) => Some(ColumnType::Text),
            Expr::NullLit      => Some(ColumnType::Null),

            // ── column references ────────────────────────────────────────
            Expr::ColumnAccess(name) => {
                Self::lookup_col(schema, name)
                    .or_else(|| self.table.get_column(name).map(|c| c.col_type.clone()))
            }
            // Field access `row[col]` has the same type as the named column.
            Expr::FieldAccess { field, .. } => {
                Self::lookup_col(schema, field)
                    .or_else(|| self.table.get_column(field).map(|c| c.col_type.clone()))
            }
            Expr::Identifier(name) => {
                if name == "_" {
                    // Return the type of the implicit lambda parameter, if bound.
                    self.lambda_param.clone()
                } else if name.contains('.') {
                    None // dotted name — not a column type
                } else {
                    Self::lookup_col(schema, name)
                        .or_else(|| self.table.get_column(name).map(|c| c.col_type.clone()))
                }
            }

            // ── lambda ───────────────────────────────────────────────────
            // `each <body>` → Lambda { param: "_", body }.
            // Type = Function(<body_return_type>).
            // Returns None when body inference failed (errors already logged).
            Expr::Lambda { body, .. } => {
                let ret = body.inferred_type.clone()?;
                Some(ColumnType::Function(Box::new(ret)))
            }

            // ── unary ops ────────────────────────────────────────────────
            Expr::UnaryOp { op, operand } => {
                let t = operand.inferred_type.clone()?;
                match op {
                    UnaryOp::Not => {
                        if t != ColumnType::Boolean {
                            self.diagnostics.push(
                                Diagnostic::error(
                                    "E406",
                                    format!("'not' requires Boolean operand, got '{}'", t),
                                )
                                .with_label(node.span.clone(), "operand must be Boolean")
                            );
                            return None;
                        }
                        Some(ColumnType::Boolean)
                    }
                    UnaryOp::Neg => {
                        if !t.is_numeric() {
                            self.diagnostics.push(
                                Diagnostic::error(
                                    "E407",
                                    format!("unary '-' requires numeric operand, got '{}'", t),
                                )
                                .with_label(node.span.clone(), "operand must be numeric")
                            );
                            return None;
                        }
                        Some(t)
                    }
                }
            }

            // ── binary ops ───────────────────────────────────────────────
            Expr::BinaryOp { left, op, right } => {
                let lt = left.inferred_type.clone();
                let rt = right.inferred_type.clone();

                if op.is_logical() {
                    let lt = lt?;
                    let rt = rt?;
                    if lt != ColumnType::Boolean {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E408",
                                format!("'{}' requires Boolean operands, left is '{}'", op, lt),
                            )
                            .with_label(left.span.clone(), "must be Boolean")
                        );
                        return None;
                    }
                    if rt != ColumnType::Boolean {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E408",
                                format!("'{}' requires Boolean operands, right is '{}'", op, rt),
                            )
                            .with_label(right.span.clone(), "must be Boolean")
                        );
                        return None;
                    }
                    return Some(ColumnType::Boolean);
                }

                let lt = lt?;
                let rt = rt?;
                self.check_binary_op_types(&lt, &rt, op, &node.span)
            }

            // ── function calls ───────────────────────────────────────────
            Expr::FunctionCall { name, args } => {
                let arg_types: Vec<Option<ColumnType>> =
                    args.iter().map(|a| a.inferred_type.clone()).collect();

                let result = infer_call_return(name, &arg_types);

                // Emit a diagnostic only when every argument type is known
                // but the call still fails — that is a genuine type mismatch.
                // When any arg is None (unknown), we simply propagate None without
                // a diagnostic because errors were already reported upstream.
                if result.is_none()
                    && arg_types.iter().all(|t| t.is_some())
                    && lookup_qualified(name).is_some()
                {
                    let arg_list = arg_types
                        .iter()
                        .map(|t| t.as_ref().map_or("?".to_string(), |t| t.to_string()))
                        .collect::<Vec<_>>()
                        .join(", ");
                    self.diagnostics.push(
                        Diagnostic::error(
                            "E409",
                            format!(
                                "type mismatch in call to '{}': \
                                 argument types ({}) are invalid",
                                name, arg_list
                            ),
                        )
                        .with_label(node.span.clone(), "argument type mismatch"),
                    );
                }

                result
            }

            // ── collections ──────────────────────────────────────────────
            Expr::List(items) => {
                if items.is_empty() {
                    return Some(ColumnType::List(Box::new(ColumnType::Null)));
                }

                // Collect already-annotated element types (from phase 1).
                let elem_types: Vec<Option<ColumnType>> =
                    items.iter().map(|i| i.inferred_type.clone()).collect();

                // If any element type is still unknown, propagate None.
                let types: Vec<ColumnType> =
                    match elem_types.into_iter().collect::<Option<Vec<_>>>() {
                        Some(v) => v,
                        None    => return None,
                    };

                // Unify all element types into a single type T.
                let mut unified: Option<ColumnType> = Some(types[0].clone());
                for t in &types[1..] {
                    unified = match unified {
                        Some(u) => unify_column_types(&u, t),
                        None    => None,
                    };
                }

                match unified {
                    Some(t) => Some(ColumnType::List(Box::new(t))),
                    None => {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E410",
                                "list elements have incompatible types",
                            )
                            .with_label(
                                node.span.clone(),
                                "all elements must have a compatible type",
                            ),
                        );
                        None
                    }
                }
            }

            Expr::Record(_) => None,
        }
    }

    fn check_binary_op_types(
        &mut self,
        lt:   &ColumnType,
        rt:   &ColumnType,
        op:   &Operator,
        span: &pq_diagnostics::Span,
    ) -> Option<ColumnType> {
        let coercion = coerce_types(lt, rt);

        match coercion {
            CoercionResult::Incompatible => {
                self.diagnostics.push(
                    TypeError::TypeMismatch {
                        left:  lt.clone(),
                        right: rt.clone(),
                        span:  span.clone(),
                    }.to_diagnostic()
                );
                None
            }

            CoercionResult::Same(ref t) | CoercionResult::Coerced(ref t) => {
                let result_type = t.clone();

                if op.is_arithmetic() {
                    if !result_type.is_numeric() {
                        self.diagnostics.push(
                            TypeError::ArithmeticOnNonNumeric {
                                col_type: result_type,
                                span:     span.clone(),
                            }.to_diagnostic()
                        );
                        return None;
                    }
                    return Some(match (lt, rt) {
                        (ColumnType::Float, _) | (_, ColumnType::Float) => ColumnType::Float,
                        _ => ColumnType::Integer,
                    });
                }

                if op.is_concatenation() {
                    // Concat always produces Text; non-text operands are auto-coerced.
                    return Some(ColumnType::Text);
                }

                if op.is_comparison() {
                    if !result_type.is_comparable() {
                        self.diagnostics.push(
                            TypeError::ComparisonOnIncomparable {
                                col_type: result_type,
                                span:     span.clone(),
                            }.to_diagnostic()
                        );
                        return None;
                    }
                    if matches!(result_type, ColumnType::Text | ColumnType::Boolean) {
                        if !matches!(op, Operator::Eq | Operator::NotEq) {
                            self.diagnostics.push(
                                Diagnostic::error(
                                    "E404",
                                    format!(
                                        "operator '{}' cannot be used with type '{}'",
                                        op, result_type
                                    ),
                                )
                                .with_label(
                                    span.clone(),
                                    format!("only '=' and '<>' are valid for {}", result_type),
                                )
                            );
                            return None;
                        }
                    }
                    return Some(ColumnType::Boolean);
                }

                Some(result_type)
            }
        }
    }

    // ── per-step expression annotation ───────────────────────────────────

    fn annotate_step_exprs(
        &mut self,
        kind:   &mut StepKind,
        schema: Option<&[(String, ColumnType)]>,
    ) {
        match kind {
            StepKind::Filter { condition, .. } => {
                if let Some(t) = self.infer_expr_mut(condition, schema) {
                    let body_ty = match t {
                        ColumnType::Function(inner) => *inner,
                        other => other,
                    };
                    if body_ty != ColumnType::Boolean {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E405",
                                format!("filter condition must be Boolean, got '{}'", body_ty),
                            )
                            .with_label(
                                condition.span.clone(),
                                "this must produce a Boolean value",
                            )
                            .with_suggestion("use a comparison operator like >, <, =, <>")
                        );
                    }
                }
            }
            StepKind::AddColumn { expression, .. } => {
                self.infer_expr_mut(expression, schema);
            }
            StepKind::TransformColumns { transforms, .. } => {
                for (col_name, expr, _col_type) in transforms.iter_mut() {
                    // Bind `_` to the column's current type so `each _ * 2` etc.
                    // can be validated against the actual column type.
                    let col_type = schema.and_then(|s| Self::lookup_col(Some(s), col_name));
                    let old = self.lambda_param.take();
                    self.lambda_param = col_type;
                    self.infer_expr_mut(expr, schema);
                    self.lambda_param = old;
                }
            }
            StepKind::Group { aggregates, .. } => {
                for agg in aggregates.iter_mut() {
                    self.infer_expr_mut(&mut agg.expression, schema);
                }
            }
            // ListTransform: infer the source list first, extract its element
            // type T, then bind `_` to T before inferring the per-element lambda.
            StepKind::ListTransform { list_expr, transform } => {
                self.infer_expr_mut(list_expr, schema);

                // Extract the element type from the (now-annotated) list expression.
                let elem_type = match &list_expr.inferred_type {
                    Some(ColumnType::List(inner)) => Some((**inner).clone()),
                    _ => None,
                };

                let old = self.lambda_param.take();
                self.lambda_param = elem_type;
                self.infer_expr_mut(transform, None);
                self.lambda_param = old;
            }
            // ListGenerate: no deep type-inference needed; just prevent panics.
            StepKind::ListGenerate { initial, condition, next, selector } => {
                self.infer_expr_mut(initial,   schema);
                self.infer_expr_mut(condition, None);
                self.infer_expr_mut(next,      None);
                if let Some(sel) = selector {
                    self.infer_expr_mut(sel, None);
                }
            }

            // New row operations with expression args
            StepKind::TransformRows { transform, .. } => {
                self.infer_expr_mut(transform, schema);
            }
            StepKind::MatchesAllRows { condition, .. }
            | StepKind::MatchesAnyRows { condition, .. } => {
                if let Some(t) = self.infer_expr_mut(condition, schema) {
                    let body_ty = match t {
                        ColumnType::Function(inner) => *inner,
                        other => other,
                    };
                    if body_ty != ColumnType::Boolean {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "E405",
                                format!("predicate must be Boolean, got '{}'", body_ty),
                            )
                            .with_label(condition.span.clone(), "this must produce a Boolean value")
                        );
                    }
                }
            }
            StepKind::FirstN { count, .. }
            | StepKind::LastN { count, .. }
            | StepKind::Skip { count, .. }
            | StepKind::RemoveFirstN { count, .. }
            | StepKind::RemoveLastN { count, .. }
            | StepKind::Repeat { count, .. } => {
                self.infer_expr_mut(count, schema);
            }
            StepKind::Range { offset, count, .. }
            | StepKind::RemoveRows { offset, count, .. } => {
                self.infer_expr_mut(offset, schema);
                self.infer_expr_mut(count, schema);
            }
            StepKind::AlternateRows { offset, skip, take, .. } => {
                self.infer_expr_mut(offset, schema);
                self.infer_expr_mut(skip, schema);
                self.infer_expr_mut(take, schema);
            }

            // New operations with expression args
            StepKind::TransformColumnNames { transform, .. } => {
                self.infer_expr_mut(transform, schema);
            }
            StepKind::CombineColumns { combiner, .. } => {
                self.infer_expr_mut(combiner, schema);
            }
            StepKind::SplitColumn { splitter, .. } => {
                self.infer_expr_mut(splitter, schema);
            }
            StepKind::ReplaceValue { old_value, new_value, replacer, .. } => {
                self.infer_expr_mut(old_value, schema);
                self.infer_expr_mut(new_value, schema);
                self.infer_expr_mut(replacer, schema);
            }
            StepKind::ReplaceErrorValues { replacements, .. } => {
                for (_, expr, _) in replacements.iter_mut() {
                    self.infer_expr_mut(expr, schema);
                }
            }
            StepKind::TableMaxN { count, .. }
            | StepKind::TableMinN { count, .. } => {
                self.infer_expr_mut(count, schema);
            }

            // Steps with no embedded expressions.
            _ => {}
        }
    }

    // ── per-step output-schema computation ───────────────────────────────

    /// Derive the output column schema for a step.
    /// Expression nodes must already be annotated before calling this.
    fn compute_output_schema(
        &self,
        kind:         &StepKind,
        input_schema: Option<Vec<(String, ColumnType)>>,
    ) -> Option<Vec<(String, ColumnType)>> {
        match kind {
            StepKind::Source { .. } => Some(
                self.table.columns.iter()
                    .map(|c| (c.name.clone(), c.col_type.clone()))
                    .collect()
            ),

            StepKind::PromoteHeaders { .. } => input_schema,

            StepKind::ChangeTypes { columns, .. } => {
                let mut schema = input_schema?;
                for col in schema.iter_mut() {
                    if let Some((_, new_ty)) = columns.iter().find(|(n, _)| n == &col.0) {
                        col.1 = new_ty.clone();
                    }
                }
                Some(schema)
            }

            StepKind::Filter { .. } => input_schema,

            StepKind::AddColumn { col_name, expression, .. } => {
                let mut schema = input_schema?;
                let new_type = expression.inferred_type.clone()
                    .map(|t| match t {
                        ColumnType::Function(inner) => *inner,
                        other => other,
                    })
                    .unwrap_or(ColumnType::Text);
                schema.push((col_name.clone(), new_type));
                Some(schema)
            }

            StepKind::RemoveColumns { columns, .. } => Some(
                input_schema?
                    .into_iter()
                    .filter(|(n, _)| !columns.contains(n))
                    .collect()
            ),

            StepKind::RenameColumns { renames, .. } => Some(
                input_schema?
                    .into_iter()
                    .map(|(n, t)| {
                        let new_n = renames.iter()
                            .find(|(old, _)| old == &n)
                            .map(|(_, new)| new.clone())
                            .unwrap_or(n);
                        (new_n, t)
                    })
                    .collect()
            ),

            StepKind::Sort { .. } => input_schema,

            StepKind::TransformColumns { transforms, .. } => {
                let mut schema = input_schema?;
                for (col_name, expr, opt_type) in transforms {
                    let new_type = opt_type.clone().or_else(|| {
                        expr.inferred_type.clone()
                            .map(|t| match t {
                                ColumnType::Function(inner) => *inner,
                                other => other,
                            })
                    }).unwrap_or(ColumnType::Text);
                    if let Some(col) = schema.iter_mut().find(|(n, _)| n == col_name) {
                        col.1 = new_type;
                    }
                }
                Some(schema)
            }

            StepKind::Group { by, aggregates, .. } => {
                let in_schema = input_schema.as_ref()?;
                let mut cols: Vec<(String, ColumnType)> = Vec::new();
                for col_name in by {
                    let ty = in_schema.iter()
                        .find(|(n, _)| n == col_name)
                        .map(|(_, t)| t.clone())
                        .unwrap_or(ColumnType::Text);
                    cols.push((col_name.clone(), ty));
                }
                for agg in aggregates {
                    cols.push((agg.name.clone(), agg.col_type.clone()));
                }
                Some(cols)
            }

            StepKind::Passthrough { .. } => input_schema,

            // ── New row operations: schema passthrough ────────────────────
            StepKind::FirstN          { .. }
            | StepKind::LastN         { .. }
            | StepKind::Skip          { .. }
            | StepKind::Range         { .. }
            | StepKind::RemoveFirstN  { .. }
            | StepKind::RemoveLastN   { .. }
            | StepKind::RemoveRows    { .. }
            | StepKind::ReverseRows   { .. }
            | StepKind::Repeat        { .. }
            | StepKind::AlternateRows { .. }
            | StepKind::FindText      { .. }
            | StepKind::FillDown      { .. }
            | StepKind::FillUp        { .. }
            | StepKind::RemoveRowsWithErrors  { .. }
            | StepKind::SelectRowsWithErrors  { .. }
            | StepKind::TransformRows { .. }
            | StepKind::MatchesAllRows { .. }
            | StepKind::MatchesAnyRows { .. }
            | StepKind::DemoteHeaders  { .. } => input_schema,

            // Distinct: schema passthrough
            StepKind::Distinct { .. } => input_schema,

            // AddIndexColumn: input + index column (Integer)
            StepKind::AddIndexColumn { col_name, .. } => {
                let mut schema = input_schema?;
                schema.push((col_name.clone(), ColumnType::Integer));
                Some(schema)
            }

            // DuplicateColumn: input + copy of source column
            StepKind::DuplicateColumn { src_col, new_col, .. } => {
                let schema = input_schema.as_ref()?;
                let src_ty = schema.iter()
                    .find(|(n, _)| n == src_col)
                    .map(|(_, t)| t.clone())
                    .unwrap_or(ColumnType::Text);
                let mut out = input_schema?;
                out.push((new_col.clone(), src_ty));
                Some(out)
            }

            // Transpose: dynamic schema
            StepKind::Transpose { .. } => None,

            // Unpivot: non-unpivoted columns + attr + val
            StepKind::Unpivot { columns, attr_col, val_col, .. } => {
                let schema = input_schema?;
                let mut out: Vec<(String, ColumnType)> = schema.into_iter()
                    .filter(|(n, _)| !columns.contains(n))
                    .collect();
                out.push((attr_col.clone(), ColumnType::Text));
                out.push((val_col.clone(), ColumnType::Text));
                Some(out)
            }

            // UnpivotOtherColumns: keep cols + attr + val
            StepKind::UnpivotOtherColumns { keep_cols, attr_col, val_col, .. } => {
                let schema = input_schema?;
                let mut out: Vec<(String, ColumnType)> = schema.into_iter()
                    .filter(|(n, _)| keep_cols.contains(n))
                    .collect();
                out.push((attr_col.clone(), ColumnType::Text));
                out.push((val_col.clone(), ColumnType::Text));
                Some(out)
            }

            // CombineTables: union of all schemas
            StepKind::CombineTables { .. } => input_schema,

            // PrefixColumns: rename all columns
            StepKind::PrefixColumns { prefix, .. } => {
                input_schema.map(|cols| {
                    cols.into_iter()
                        .map(|(n, t)| (format!("{}.{}", prefix, n), t))
                        .collect()
                })
            }

            // SelectColumns: keep subset in given order
            StepKind::SelectColumns { columns, .. } => {
                let schema = input_schema?;
                Some(columns.iter().filter_map(|c| {
                    schema.iter().find(|(n, _)| n == c).cloned()
                }).collect())
            }

            // ReorderColumns: listed first, then remaining
            StepKind::ReorderColumns { columns, .. } => {
                let schema = input_schema?;
                let mut out: Vec<(String, ColumnType)> = columns.iter()
                    .filter_map(|c| schema.iter().find(|(n, _)| n == c).cloned())
                    .collect();
                for col in &schema {
                    if !out.iter().any(|(n, _)| n == &col.0) {
                        out.push(col.clone());
                    }
                }
                Some(out)
            }

            // TransformColumnNames: names change dynamically
            StepKind::TransformColumnNames { .. } => None,

            // CombineColumns: input minus merged cols + new col
            StepKind::CombineColumns { columns, new_col, combiner, .. } => {
                let schema = input_schema?;
                let mut out: Vec<(String, ColumnType)> = schema.into_iter()
                    .filter(|(n, _)| !columns.contains(n))
                    .collect();
                let item_type = combiner.inferred_type.clone()
                    .map(|t| match t {
                        ColumnType::Function(inner) => *inner,
                        other => other,
                    })
                    .unwrap_or(ColumnType::Text);
                out.push((new_col.clone(), item_type));
                Some(out)
            }

            // SplitColumn: input minus split col (dynamic new cols)
            StepKind::SplitColumn { col_name, .. } => {
                let schema = input_schema?;
                Some(schema.into_iter().filter(|(n, _)| n != col_name).collect())
            }

            // Expand columns: input minus expanded col + new cols (all Text)
            StepKind::ExpandTableColumn { col_name, columns, .. } => {
                let schema = input_schema?;
                let mut out: Vec<(String, ColumnType)> = schema.into_iter()
                    .filter(|(n, _)| n != col_name)
                    .collect();
                for c in columns {
                    out.push((c.clone(), ColumnType::Text));
                }
                Some(out)
            }
            StepKind::ExpandRecordColumn { col_name, fields, .. } => {
                let schema = input_schema?;
                let mut out: Vec<(String, ColumnType)> = schema.into_iter()
                    .filter(|(n, _)| n != col_name)
                    .collect();
                for f in fields {
                    out.push((f.clone(), ColumnType::Text));
                }
                Some(out)
            }

            // Pivot: dynamic schema
            StepKind::Pivot { .. } => None,

            // Information functions: single-value schemas
            StepKind::RowCount { .. }
            | StepKind::ColumnCount { .. } => {
                Some(vec![("Value".to_string(), ColumnType::Integer)])
            }
            StepKind::TableColumnNames { .. } => {
                Some(vec![("Value".to_string(), ColumnType::Text)])
            }
            StepKind::TableIsEmpty { .. }
            | StepKind::TableIsDistinct { .. }
            | StepKind::HasColumns { .. } => {
                Some(vec![("Value".to_string(), ColumnType::Boolean)])
            }
            StepKind::TableSchema { .. } => {
                Some(vec![
                    ("Name".to_string(), ColumnType::Text),
                    ("Kind".to_string(), ColumnType::Text),
                    ("IsNullable".to_string(), ColumnType::Boolean),
                ])
            }

            // Join: merge both schemas
            StepKind::Join { .. } => input_schema,

            // NestedJoin: left schema + nested table column
            StepKind::NestedJoin { new_col, .. } => {
                let mut schema = input_schema?;
                schema.push((new_col.clone(), ColumnType::Text));
                Some(schema)
            }

            // AddRankColumn: input + Integer column
            StepKind::AddRankColumn { col_name, .. } => {
                let mut schema = input_schema?;
                schema.push((col_name.clone(), ColumnType::Integer));
                Some(schema)
            }

            // TableMax/Min: single row, same schema
            StepKind::TableMax { .. }
            | StepKind::TableMin { .. }
            | StepKind::TableMaxN { .. }
            | StepKind::TableMinN { .. } => input_schema,

            // ReplaceValue/ReplaceErrorValues/InsertRows: schema passthrough
            StepKind::ReplaceValue { .. }
            | StepKind::ReplaceErrorValues { .. }
            | StepKind::InsertRows { .. } => input_schema,

            // ListGenerate: single-column "Value" result.
            StepKind::ListGenerate { .. } => {
                Some(vec![("Value".to_string(), ColumnType::Text)])
            }

            // ListTransform: single-column "Value" result.
            // The column type is derived from the transform lambda's return type.
            StepKind::ListTransform { transform, .. } => {
                let item_type = transform.inferred_type.clone()
                    .map(|t| match t {
                        ColumnType::Function(inner) => *inner,
                        other => other,
                    })
                    .unwrap_or(ColumnType::Text);
                Some(vec![("Value".to_string(), item_type)])
            }
        }
    }

    // ── per-step entry point ──────────────────────────────────────────────

    fn check_step_mut(&mut self, step_name: &str, step: &mut Step) {
        // 1. Resolve the input step's schema (owned clone, releases borrow).
        let input_schema: Option<Vec<(String, ColumnType)>> = {
            let input_name = input_step_name(&step.kind);
            input_name.and_then(|n| self.step_schemas.get(n).cloned())
        };

        // 2. Annotate all expression nodes using input schema as row context.
        //    Scoped block ensures schema_ref's borrow of input_schema ends
        //    before we move input_schema into compute_output_schema.
        {
            let schema_ref: Option<&[(String, ColumnType)]> = input_schema.as_deref();
            self.annotate_step_exprs(&mut step.kind, schema_ref);
        }

        // 3. Derive the output schema (expressions already annotated).
        let output_schema = self.compute_output_schema(&step.kind, input_schema);

        // 4. Store on the step node.
        step.output_type = output_schema.clone();

        // 5. Register for downstream steps.
        if let Some(schema) = output_schema {
            self.step_schemas.insert(step_name.to_string(), schema);
        }
    }

    // ── public entry point ────────────────────────────────────────────────

    pub fn check(&mut self, program: &mut Program) -> CheckResult {
        for binding in program.steps.iter_mut() {
            let step_name = binding.name.clone();
            self.check_step_mut(&step_name, &mut binding.step);
        }
        if self.diagnostics.is_empty() {
            Ok(())
        } else {
            Err(std::mem::take(&mut self.diagnostics))
        }
    }
}

// ── Step input-name helper ────────────────────────────────────────────────

fn input_step_name(kind: &StepKind) -> Option<&str> {
    match kind {
        StepKind::Source { .. }                  => None,
        StepKind::PromoteHeaders   { input }     => Some(input),
        StepKind::ChangeTypes      { input, .. } => Some(input),
        StepKind::Filter           { input, .. } => Some(input),
        StepKind::AddColumn        { input, .. } => Some(input),
        StepKind::RemoveColumns    { input, .. } => Some(input),
        StepKind::RenameColumns    { input, .. } => Some(input),
        StepKind::Sort             { input, .. } => Some(input),
        StepKind::TransformColumns { input, .. } => Some(input),
        StepKind::Group            { input, .. } => Some(input),
        // New operations
        StepKind::FirstN          { input, .. }
        | StepKind::LastN         { input, .. }
        | StepKind::Skip          { input, .. }
        | StepKind::Range         { input, .. }
        | StepKind::RemoveFirstN  { input, .. }
        | StepKind::RemoveLastN   { input, .. }
        | StepKind::RemoveRows    { input, .. }
        | StepKind::ReverseRows   { input }
        | StepKind::Distinct      { input, .. }
        | StepKind::Repeat        { input, .. }
        | StepKind::AlternateRows { input, .. }
        | StepKind::FindText      { input, .. }
        | StepKind::FillDown      { input, .. }
        | StepKind::FillUp        { input, .. }
        | StepKind::AddIndexColumn { input, .. }
        | StepKind::DuplicateColumn { input, .. }
        | StepKind::Unpivot       { input, .. }
        | StepKind::UnpivotOtherColumns { input, .. }
        | StepKind::Transpose     { input }
        | StepKind::RemoveRowsWithErrors  { input, .. }
        | StepKind::SelectRowsWithErrors  { input, .. }
        | StepKind::TransformRows { input, .. }
        | StepKind::MatchesAllRows { input, .. }
        | StepKind::MatchesAnyRows { input, .. }
        | StepKind::PrefixColumns  { input, .. }
        | StepKind::DemoteHeaders  { input }
        // New operations
        | StepKind::SelectColumns       { input, .. }
        | StepKind::ReorderColumns      { input, .. }
        | StepKind::TransformColumnNames { input, .. }
        | StepKind::CombineColumns      { input, .. }
        | StepKind::SplitColumn          { input, .. }
        | StepKind::ExpandTableColumn   { input, .. }
        | StepKind::ExpandRecordColumn  { input, .. }
        | StepKind::Pivot               { input, .. }
        | StepKind::RowCount            { input }
        | StepKind::ColumnCount         { input }
        | StepKind::TableColumnNames    { input }
        | StepKind::TableIsEmpty        { input }
        | StepKind::TableSchema         { input }
        | StepKind::HasColumns          { input, .. }
        | StepKind::TableIsDistinct     { input }
        | StepKind::AddRankColumn       { input, .. }
        | StepKind::TableMax            { input, .. }
        | StepKind::TableMin            { input, .. }
        | StepKind::TableMaxN           { input, .. }
        | StepKind::TableMinN           { input, .. }
        | StepKind::ReplaceValue        { input, .. }
        | StepKind::ReplaceErrorValues  { input, .. }
        | StepKind::InsertRows          { input, .. } => Some(input),
        StepKind::Join { left, .. }
        | StepKind::NestedJoin { left, .. } => Some(left),
        StepKind::CombineTables { inputs } => inputs.first().map(|s| s.as_str()),
        StepKind::Passthrough      { input, .. } => {
            if input.is_empty() { None } else { Some(input) }
        }
        // ListGenerate has no table input.
        StepKind::ListGenerate { .. } => None,
        // ListTransform has no table input; the list_expr is its own source.
        StepKind::ListTransform { .. } => None,
    }
}

// ── Function return-type inference via the grammar registry ───────────────

fn column_type_to_sig_type(ct: &ColumnType) -> Type {
    match ct {
        ColumnType::Integer | ColumnType::Float | ColumnType::Currency => Type::Number,
        ColumnType::Boolean                     => Type::Boolean,
        ColumnType::Text                        => Type::Text,
        ColumnType::Date
        | ColumnType::DateTime
        | ColumnType::DateTimeZone
        | ColumnType::Duration
        | ColumnType::Time
        | ColumnType::Binary                    => Type::Any,
        ColumnType::Null                        => nullable(Type::Any),
        ColumnType::Function(ret) => {
            // Represent as a single-param function so it can unify with
            // generic HOF signatures like `(T) → U`.
            Type::Function(Box::new(FunctionType::new(
                vec![Param::required(Type::Any)],
                column_type_to_sig_type(ret),
            )))
        }
        ColumnType::List(inner) => Type::List(Box::new(column_type_to_sig_type(inner))),
    }
}

fn sig_type_to_column_type(t: &Type) -> Option<ColumnType> {
    match t {
        Type::Number          => Some(ColumnType::Float),
        Type::Text            => Some(ColumnType::Text),
        Type::Boolean         => Some(ColumnType::Boolean),
        Type::Any             => Some(ColumnType::Text),
        Type::Nullable(inner) => sig_type_to_column_type(inner),
        Type::List(inner)     => {
            Some(ColumnType::List(Box::new(sig_type_to_column_type(inner)?)))
        }
        Type::Function(ft) => {
            sig_type_to_column_type(&ft.return_type)
                .map(|ret| ColumnType::Function(Box::new(ret)))
        }
        _ => None,
    }
}

fn infer_call_return(name: &str, arg_types: &[Option<ColumnType>]) -> Option<ColumnType> {
    let def = lookup_qualified(name)?;
    for overload in &def.signatures {
        if let Some(ct) = try_overload(overload, arg_types) {
            return Some(ct);
        }
    }
    None
}

fn try_overload(
    sig:       &pq_grammar::FunctionType,
    arg_types: &[Option<ColumnType>],
) -> Option<ColumnType> {
    if !sig.arity_matches(arg_types.len()) {
        return None;
    }
    let mut subst = HashMap::new();
    for (param, col_opt) in sig.params.iter().zip(arg_types.iter()) {
        match col_opt {
            // Unknown arg type → we cannot validate this overload.
            None => return None,
            Some(ct) => {
                let concrete = column_type_to_sig_type(ct);
                // If unification fails the types are incompatible; try next overload.
                if !unify(&param.ty, &concrete, &mut subst) {
                    return None;
                }
            }
        }
    }
    let ret = sig.return_type.substitute(&subst);
    sig_type_to_column_type(&ret)
}

// ── Helpers ───────────────────────────────────────────────────────────────

/// Determine what type `_` should be bound to when inferring a lambda argument
/// at position `lambda_arg_idx` in a call to `fn_name`, given the already-
/// inferred types of the other arguments (`known_arg_types`).
///
/// Works by building a substitution from the non-lambda args and then resolving
/// the expected function-parameter type from the overload signature.
fn determine_lambda_param_type(
    fn_name:        &str,
    lambda_arg_idx: usize,
    known_arg_types: &[Option<ColumnType>],
) -> Option<ColumnType> {
    let def = lookup_qualified(fn_name)?;

    for sig in &def.signatures {
        if lambda_arg_idx >= sig.params.len() {
            continue;
        }

        // Build a substitution from the non-lambda (already-known) args.
        let mut subst = HashMap::new();
        for (param, col_opt) in sig.params.iter().zip(known_arg_types.iter()) {
            if let Some(ct) = col_opt {
                let concrete = column_type_to_sig_type(ct);
                unify(&param.ty, &concrete, &mut subst);
            }
        }

        // Resolve the expected type at the lambda position.
        let expected = sig.params[lambda_arg_idx].ty.substitute(&subst);

        // The expected type should be a `(T) → U`; extract T.
        if let Type::Function(ft) = &expected {
            if let Some(first_param) = ft.params.first() {
                let param_ty = first_param.ty.substitute(&subst);
                return sig_type_to_column_type(&param_ty);
            }
        }
    }
    None
}

/// Unify two `ColumnType` values into a single compatible type.
///
/// Rules:
///   - identical types unify to themselves,
///   - `Integer` + `Float` (or vice-versa) widen to `Float`,
///   - `Null` is compatible with any concrete type (the concrete type wins).
fn unify_column_types(a: &ColumnType, b: &ColumnType) -> Option<ColumnType> {
    if a == b {
        return Some(a.clone());
    }
    match (a, b) {
        (ColumnType::Integer, ColumnType::Float)
        | (ColumnType::Float, ColumnType::Integer) => Some(ColumnType::Float),
        (ColumnType::Null, other) | (other, ColumnType::Null) => Some(other.clone()),
        _ => None,
    }
}

// ── tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use pq_lexer::Lexer;
    use pq_parser::Parser;
    use pq_pipeline::{build_table, RawWorkbook};

    fn make_table() -> Table {
        build_table(RawWorkbook {
            source: "test.xlsx".into(),
            sheet:  "Sheet1".into(),
            rows:   vec![
                vec!["Name".into(), "Age".into(), "Salary".into(), "Active".into()],
                vec!["Alice".into(), "30".into(), "50000.50".into(), "true".into()],
                vec!["Bob".into(),   "25".into(), "40000.00".into(), "false".into()],
            ],
        })
    }

    fn parse(input: &str) -> Program {
        let tokens = Lexer::new(input).tokenize().unwrap();
        Parser::new(tokens).parse().unwrap()
    }

    // ── existing validation tests (unchanged semantics) ───────────────────

    #[test]
    fn test_valid_filter() {
        let table       = make_table();
        let mut program = parse(r#"
            let
                Source   = Excel.Workbook(File.Contents("test.xlsx"), null, true),
                Filtered = Table.SelectRows(Source, each Age > 25)
            in Filtered
        "#);
        let mut checker = TypeChecker::new(&table);
        assert!(checker.check(&mut program).is_ok());
    }

    #[test]
    fn test_valid_filter_with_and() {
        let table       = make_table();
        let mut program = parse(r#"
            let
                Source   = Excel.Workbook(File.Contents("test.xlsx"), null, true),
                Filtered = Table.SelectRows(Source, each Age > 25 and Active = true)
            in Filtered
        "#);
        let mut checker = TypeChecker::new(&table);
        assert!(checker.check(&mut program).is_ok());
    }

    #[test]
    fn test_valid_add_column_int_float() {
        let table       = make_table();
        let mut program = parse(r#"
            let
                Source    = Excel.Workbook(File.Contents("test.xlsx"), null, true),
                WithBonus = Table.AddColumn(Source, "Bonus", each Salary + 1000.0)
            in WithBonus
        "#);
        let mut checker = TypeChecker::new(&table);
        assert!(checker.check(&mut program).is_ok());
    }

    #[test]
    fn test_type_mismatch_text_plus_int() {
        let table       = make_table();
        let mut program = parse(r#"
            let
                Source  = Excel.Workbook(File.Contents("test.xlsx"), null, true),
                WithCol = Table.AddColumn(Source, "Bad", each Name + 1)
            in WithCol
        "#);
        let mut checker = TypeChecker::new(&table);
        assert!(checker.check(&mut program).is_err());
    }

    #[test]
    fn test_arithmetic_on_boolean() {
        let table       = make_table();
        let mut program = parse(r#"
            let
                Source  = Excel.Workbook(File.Contents("test.xlsx"), null, true),
                WithCol = Table.AddColumn(Source, "Bad", each Active + 1)
            in WithCol
        "#);
        let mut checker = TypeChecker::new(&table);
        assert!(checker.check(&mut program).is_err());
    }

    #[test]
    fn test_filter_must_be_boolean() {
        let table       = make_table();
        let mut program = parse(r#"
            let
                Source   = Excel.Workbook(File.Contents("test.xlsx"), null, true),
                Filtered = Table.SelectRows(Source, each Age + 1)
            in Filtered
        "#);
        let mut checker = TypeChecker::new(&table);
        assert!(checker.check(&mut program).is_err());
    }

    #[test]
    fn test_int_float_mix_is_valid() {
        let table       = make_table();
        let mut program = parse(r#"
            let
                Source    = Excel.Workbook(File.Contents("test.xlsx"), null, true),
                WithBonus = Table.AddColumn(Source, "Bonus", each Age + 1000.0)
            in WithBonus
        "#);
        let mut checker = TypeChecker::new(&table);
        assert!(checker.check(&mut program).is_ok());
    }

    #[test]
    fn test_not_on_non_boolean_fails() {
        let table       = make_table();
        let mut program = parse(r#"
            let
                Source   = Excel.Workbook(File.Contents("test.xlsx"), null, true),
                Filtered = Table.SelectRows(Source, each not Age)
            in Filtered
        "#);
        let mut checker = TypeChecker::new(&table);
        assert!(checker.check(&mut program).is_err());
    }

    // ── annotation completeness tests ─────────────────────────────────────

    #[test]
    fn test_source_step_output_type_populated() {
        let table       = make_table();
        let mut program = parse(r#"
            let Source = Excel.Workbook(File.Contents("test.xlsx"), null, true) in Source
        "#);
        let mut checker = TypeChecker::new(&table);
        checker.check(&mut program).ok();
        let cols = program.steps[0].step.output_type.as_ref()
            .expect("Source must have output_type after checking");
        assert_eq!(cols.len(), 4);
    }

    #[test]
    fn test_add_column_output_type_includes_new_column() {
        let table       = make_table();
        let mut program = parse(r#"
            let
                Source    = Excel.Workbook(File.Contents("test.xlsx"), null, true),
                WithBonus = Table.AddColumn(Source, "Bonus", each Salary + 1000.0)
            in WithBonus
        "#);
        let mut checker = TypeChecker::new(&table);
        checker.check(&mut program).ok();
        let cols = program.steps[1].step.output_type.as_ref()
            .expect("WithBonus must have output_type");
        let bonus = cols.iter().find(|(n, _)| n == "Bonus");
        assert!(bonus.is_some(), "Bonus column must appear in output schema");
        assert_eq!(bonus.unwrap().1, ColumnType::Float);
    }

    #[test]
    fn test_remove_columns_shrinks_schema() {
        let table       = make_table();
        let mut program = parse(r#"
            let
                Source  = Excel.Workbook(File.Contents("test.xlsx"), null, true),
                Trimmed = Table.RemoveColumns(Source, {"Active"})
            in Trimmed
        "#);
        let mut checker = TypeChecker::new(&table);
        checker.check(&mut program).ok();
        let cols = program.steps[1].step.output_type.as_ref().unwrap();
        assert_eq!(cols.len(), 3);
        assert!(cols.iter().all(|(n, _)| n != "Active"));
    }

    #[test]
    fn test_rename_columns_updates_schema() {
        let table       = make_table();
        let mut program = parse(r#"
            let
                Source  = Excel.Workbook(File.Contents("test.xlsx"), null, true),
                Renamed = Table.RenameColumns(Source, {{"Name", "FullName"}})
            in Renamed
        "#);
        let mut checker = TypeChecker::new(&table);
        checker.check(&mut program).ok();
        let cols = program.steps[1].step.output_type.as_ref().unwrap();
        assert!(cols.iter().any(|(n, _)| n == "FullName"));
        assert!(cols.iter().all(|(n, _)| n != "Name"));
    }

    #[test]
    fn test_change_types_updates_column_types() {
        let table       = make_table();
        let mut program = parse(r#"
            let
                Source = Excel.Workbook(File.Contents("test.xlsx"), null, true),
                Typed  = Table.TransformColumnTypes(Source, {{"Age", Int64.Type}, {"Salary", Number.Type}})
            in Typed
        "#);
        let mut checker = TypeChecker::new(&table);
        checker.check(&mut program).ok();
        let cols = program.steps[1].step.output_type.as_ref().unwrap();
        assert_eq!(cols.len(), 4);
        let age_ty    = cols.iter().find(|(n, _)| n == "Age").map(|(_, t)| t);
        let salary_ty = cols.iter().find(|(n, _)| n == "Salary").map(|(_, t)| t);
        assert_eq!(age_ty,    Some(&ColumnType::Integer));
        assert_eq!(salary_ty, Some(&ColumnType::Float));
    }

    #[test]
    fn test_filter_expr_nodes_annotated() {
        let table       = make_table();
        let mut program = parse(r#"
            let
                Source   = Excel.Workbook(File.Contents("test.xlsx"), null, true),
                Filtered = Table.SelectRows(Source, each Age > 25)
            in Filtered
        "#);
        let mut checker = TypeChecker::new(&table);
        checker.check(&mut program).ok();

        use pq_ast::step::StepKind;
        let step = &program.steps[1].step;
        if let StepKind::Filter { condition, .. } = &step.kind {
            assert_eq!(
                condition.inferred_type,
                Some(ColumnType::Function(Box::new(ColumnType::Boolean)))
            );
            if let pq_ast::expr::Expr::Lambda { body, .. } = &condition.expr {
                assert_eq!(body.inferred_type, Some(ColumnType::Boolean));
                if let pq_ast::expr::Expr::BinaryOp { left, right, .. } = &body.expr {
                    assert_eq!(left.inferred_type,  Some(ColumnType::Integer));
                    assert_eq!(right.inferred_type, Some(ColumnType::Integer));
                }
            }
        }
    }

    #[test]
    fn test_step_schemas_propagated() {
        let table       = make_table();
        let mut program = parse(r#"
            let
                Source   = Excel.Workbook(File.Contents("test.xlsx"), null, true),
                Filtered = Table.SelectRows(Source, each Age > 25),
                Removed  = Table.RemoveColumns(Filtered, {"Active"})
            in Removed
        "#);
        let mut checker = TypeChecker::new(&table);
        checker.check(&mut program).ok();

        assert!(checker.step_schemas.contains_key("Source"));
        assert!(checker.step_schemas.contains_key("Filtered"));
        assert!(checker.step_schemas.contains_key("Removed"));
        assert!(checker.step_schemas["Removed"].iter().all(|(n, _)| n != "Active"));
    }

    // ── List type-inference tests ─────────────────────────────────────────

    /// An integer list literal should infer as `List<Integer>`.
    #[test]
    fn test_list_literal_integer_type() {
        let table       = make_table();
        let mut program = parse(r#"
            let Result = List.Transform({1, 2, 3}, each Text.Length(_)) in Result
        "#);
        let mut checker = TypeChecker::new(&table);
        // The call will fail (type error), but we still check the list node.
        checker.check(&mut program).ok();

        use pq_ast::step::StepKind;
        if let StepKind::ListTransform { list_expr, .. } = &program.steps[0].step.kind {
            assert_eq!(
                list_expr.inferred_type,
                Some(ColumnType::List(Box::new(ColumnType::Integer))),
                "integer list literal should infer as List<Integer>"
            );
        } else {
            panic!("expected ListTransform step");
        }
    }

    /// A text list literal should infer as `List<Text>`.
    #[test]
    fn test_list_literal_text_type() {
        let table       = make_table();
        let mut program = parse(r#"
            let Result = List.Transform({"a", "b", "c"}, each Text.Length(_)) in Result
        "#);
        let mut checker = TypeChecker::new(&table);
        checker.check(&mut program).ok();

        use pq_ast::step::StepKind;
        if let StepKind::ListTransform { list_expr, .. } = &program.steps[0].step.kind {
            assert_eq!(
                list_expr.inferred_type,
                Some(ColumnType::List(Box::new(ColumnType::Text))),
                "text list literal should infer as List<Text>"
            );
        } else {
            panic!("expected ListTransform step");
        }
    }

    // ── Higher-order function validation tests ────────────────────────────

    /// `List.Transform({1,2,3}, each Text.Length(_))` must fail because `_`
    /// is `Integer` and `Text.Length` requires `Text`.
    #[test]
    fn test_list_transform_wrong_element_type_errors() {
        let table       = make_table();
        let mut program = parse(r#"
            let Result = List.Transform({1, 2, 3}, each Text.Length(_)) in Result
        "#);
        let mut checker = TypeChecker::new(&table);
        let result = checker.check(&mut program);
        assert!(
            result.is_err(),
            "Text.Length(_) where _ is Integer should produce a type error"
        );
    }

    /// `List.Transform({"a","b"}, each Text.Length(_))` must succeed because
    /// `_` is `Text` and `Text.Length` accepts `Text`.
    #[test]
    fn test_list_transform_correct_element_type_passes() {
        let table       = make_table();
        let mut program = parse(r#"
            let Result = List.Transform({"a", "b", "c"}, each Text.Length(_)) in Result
        "#);
        let mut checker = TypeChecker::new(&table);
        let result = checker.check(&mut program);
        assert!(
            result.is_ok(),
            "Text.Length(_) where _ is Text should pass type checking"
        );
    }

    /// `List.Transform({"a","b"}, each Text.Length(_))` should produce a
    /// `List<Float>` output schema (Text.Length returns Number).
    #[test]
    fn test_list_transform_output_schema_type() {
        let table       = make_table();
        let mut program = parse(r#"
            let Result = List.Transform({"a", "b", "c"}, each Text.Length(_)) in Result
        "#);
        let mut checker = TypeChecker::new(&table);
        checker.check(&mut program).ok();

        let cols = program.steps[0].step.output_type.as_ref()
            .expect("ListTransform must have output_type after checking");
        assert_eq!(cols.len(), 1);
        assert_eq!(cols[0].0, "Value");
        // Text.Length returns Number, which maps to Float in ColumnType.
        assert_eq!(cols[0].1, ColumnType::Float);
    }

    /// `_` inside a `TransformColumns` lambda should be bound to the column
    /// type.  `each _ * 2` on a Float column must produce Float.
    #[test]
    fn test_transform_columns_binds_underscore() {
        let table       = make_table();
        let mut program = parse(r#"
            let
                Source  = Excel.Workbook(File.Contents("test.xlsx"), null, true),
                Scaled  = Table.TransformColumns(Source, {{"Salary", each _ * 2}})
            in Scaled
        "#);
        let mut checker = TypeChecker::new(&table);
        let result = checker.check(&mut program);
        assert!(result.is_ok(), "_ * 2 on a Float column should pass: {:?}", result);
    }
}
