use std::collections::HashMap;
use pq_ast::{
    Program,
    expr::{Expr, ExprNode},
    step::StepKind,
};
use pq_diagnostics::{Diagnostic, Span};
use pq_pipeline::Table;
use crate::scope::Scope;

pub type ResolveResult = Result<(), Vec<Diagnostic>>;

pub struct Resolver<'a> {
    table:        &'a Table,
    diagnostics:  Vec<Diagnostic>,
    /// Maps each step name to the column names it produces.
    /// Populated as steps are resolved so later steps can validate
    /// column references against the actual schema at that point in
    /// the pipeline — not just the raw source table.
    step_schemas: HashMap<String, Vec<String>>,
}

impl<'a> Resolver<'a> {
    pub fn new(table: &'a Table) -> Self {
        Resolver {
            table,
            diagnostics: vec![],
            step_schemas: HashMap::new(),
        }
    }

    // ── diagnostic helpers ────────────────────────────────────────────────

    fn unknown_step(&mut self, name: &str, span: Span, scope: &Scope, context: &str) {
        let mut d = Diagnostic::error("E301", format!("unknown step '{}'", name))
            .with_label(span, format!("'{}' is not defined", name));
        if !context.is_empty() {
            d = d.with_label(
                pq_diagnostics::Span::dummy(),
                format!("referenced in step '{}'", context),
            );
        }
        if let Some(closest) = scope.closest_match(name) {
            d = d.with_suggestion(format!("did you mean '{}'?", closest));
        }
        self.diagnostics.push(d);
    }

    fn unknown_column(&mut self, name: &str, span: Span) {
        let col_names = self.table.column_names();
        let mut d = Diagnostic::error("E302", format!("unknown column '{}'", name))
            .with_label(span, format!("column '{}' does not exist", name));
        let closest = col_names.iter()
            .filter_map(|c| {
                let dist = edit_distance(c, name);
                if dist <= 3 { Some((dist, *c)) } else { None }
            })
            .min_by_key(|(d, _)| *d)
            .map(|(_, c)| c);
        if let Some(c) = closest {
            d = d.with_suggestion(format!("did you mean '{}'?", c));
        } else {
            d = d.with_suggestion(format!("available columns: {}", col_names.join(", ")));
        }
        self.diagnostics.push(d);
    }

    fn column_already_exists(&mut self, name: &str, span: Span) {
        self.diagnostics.push(
            Diagnostic::error("E303", format!("column '{}' already exists", name))
                .with_label(span, "duplicate column name")
                .with_suggestion("choose a different name for the new column"),
        );
    }

    fn unknown_output_step(&mut self, name: &str, span: Span, scope: &Scope) {
        let mut d = Diagnostic::error("E304", format!("output step '{}' does not exist", name))
            .with_label(span, "this step was never defined");
        if let Some(closest) = scope.closest_match(name) {
            d = d.with_suggestion(format!("did you mean '{}'?", closest));
        }
        self.diagnostics.push(d);
    }

    // ── expression resolver ───────────────────────────────────────────────

    /// Walk an expression tree and validate all column references.
    ///
    /// `schema` is the list of column names available at this point in the
    /// pipeline (the output of the current step's input step).  When `None`
    /// the schema is unknown (e.g. a `Table.FromRecords` source) and column
    /// validation is skipped to avoid false positives.
    fn resolve_expr(&mut self, node: &ExprNode, schema: Option<&[String]>) {
        match &node.expr {
            // Bare identifier: variable, step reference, or `_` lambda param.
            // Dotted names (e.g. "Order.Ascending") are never column names.
            Expr::Identifier(name) => {
                if let Some(cols) = schema {
                    if !name.contains('.') && !cols.iter().any(|c| c == name) && name != "_" {
                        self.unknown_column(name, node.span.clone());
                    }
                }
                // schema == None → unknown pipeline shape, skip validation
            }

            // Bracket column access: [ColName]
            Expr::ColumnAccess(name) => {
                if let Some(cols) = schema {
                    if !cols.iter().any(|c| c == name) {
                        self.unknown_column(name, node.span.clone());
                    }
                }
            }

            // Field access on a named record variable: `row[ColName]`
            Expr::FieldAccess { record, field } => {
                self.resolve_expr(record, schema);
                if let Some(cols) = schema {
                    if !cols.iter().any(|c| c == field) {
                        self.unknown_column(field, node.span.clone());
                    }
                }
            }

            // Lambda covers both explicit `(x) => body` and desugared `each body`
            // (param == "_").  Resolve the body in row context.
            Expr::Lambda { body, .. } => {
                self.resolve_expr(body, schema);
            }

            // Binary / unary — recurse
            Expr::BinaryOp { left, right, .. } => {
                self.resolve_expr(left, schema);
                self.resolve_expr(right, schema);
            }
            Expr::UnaryOp { operand, .. } => {
                self.resolve_expr(operand, schema);
            }

            // Function call — resolve each argument expression
            Expr::FunctionCall { args, .. } => {
                for arg in args { self.resolve_expr(arg, schema); }
            }

            // List / Record — resolve all children
            Expr::List(items) => {
                for item in items { self.resolve_expr(item, schema); }
            }
            Expr::Record(fields) => {
                for (_, val) in fields { self.resolve_expr(val, schema); }
            }

            // Literals are always valid
            Expr::IntLit(_)
            | Expr::FloatLit(_)
            | Expr::BoolLit(_)
            | Expr::StringLit(_)
            | Expr::NullLit => {}
        }
    }

    // ── schema helpers ────────────────────────────────────────────────────

    /// Returns the column names produced by the step referenced by `input_name`.
    /// Falls back to the raw source table when `input_name` is empty or unknown.
    /// Returns `None` when the schema is genuinely unknowable (e.g. the input
    /// step is a Passthrough with no tracked schema), so callers can skip
    /// column validation rather than producing false positives.
    fn schema_of(&self, input_name: &str) -> Option<Vec<String>> {
        if input_name.is_empty() {
            return None;
        }
        self.step_schemas
            .get(input_name)
            .cloned()
            // Fall back to raw table columns if the step isn't tracked yet.
            // This handles the very first `Source` reference before any schema
            // has been registered.
            .or_else(|| {
                Some(self.table.column_names().iter().map(|s| s.to_string()).collect())
            })
    }

    /// Compute and store the output schema for a step after it has been resolved.
    fn register_output_schema(&mut self, step_name: &str, kind: &StepKind) {
        let output: Option<Vec<String>> = match kind {
            // Source: output = raw JSON table columns
            StepKind::Source { .. } => {
                Some(self.table.column_names().iter().map(|s| s.to_string()).collect())
            }

            // Pass-through shape: same columns as input
            StepKind::PromoteHeaders { input }
            | StepKind::Filter        { input, .. }
            | StepKind::Sort          { input, .. }
            | StepKind::TransformColumns { input, .. }
            | StepKind::Passthrough   { input, .. } => {
                self.schema_of(input)
            }

            StepKind::ChangeTypes { input, .. } => self.schema_of(input),

            // AddColumn: input schema + new column
            StepKind::AddColumn { input, col_name, .. } => {
                match self.schema_of(input) {
                    Some(mut cols) => { cols.push(col_name.clone()); Some(cols) }
                    None => None,
                }
            }

            // RemoveColumns: input schema minus removed columns
            StepKind::RemoveColumns { input, columns } => {
                self.schema_of(input).map(|cols| {
                    cols.into_iter().filter(|n| !columns.contains(n)).collect()
                })
            }

            // RenameColumns: apply renames to input schema
            StepKind::RenameColumns { input, renames } => {
                self.schema_of(input).map(|cols| {
                    cols.into_iter()
                        .map(|n| {
                            renames.iter()
                                .find(|(old, _)| old == &n)
                                .map(|(_, new)| new.clone())
                                .unwrap_or(n)
                        })
                        .collect()
                })
            }

            // Group: key columns + aggregate output names
            StepKind::Group { by, aggregates, .. } => {
                let mut cols = by.clone();
                for agg in aggregates {
                    cols.push(agg.name.clone());
                }
                Some(cols)
            }

            // ── New row operations: pass-through schema ──────────────────
            StepKind::FirstN          { input, .. }
            | StepKind::LastN         { input, .. }
            | StepKind::Skip          { input, .. }
            | StepKind::Range         { input, .. }
            | StepKind::RemoveFirstN  { input, .. }
            | StepKind::RemoveLastN   { input, .. }
            | StepKind::RemoveRows    { input, .. }
            | StepKind::ReverseRows   { input }
            | StepKind::Repeat        { input, .. }
            | StepKind::AlternateRows { input, .. }
            | StepKind::FindText      { input, .. }
            | StepKind::FillDown      { input, .. }
            | StepKind::FillUp        { input, .. }
            | StepKind::RemoveRowsWithErrors  { input, .. }
            | StepKind::SelectRowsWithErrors  { input, .. }
            | StepKind::TransformRows { input, .. }
            | StepKind::MatchesAllRows { input, .. }
            | StepKind::MatchesAnyRows { input, .. }
            | StepKind::DemoteHeaders  { input } => {
                self.schema_of(input)
            }

            // Distinct: schema passthrough
            StepKind::Distinct { input, .. } => self.schema_of(input),

            // Transpose: dynamic schema — column names unknown
            StepKind::Transpose { .. } => None,

            // AddIndexColumn: input schema + new column
            StepKind::AddIndexColumn { input, col_name, .. } => {
                match self.schema_of(input) {
                    Some(mut cols) => { cols.push(col_name.clone()); Some(cols) }
                    None => None,
                }
            }

            // DuplicateColumn: input schema + new column
            StepKind::DuplicateColumn { input, new_col, .. } => {
                match self.schema_of(input) {
                    Some(mut cols) => { cols.push(new_col.clone()); Some(cols) }
                    None => None,
                }
            }

            // Unpivot: non-unpivoted cols + attr + val
            StepKind::Unpivot { input, columns, attr_col, val_col } => {
                self.schema_of(input).map(|cols| {
                    let mut out: Vec<String> = cols.into_iter()
                        .filter(|c| !columns.contains(c))
                        .collect();
                    out.push(attr_col.clone());
                    out.push(val_col.clone());
                    out
                })
            }

            // UnpivotOtherColumns: keep cols + attr + val
            StepKind::UnpivotOtherColumns { keep_cols, attr_col, val_col, .. } => {
                let mut out = keep_cols.clone();
                out.push(attr_col.clone());
                out.push(val_col.clone());
                Some(out)
            }

            // Combine: union of all input schemas
            StepKind::CombineTables { inputs } => {
                let mut all_cols: Vec<String> = Vec::new();
                for inp in inputs {
                    if let Some(cols) = self.schema_of(inp) {
                        for c in cols {
                            if !all_cols.contains(&c) {
                                all_cols.push(c);
                            }
                        }
                    }
                }
                if all_cols.is_empty() { None } else { Some(all_cols) }
            }

            // PrefixColumns: rename all columns with prefix.
            StepKind::PrefixColumns { input, prefix } => {
                self.schema_of(input).map(|cols| {
                    cols.into_iter().map(|c| format!("{}.{}", prefix, c)).collect()
                })
            }

            // SelectColumns: keep only listed columns in order
            StepKind::SelectColumns { columns, .. } => Some(columns.clone()),

            // ReorderColumns: listed columns first, then remaining
            StepKind::ReorderColumns { input, columns } => {
                self.schema_of(input).map(|cols| {
                    let mut out = columns.clone();
                    for c in &cols {
                        if !out.contains(c) {
                            out.push(c.clone());
                        }
                    }
                    out
                })
            }

            // TransformColumnNames: names change dynamically
            StepKind::TransformColumnNames { .. } => None,

            // CombineColumns: input minus merged cols + new col
            StepKind::CombineColumns { input, columns, new_col, .. } => {
                self.schema_of(input).map(|cols| {
                    let mut out: Vec<String> = cols.into_iter()
                        .filter(|c| !columns.contains(c))
                        .collect();
                    out.push(new_col.clone());
                    out
                })
            }

            // SplitColumn: input minus split col (new cols are dynamic)
            StepKind::SplitColumn { input, col_name, .. } => {
                self.schema_of(input).map(|cols| {
                    cols.into_iter().filter(|c| c != col_name).collect()
                })
            }

            // ExpandTableColumn/ExpandRecordColumn: input minus expanded col + new cols
            StepKind::ExpandTableColumn { input, col_name, columns }
            | StepKind::ExpandRecordColumn { input, col_name, fields: columns } => {
                self.schema_of(input).map(|cols| {
                    let mut out: Vec<String> = cols.into_iter()
                        .filter(|c| c != col_name)
                        .collect();
                    out.extend(columns.iter().cloned());
                    out
                })
            }

            // Pivot: dynamic schema
            StepKind::Pivot { .. } => None,

            // Information functions: single-value output
            StepKind::RowCount { .. }
            | StepKind::ColumnCount { .. } => Some(vec!["Value".to_string()]),

            StepKind::TableColumnNames { .. } => Some(vec!["Value".to_string()]),

            StepKind::TableIsEmpty { .. }
            | StepKind::TableIsDistinct { .. } => Some(vec!["Value".to_string()]),

            StepKind::HasColumns { .. } => Some(vec!["Value".to_string()]),

            StepKind::TableSchema { .. } => {
                Some(vec!["Name".to_string(), "Kind".to_string(), "IsNullable".to_string()])
            }

            // Join: all columns from both tables
            StepKind::Join { left, right, .. } => {
                let mut all = self.schema_of(left).unwrap_or_default();
                if let Some(right_cols) = self.schema_of(right) {
                    for c in right_cols {
                        if !all.contains(&c) {
                            all.push(c);
                        }
                    }
                }
                Some(all)
            }

            // NestedJoin: left schema + new column
            StepKind::NestedJoin { left, new_col, .. } => {
                match self.schema_of(left) {
                    Some(mut cols) => { cols.push(new_col.clone()); Some(cols) }
                    None => None,
                }
            }

            // AddRankColumn: input + new column
            StepKind::AddRankColumn { input, col_name, .. } => {
                match self.schema_of(input) {
                    Some(mut cols) => { cols.push(col_name.clone()); Some(cols) }
                    None => None,
                }
            }

            // TableMax/Min: single row, same schema
            StepKind::TableMax { input, .. }
            | StepKind::TableMin { input, .. } => self.schema_of(input),

            // MaxN/MinN: same schema, subset of rows
            StepKind::TableMaxN { input, .. }
            | StepKind::TableMinN { input, .. } => self.schema_of(input),

            // ReplaceValue/ReplaceErrorValues: schema passthrough
            StepKind::ReplaceValue { input, .. }
            | StepKind::ReplaceErrorValues { input, .. }
            | StepKind::InsertRows { input, .. } => self.schema_of(input),

            // ListGenerate / ListTransform: single "Value" column
            StepKind::ListGenerate { .. }
            | StepKind::ListTransform { .. } => Some(vec!["Value".to_string()]),
        };

        if let Some(cols) = output {
            self.step_schemas.insert(step_name.to_string(), cols);
        }
    }

    // ── step validation helper ────────────────────────────────────────────

    /// Encapsulates the three-part pattern repeated in every step arm:
    /// 1. Verify `input` is a known step in `scope`.
    /// 2. Verify every name in `col_names` exists in `schema_ref`.
    /// 3. Recursively resolve every expression in `exprs`.
    ///
    /// `input` may be empty (e.g. `Passthrough` with no predecessor) — the
    /// scope check is skipped in that case.
    fn validate_step(
        &mut self,
        input:      &str,
        col_names:  &[&str],
        exprs:      &[&ExprNode],
        step_name:  &str,
        step_span:  &Span,
        scope:      &Scope,
        schema_ref: Option<&[String]>,
    ) {
        if !input.is_empty() && !scope.contains(input) {
            self.unknown_step(input, step_span.clone(), scope, step_name);
        }
        if let Some(cols) = schema_ref {
            for col in col_names {
                if !cols.iter().any(|c| c == col) {
                    self.unknown_column(col, step_span.clone());
                }
            }
        }
        for expr in exprs {
            self.resolve_expr(expr, schema_ref);
        }
    }

    // ── step resolver ─────────────────────────────────────────────────────

    fn resolve_step(
        &mut self,
        step_name: &str,
        step_span: &Span,
        kind:      &StepKind,
        scope:     &Scope,
    ) {
        let input_schema: Option<Vec<String>> = match kind {
            StepKind::Source { .. }                  => None,
            StepKind::PromoteHeaders   { input }     => self.schema_of(input),
            StepKind::ChangeTypes      { input, .. } => self.schema_of(input),
            StepKind::Filter           { input, .. } => self.schema_of(input),
            StepKind::AddColumn        { input, .. } => self.schema_of(input),
            StepKind::RemoveColumns    { input, .. } => self.schema_of(input),
            StepKind::RenameColumns    { input, .. } => self.schema_of(input),
            StepKind::Sort             { input, .. } => self.schema_of(input),
            StepKind::TransformColumns { input, .. } => self.schema_of(input),
            StepKind::Group            { input, .. } => self.schema_of(input),
            // New row/column operations
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
            // New column/value ops
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
            | StepKind::InsertRows          { input, .. } => self.schema_of(input),
            StepKind::Join { left, .. }
            | StepKind::NestedJoin { left, .. } => self.schema_of(left),
            StepKind::CombineTables { inputs } => {
                inputs.first().and_then(|i| self.schema_of(i))
            }
            StepKind::Passthrough      { input, .. } if !input.is_empty() => self.schema_of(input),
            StepKind::Passthrough      { .. }        => None,
            StepKind::ListGenerate     { .. }        => None,
            StepKind::ListTransform    { .. }        => None,
        };
        let schema_ref: Option<&[String]> = input_schema.as_deref();

        match kind {
            StepKind::Source { .. } => {}

            StepKind::PromoteHeaders { input } => {
                self.validate_step(input, &[], &[], step_name, step_span, scope, schema_ref);
            }

            StepKind::ChangeTypes { input, columns } => {
                let col_names: Vec<&str> = columns.iter().map(|(n, _)| n.as_str()).collect();
                self.validate_step(input, &col_names, &[], step_name, step_span, scope, schema_ref);
            }

            StepKind::Filter { input, condition } => {
                self.validate_step(input, &[], &[condition], step_name, step_span, scope, schema_ref);
            }

            // AddColumn uses `column_already_exists` instead of `unknown_column`,
            // so the col_name check stays inline.
            StepKind::AddColumn { input, col_name, expression } => {
                if !input.is_empty() && !scope.contains(input) {
                    self.unknown_step(input, step_span.clone(), scope, step_name);
                }
                if let Some(ref cols) = input_schema {
                    if cols.iter().any(|c| c == col_name) {
                        self.column_already_exists(col_name, step_span.clone());
                    }
                }
                self.resolve_expr(expression, schema_ref);
            }

            StepKind::RemoveColumns { input, columns } => {
                let col_names: Vec<&str> = columns.iter().map(|n| n.as_str()).collect();
                self.validate_step(input, &col_names, &[], step_name, step_span, scope, schema_ref);
            }

            StepKind::RenameColumns { input, renames } => {
                let col_names: Vec<&str> = renames.iter().map(|(old, _)| old.as_str()).collect();
                self.validate_step(input, &col_names, &[], step_name, step_span, scope, schema_ref);
            }

            StepKind::Sort { input, by } => {
                let col_names: Vec<&str> = by.iter().map(|(n, _)| n.as_str()).collect();
                self.validate_step(input, &col_names, &[], step_name, step_span, scope, schema_ref);
            }

            StepKind::TransformColumns { input, transforms } => {
                let col_names: Vec<&str> = transforms.iter().map(|(n, _, _)| n.as_str()).collect();
                let exprs: Vec<&ExprNode> = transforms.iter().map(|(_, e, _)| e).collect();
                self.validate_step(input, &col_names, &exprs, step_name, step_span, scope, schema_ref);
            }

            StepKind::Group { input, by, aggregates } => {
                let col_names: Vec<&str> = by.iter().map(|n| n.as_str()).collect();
                let exprs: Vec<&ExprNode> = aggregates.iter().map(|a| &a.expression).collect();
                self.validate_step(input, &col_names, &exprs, step_name, step_span, scope, schema_ref);
            }

            // ── New row operations: validate input step only ─────────────
            StepKind::FirstN         { input, .. }
            | StepKind::LastN        { input, .. }
            | StepKind::Skip         { input, .. }
            | StepKind::Range        { input, .. }
            | StepKind::RemoveFirstN { input, .. }
            | StepKind::RemoveLastN  { input, .. }
            | StepKind::RemoveRows   { input, .. }
            | StepKind::ReverseRows  { input }
            | StepKind::Repeat       { input, .. }
            | StepKind::AlternateRows { input, .. }
            | StepKind::Transpose    { input }
            | StepKind::DemoteHeaders { input } => {
                self.validate_step(input, &[], &[], step_name, step_span, scope, schema_ref);
            }

            StepKind::FindText { input, .. } => {
                self.validate_step(input, &[], &[], step_name, step_span, scope, schema_ref);
            }

            StepKind::PrefixColumns { input, .. } => {
                self.validate_step(input, &[], &[], step_name, step_span, scope, schema_ref);
            }

            // Distinct: validate step + column names
            StepKind::Distinct { input, columns } => {
                let col_names: Vec<&str> = columns.iter().map(|n| n.as_str()).collect();
                self.validate_step(input, &col_names, &[], step_name, step_span, scope, schema_ref);
            }

            // FillDown/FillUp: validate step + column names
            StepKind::FillDown { input, columns }
            | StepKind::FillUp { input, columns } => {
                let col_names: Vec<&str> = columns.iter().map(|n| n.as_str()).collect();
                self.validate_step(input, &col_names, &[], step_name, step_span, scope, schema_ref);
            }

            StepKind::RemoveRowsWithErrors { input, columns }
            | StepKind::SelectRowsWithErrors { input, columns } => {
                let col_names: Vec<&str> = columns.iter().map(|n| n.as_str()).collect();
                self.validate_step(input, &col_names, &[], step_name, step_span, scope, schema_ref);
            }

            // AddIndexColumn: validate step + check new col doesn't exist
            StepKind::AddIndexColumn { input, col_name, .. } => {
                if !scope.contains(input) {
                    self.unknown_step(input, step_span.clone(), scope, step_name);
                }
                if let Some(ref cols) = input_schema {
                    if cols.iter().any(|c| c == col_name) {
                        self.column_already_exists(col_name, step_span.clone());
                    }
                }
            }

            // DuplicateColumn: validate step + check columns
            StepKind::DuplicateColumn { input, src_col, new_col } => {
                if !scope.contains(input) {
                    self.unknown_step(input, step_span.clone(), scope, step_name);
                }
                if let Some(ref cols) = input_schema {
                    if !cols.iter().any(|c| c == src_col) {
                        self.unknown_column(src_col, step_span.clone());
                    }
                    if cols.iter().any(|c| c == new_col) {
                        self.column_already_exists(new_col, step_span.clone());
                    }
                }
            }

            // Unpivot: validate columns exist
            StepKind::Unpivot { input, columns, .. } => {
                let col_names: Vec<&str> = columns.iter().map(|n| n.as_str()).collect();
                self.validate_step(input, &col_names, &[], step_name, step_span, scope, schema_ref);
            }

            // UnpivotOtherColumns: validate keep_cols exist
            StepKind::UnpivotOtherColumns { input, keep_cols, .. } => {
                let col_names: Vec<&str> = keep_cols.iter().map(|n| n.as_str()).collect();
                self.validate_step(input, &col_names, &[], step_name, step_span, scope, schema_ref);
            }

            // CombineTables: validate all step refs
            StepKind::CombineTables { inputs } => {
                for inp in inputs {
                    if !scope.contains(inp.as_str()) {
                        self.unknown_step(inp, step_span.clone(), scope, step_name);
                    }
                }
            }

            // TransformRows: validate step + resolve expression
            StepKind::TransformRows { input, transform } => {
                self.validate_step(input, &[], &[transform], step_name, step_span, scope, schema_ref);
            }

            // MatchesAllRows / MatchesAnyRows: validate step + resolve predicate
            StepKind::MatchesAllRows { input, condition }
            | StepKind::MatchesAnyRows { input, condition } => {
                self.validate_step(input, &[], &[condition], step_name, step_span, scope, schema_ref);
            }

            // ── New column operations ───────────────────────────────────
            StepKind::SelectColumns { input, columns }
            | StepKind::ReorderColumns { input, columns } => {
                let col_names: Vec<&str> = columns.iter().map(|n| n.as_str()).collect();
                self.validate_step(input, &col_names, &[], step_name, step_span, scope, schema_ref);
            }

            StepKind::TransformColumnNames { input, transform } => {
                self.validate_step(input, &[], &[transform], step_name, step_span, scope, schema_ref);
            }

            StepKind::CombineColumns { input, columns, combiner, .. } => {
                let col_names: Vec<&str> = columns.iter().map(|n| n.as_str()).collect();
                self.validate_step(input, &col_names, &[combiner], step_name, step_span, scope, schema_ref);
            }

            StepKind::SplitColumn { input, col_name, splitter } => {
                self.validate_step(input, &[col_name.as_str()], &[splitter], step_name, step_span, scope, schema_ref);
            }

            StepKind::ExpandTableColumn { input, col_name, .. }
            | StepKind::ExpandRecordColumn { input, col_name, .. } => {
                self.validate_step(input, &[col_name.as_str()], &[], step_name, step_span, scope, schema_ref);
            }

            StepKind::Pivot { input, .. } => {
                self.validate_step(input, &[], &[], step_name, step_span, scope, schema_ref);
            }

            // ── Information functions ────────────────────────────────────
            StepKind::RowCount { input }
            | StepKind::ColumnCount { input }
            | StepKind::TableColumnNames { input }
            | StepKind::TableIsEmpty { input }
            | StepKind::TableSchema { input }
            | StepKind::TableIsDistinct { input } => {
                self.validate_step(input, &[], &[], step_name, step_span, scope, schema_ref);
            }

            StepKind::HasColumns { input, columns } => {
                // Don't validate column names — the function itself checks existence
                self.validate_step(input, &[], &[], step_name, step_span, scope, schema_ref);
                let _ = columns;
            }

            // ── Joins ─────────────────────────────────────────────────────
            StepKind::Join { left, left_keys, right, right_keys, .. } => {
                if !scope.contains(left.as_str()) {
                    self.unknown_step(left, step_span.clone(), scope, step_name);
                }
                if !scope.contains(right.as_str()) {
                    self.unknown_step(right, step_span.clone(), scope, step_name);
                }
                if let Some(ref left_schema) = self.schema_of(left) {
                    for k in left_keys {
                        if !left_schema.iter().any(|c| c == k) {
                            self.unknown_column(k, step_span.clone());
                        }
                    }
                }
                if let Some(ref right_schema) = self.schema_of(right) {
                    for k in right_keys {
                        if !right_schema.iter().any(|c| c == k) {
                            self.unknown_column(k, step_span.clone());
                        }
                    }
                }
            }

            StepKind::NestedJoin { left, left_keys, right, right_keys, new_col, .. } => {
                if !scope.contains(left.as_str()) {
                    self.unknown_step(left, step_span.clone(), scope, step_name);
                }
                if !scope.contains(right.as_str()) {
                    self.unknown_step(right, step_span.clone(), scope, step_name);
                }
                if let Some(ref left_schema) = self.schema_of(left) {
                    if left_schema.iter().any(|c| c == new_col) {
                        self.column_already_exists(new_col, step_span.clone());
                    }
                    for k in left_keys {
                        if !left_schema.iter().any(|c| c == k) {
                            self.unknown_column(k, step_span.clone());
                        }
                    }
                }
                if let Some(ref right_schema) = self.schema_of(right) {
                    for k in right_keys {
                        if !right_schema.iter().any(|c| c == k) {
                            self.unknown_column(k, step_span.clone());
                        }
                    }
                }
            }

            // ── Ordering ─────────────────────────────────────────────────
            StepKind::AddRankColumn { input, col_name, by } => {
                if !scope.contains(input.as_str()) {
                    self.unknown_step(input, step_span.clone(), scope, step_name);
                }
                if let Some(ref cols) = input_schema {
                    if cols.iter().any(|c| c == col_name) {
                        self.column_already_exists(col_name, step_span.clone());
                    }
                    for (col, _) in by {
                        if !cols.iter().any(|c| c == col) {
                            self.unknown_column(col, step_span.clone());
                        }
                    }
                }
            }

            StepKind::TableMax { input, col_name }
            | StepKind::TableMin { input, col_name } => {
                self.validate_step(input, &[col_name.as_str()], &[], step_name, step_span, scope, schema_ref);
            }

            StepKind::TableMaxN { input, col_name, .. }
            | StepKind::TableMinN { input, col_name, .. } => {
                self.validate_step(input, &[col_name.as_str()], &[], step_name, step_span, scope, schema_ref);
            }

            // ── Value operations ──────────────────────────────────────────
            StepKind::ReplaceValue { input, old_value, new_value, replacer } => {
                self.validate_step(input, &[], &[old_value, new_value, replacer], step_name, step_span, scope, schema_ref);
            }

            StepKind::ReplaceErrorValues { input, replacements } => {
                let col_names: Vec<&str> = replacements.iter().map(|(n, _, _)| n.as_str()).collect();
                let exprs: Vec<&ExprNode> = replacements.iter().map(|(_, e, _)| e).collect();
                self.validate_step(input, &col_names, &exprs, step_name, step_span, scope, schema_ref);
            }

            StepKind::InsertRows { input, .. } => {
                self.validate_step(input, &[], &[], step_name, step_span, scope, schema_ref);
            }

            StepKind::Passthrough { input, .. } => {
                self.validate_step(input, &[], &[], step_name, step_span, scope, schema_ref);
            }

            // List.Generate: validate each lambda expression against the source
            // table schema so column references like `[A]` are caught.
            StepKind::ListGenerate { initial, condition, next, selector } => {
                let table_schema: Vec<String> = self
                    .table
                    .column_names()
                    .iter()
                    .map(|s| s.to_string())
                    .collect();
                let gen_schema: Option<&[String]> = Some(&table_schema);
                self.resolve_expr(initial,   None);           // () => seed — no col context
                self.resolve_expr(condition, gen_schema);     // each _ < [Col]
                self.resolve_expr(next,      gen_schema);     // each _ + 1
                if let Some(sel) = selector {
                    self.resolve_expr(sel, gen_schema);
                }
            }

            StepKind::ListTransform { list_expr, transform } => {
                match &list_expr.expr {
                    pq_ast::expr::Expr::Identifier(name)
                        if !scope.contains(name.as_str()) =>
                    {
                        self.unknown_step(name, step_span.clone(), scope, step_name);
                    }
                    _ => {
                        self.resolve_expr(list_expr, schema_ref);
                    }
                }
                self.resolve_expr(transform, None);
            }
        }
    }

    // ── public entry point ────────────────────────────────────────────────

    pub fn resolve(&mut self, program: &Program) -> ResolveResult {
        let mut scope = Scope::new();

        for binding in &program.steps {
            self.resolve_step(
                &binding.name,
                &binding.step.span,
                &binding.step.kind,
                &scope,
            );
            // Register the output schema before defining the name in scope so
            // that downstream steps can look it up via `schema_of`.
            self.register_output_schema(&binding.name, &binding.step.kind);
            scope.define(binding.name.clone(), binding.name_span.clone());
        }

        if !scope.contains(&program.output) {
            self.unknown_output_step(
                &program.output,
                program.output_span.clone(),
                &scope,
            );
        }

        if self.diagnostics.is_empty() {
            Ok(())
        } else {
            Err(std::mem::take(&mut self.diagnostics))
        }
    }
}

// ── helpers ───────────────────────────────────────────────────────────────

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let m = a.len();
    let n = b.len();
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for i in 0..=m { dp[i][0] = i; }
    for j in 0..=n { dp[0][j] = j; }
    for i in 1..=m {
        for j in 1..=n {
            dp[i][j] = if a[i-1] == b[j-1] {
                dp[i-1][j-1]
            } else {
                1 + dp[i-1][j].min(dp[i][j-1]).min(dp[i-1][j-1])
            };
        }
    }
    dp[m][n]
}

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

    #[test]
    fn test_valid_program() {
        let table   = make_table();
        let program = parse(r#"
            let
                Source          = Excel.Workbook(File.Contents("test.xlsx"), null, true),
                PromotedHeaders = Table.PromoteHeaders(Source),
                ChangedTypes    = Table.TransformColumnTypes(PromotedHeaders, {{"Name", Text.Type}, {"Age", Int64.Type}})
            in
                ChangedTypes
        "#);
        let mut resolver = Resolver::new(&table);
        assert!(resolver.resolve(&program).is_ok());
    }

    #[test]
    fn test_unknown_step_reference() {
        let table   = make_table();
        let program = parse(r#"
            let
                Source   = Excel.Workbook(File.Contents("test.xlsx"), null, true),
                Filtered = Table.SelectRows(DoesNotExist, each Age > 25)
            in
                Filtered
        "#);
        let mut resolver = Resolver::new(&table);
        assert!(resolver.resolve(&program).is_err());
    }

    #[test]
    fn test_unknown_column_in_filter() {
        let table   = make_table();
        let program = parse(r#"
            let
                Source   = Excel.Workbook(File.Contents("test.xlsx"), null, true),
                Filtered = Table.SelectRows(Source, each BadColumn > 25)
            in
                Filtered
        "#);
        let mut resolver = Resolver::new(&table);
        assert!(resolver.resolve(&program).is_err());
    }

    #[test]
    fn test_bracket_column_access_valid() {
        let table   = make_table();
        let program = parse(r#"
            let
                Source   = Excel.Workbook(File.Contents("test.xlsx"), null, true),
                Filtered = Table.SelectRows(Source, each [Age] > 25)
            in
                Filtered
        "#);
        let mut resolver = Resolver::new(&table);
        assert!(resolver.resolve(&program).is_ok());
    }

    #[test]
    fn test_filter_with_and() {
        let table   = make_table();
        let program = parse(r#"
            let
                Source   = Excel.Workbook(File.Contents("test.xlsx"), null, true),
                Filtered = Table.SelectRows(Source, each Age > 25 and Active = true)
            in
                Filtered
        "#);
        let mut resolver = Resolver::new(&table);
        assert!(resolver.resolve(&program).is_ok());
    }

    #[test]
    fn test_column_already_exists() {
        let table   = make_table();
        let program = parse(r#"
            let
                Source    = Excel.Workbook(File.Contents("test.xlsx"), null, true),
                WithExtra = Table.AddColumn(Source, "Age", each Salary + 1000.0)
            in
                WithExtra
        "#);
        let mut resolver = Resolver::new(&table);
        assert!(resolver.resolve(&program).is_err());
    }

    #[test]
    fn test_unknown_output_step() {
        let table   = make_table();
        let program = parse(r#"
            let
                Source = Excel.Workbook(File.Contents("test.xlsx"), null, true)
            in
                DoesNotExist
        "#);
        let mut resolver = Resolver::new(&table);
        assert!(resolver.resolve(&program).is_err());
    }
}

