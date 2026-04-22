use std::collections::HashMap;
use pq_ast::{
    Program,
    expr::{Expr, ExprNode},
    step::{StepKind, MissingFieldKind},
    call_arg::CallArg,
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
            StepKind::Source { .. } => {
                Some(self.table.column_names().iter().map(|s| s.to_string()).collect())
            }
            StepKind::NavigateSheet { input, .. } => self.schema_of(input),
            StepKind::ValueBinding { .. } => None,
            StepKind::FunctionCall { name, args } => {
                let input_name = args.first().and_then(|a| a.as_step_ref()).unwrap_or("");
                match name.as_str() {
                    "Table.AddColumn" => {
                        let col_name = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                        if let Some(cols) = self.schema_of(input_name) {
                            if cols.iter().any(|c| c == col_name) {
                                self.column_already_exists(col_name, pq_diagnostics::Span::dummy());
                            }
                            Some(cols.into_iter().chain(std::iter::once(col_name.to_string())).collect())
                        } else {
                            None
                        }
                    }
                    "Table.RemoveColumns" => {
                        let cols = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        self.schema_of(input_name).map(|s| s.into_iter().filter(|n| !cols.contains(n)).collect())
                    }
                    "Table.RenameColumns" => {
                        let renames = args.get(1).and_then(|a| a.as_rename_list()).unwrap_or(&[]);
                        self.schema_of(input_name).map(|cols| {
                            cols.into_iter().map(|n| {
                                renames.iter().find(|(old, _)| old == &n)
                                    .map(|(_, new)| new.clone()).unwrap_or(n)
                            }).collect()
                        })
                    }
                    "Table.Group" | "Table.FuzzyGroup" => {
                        let by  = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let agg = args.get(2).and_then(|a| a.as_agg_list()).unwrap_or(&[]);
                        let mut cols: Vec<String> = by.iter().cloned().collect();
                        for a in agg { cols.push(a.name.clone()); }
                        Some(cols)
                    }
                    "Table.AddIndexColumn" => {
                        let col_name = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                        self.schema_of(input_name).map(|mut cols| { cols.push(col_name.to_string()); cols })
                    }
                    "Table.DuplicateColumn" => {
                        let new_col = args.get(2).and_then(|a| a.as_str()).unwrap_or("");
                        self.schema_of(input_name).map(|mut cols| { cols.push(new_col.to_string()); cols })
                    }
                    "Table.Unpivot" => {
                        let columns  = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let attr_col = args.get(2).and_then(|a| a.as_str()).unwrap_or("Attribute");
                        let val_col  = args.get(3).and_then(|a| a.as_str()).unwrap_or("Value");
                        self.schema_of(input_name).map(|cols| {
                            let mut out: Vec<String> = cols.into_iter().filter(|n| !columns.contains(n)).collect();
                            out.push(attr_col.to_string()); out.push(val_col.to_string()); out
                        })
                    }
                    "Table.UnpivotOtherColumns" => {
                        let keep = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let attr = args.get(2).and_then(|a| a.as_str()).unwrap_or("Attribute");
                        let val  = args.get(3).and_then(|a| a.as_str()).unwrap_or("Value");
                        let mut out: Vec<String> = keep.iter().cloned().collect();
                        out.push(attr.to_string()); out.push(val.to_string()); Some(out)
                    }
                    "Table.PrefixColumns" => {
                        let prefix = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                        self.schema_of(input_name).map(|cols| cols.into_iter().map(|n| format!("{}.{}", prefix, n)).collect())
                    }
                    "Table.SelectColumns" => {
                        let columns = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        Some(columns.iter().cloned().collect())
                    }
                    "Table.ReorderColumns" => {
                        let columns = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        self.schema_of(input_name).map(|all_cols| {
                            let mut out: Vec<String> = columns.iter().filter(|c| all_cols.contains(*c)).cloned().collect();
                            for c in all_cols { if !columns.contains(&c) { out.push(c); } }
                            out
                        })
                    }
                    "Table.CombineColumns" => {
                        let columns = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let new_col = args.get(3).and_then(|a| a.as_str()).unwrap_or("Combined");
                        self.schema_of(input_name).map(|cols| {
                            let mut out: Vec<String> = cols.into_iter().filter(|n| !columns.contains(n)).collect();
                            out.push(new_col.to_string()); out
                        })
                    }
                    "Table.SplitColumn" => {
                        let col_name = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                        self.schema_of(input_name).map(|cols| {
                            let mut out: Vec<String> = cols.into_iter().filter(|n| n != col_name).collect();
                            out.push(format!("{}.1", col_name)); out.push(format!("{}.2", col_name)); out
                        })
                    }
                    "Table.ExpandTableColumn" | "Table.ExpandRecordColumn" => {
                        let col_name = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                        let columns  = args.get(2).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        self.schema_of(input_name).map(|cols| {
                            let mut out: Vec<String> = cols.into_iter().filter(|n| n != col_name).collect();
                            out.extend(columns.iter().cloned()); out
                        })
                    }
                    "Table.Join" | "Table.FuzzyJoin" => {
                        let right_name = args.get(2).and_then(|a| a.as_step_ref()).unwrap_or("");
                        let left  = self.schema_of(input_name).unwrap_or_default();
                        let right = self.schema_of(right_name).unwrap_or_default();
                        let mut out = left.clone();
                        for c in right { if !left.contains(&c) { out.push(c); } }
                        Some(out)
                    }
                    "Table.NestedJoin" | "Table.FuzzyNestedJoin" => {
                        let new_col = args.get(4).and_then(|a| a.as_str()).unwrap_or("NewColumn");
                        self.schema_of(input_name).map(|mut cols| { cols.push(new_col.to_string()); cols })
                    }
                    "Table.AddRankColumn" => {
                        let col_name = args.get(1).and_then(|a| a.as_str()).unwrap_or("Rank");
                        self.schema_of(input_name).map(|mut cols| { cols.push(col_name.to_string()); cols })
                    }
                    "Table.RowCount" | "Table.ColumnCount" | "Table.IsEmpty" | "Table.IsDistinct"
                    | "Table.HasColumns" | "Table.ColumnNames" | "Table.ColumnsOfType"
                    | "Table.MatchesAllRows" | "Table.MatchesAnyRows"
                    | "List.Generate" | "List.Select" | "List.Transform" | "List.RemoveItems" | "List.Difference" | "List.Intersect" | "List.Contains" => {
                        Some(vec!["Value".to_string()])
                    }
                    "Table.Schema" => {
                        Some(vec!["Name".to_string(), "Kind".to_string(), "IsNullable".to_string()])
                    }
                    "Table.Combine" => {
                        let inputs = args.first().and_then(|a| a.as_step_ref_list()).unwrap_or(&[]);
                        inputs.first().and_then(|s| self.schema_of(s))
                    }
                    "Table.Transpose" | "Table.Pivot" | "Table.TransformColumnNames"
                    | "Table.FromColumns" | "Table.FromList" | "Table.FromRecords"
                    | "Table.FromRows" | "Table.FromValue"
                    | "Table.ToColumns" | "Table.ToList" | "Table.ToRecords" | "Table.ToRows"
                    | "Table.PartitionValues" | "Table.Profile" | "Table.Column" => None,
                    _ => self.schema_of(input_name),
                }
            }
        };
        if let Some(cols) = output {
            self.step_schemas.insert(step_name.to_string(), cols);
        }
    }

    // ── step validation helper ────────────────────────────────────────────────

    /// Encapsulates the three-part pattern repeated in every step arm:
    /// 1. Verify `input` is a known step in `scope`.
    /// 2. Verify every name in `col_names` exists in `schema_ref`.
    /// 3. Recursively resolve every expression in `exprs`.
    ///
    /// `input` may be empty (e.g. `Passthrough` with no predecessor) -- the
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

    // ── step resolver ─────────────────────────────────────────────────────────

    fn resolve_step(
        &mut self,
        step_name: &str,
        step_span: &Span,
        kind:      &StepKind,
        scope:     &Scope,
    ) {
        match kind {
            StepKind::Source { .. } => {
                self.register_output_schema(step_name, kind);
            }
            StepKind::NavigateSheet { input, .. } => {
                if !scope.contains(input.as_str()) {
                    self.unknown_step(input, step_span.clone(), scope, step_name);
                }
                self.register_output_schema(step_name, kind);
            }
            StepKind::ValueBinding { expr } => {
                self.resolve_expr(expr, None);
                self.register_output_schema(step_name, kind);
            }
            StepKind::FunctionCall { name, args } => {
                // Extract primary input step reference (args[0])
                let input_name = args.first().and_then(|a| a.as_step_ref()).unwrap_or("");

                // Validate primary input step exists
                if !input_name.is_empty() && !scope.contains(input_name) {
                    self.unknown_step(input_name, step_span.clone(), scope, step_name);
                }

                // For joins, also validate the secondary table reference (args[2])
                match name.as_str() {
                    "Table.Join" | "Table.NestedJoin" | "Table.FuzzyJoin" | "Table.FuzzyNestedJoin" => {
                        if let Some(right) = args.get(2).and_then(|a| a.as_step_ref()) {
                            if !scope.contains(right) {
                                self.unknown_step(right, step_span.clone(), scope, step_name);
                            }
                        }
                    }
                    "Table.Combine" => {
                        if let Some(inputs) = args.first().and_then(|a| a.as_step_ref_list()) {
                            for inp in inputs {
                                if !scope.contains(inp.as_str()) {
                                    self.unknown_step(inp, step_span.clone(), scope, step_name);
                                }
                            }
                        }
                    }
                    _ => {}
                }

                // Get input schema for column validation
                let input_schema = if !input_name.is_empty() {
                    self.schema_of(input_name)
                } else {
                    None
                };
                let schema_ref = input_schema.as_deref();

                // Validate column list args against input schema
                for arg in args.iter() {
                    if let CallArg::ColList(cols) = arg {
                        if let Some(sch) = schema_ref {
                            for col in cols {
                                if !sch.iter().any(|c| c == col) {
                                    self.unknown_column(col, step_span.clone());
                                }
                            }
                        }
                    }
                }

                // Resolve expression args against input schema
                for arg in args.iter() {
                    match arg {
                        CallArg::Expr(expr) => {
                            self.resolve_expr(expr, schema_ref);
                        }
                        CallArg::AggList(aggs) => {
                            for agg in aggs {
                                self.resolve_expr(&agg.expression, schema_ref);
                            }
                        }
                        CallArg::TransformList(transforms) => {
                            for (_, expr, _) in transforms {
                                self.resolve_expr(expr, schema_ref);
                            }
                        }
                        _ => {}
                    }
                }

                // Register this step's output schema for downstream steps
                self.register_output_schema(step_name, kind);
            }
        }
    }
    // ── public entry point ────────────────────────────────────────────────────

    pub fn resolve(&mut self, program: &Program) -> ResolveResult {
        let mut scope = Scope::new();
        for binding in &program.steps {
            self.resolve_step(
                &binding.name,
                &binding.step.span,
                &binding.step.kind,
                &scope,
            );
            scope.define(binding.name.clone(), binding.step.span.clone());
        }
        // Validate the output step is in scope.
        // When the `in` clause is a full expression (output_expr is Some),
        // there is no single step name to validate — resolve the expression
        // instead so that any identifier references inside it are checked.
        if let Some(expr) = &program.output_expr {
            self.resolve_expr(expr, None);
        } else if !program.output.is_empty() && !scope.contains(&program.output) {
            let dummy_span = pq_diagnostics::Span::dummy();
            self.unknown_step(&program.output, dummy_span, &scope, "output");
        }
        if self.diagnostics.is_empty() {
            Ok(())
        } else {
            Err(std::mem::take(&mut self.diagnostics))
        }
    }}


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
