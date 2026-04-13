use std::collections::HashMap;

use pq_ast::{
    Program,
    expr::{Expr, ExprNode},
    step::{StepKind, SortOrder},
};
use pq_grammar::operators::Operator;
use pq_pipeline::Table;
use pq_types::ColumnType;

// ── public entry point ────────────────────────────────────────────────────

/// Convert a typed `Program` into a SQL query string.
///
/// Every M step becomes a CTE.  The final output is:
///
/// ```sql
/// WITH
///   Source          AS (SELECT * FROM "sheet"),
///   PromotedHeaders AS (SELECT * FROM Source),
///   ...
/// SELECT * FROM <output_step>
/// [ORDER BY ...]
/// ```
pub fn generate_sql(program: &Program, table: &Table) -> String {
    Generator::new(table).run(program)
}

// ── internal ──────────────────────────────────────────────────────────────

struct Generator {
    /// schema after each step: step_name -> [(col_name, col_type)]
    schemas: HashMap<String, Vec<(String, ColumnType)>>,
    /// initial column schema from the source Table
    initial: Vec<(String, ColumnType)>,
}

impl Generator {
    fn new(table: &Table) -> Self {
        let initial = table.columns.iter()
            .map(|c| (c.name.clone(), c.col_type.clone()))
            .collect();
        Generator { schemas: HashMap::new(), initial }
    }

    fn run(&mut self, program: &Program) -> String {
        let mut ctes:     Vec<String>              = Vec::new();
        let mut order_by: Vec<(String, SortOrder)> = Vec::new();

        for binding in &program.steps {
            // what schema does the input step expose?
            let in_name = step_input(&binding.step.kind);
            let schema  = self.schema_of(in_name);

            let (body, out_schema) =
                self.emit(&binding.step.kind, &schema, &mut order_by);

            ctes.push(format!("  {} AS (\n{}\n  )", binding.name, body));
            self.schemas.insert(binding.name.clone(), out_schema);
        }

        // final SELECT — ORDER BY lives here, not inside CTEs
        let mut sql = format!("SELECT *\nFROM {}", program.output);
        if !order_by.is_empty() {
            let ob = order_by.iter()
                .map(|(col, ord)| format!("{} {}", qi(col), sort_dir(ord)))
                .collect::<Vec<_>>()
                .join(", ");
            sql.push_str(&format!("\nORDER BY {}", ob));
        }

        if ctes.is_empty() {
            return sql;
        }
        format!("WITH\n{}\n{}", ctes.join(",\n"), sql)
    }

    // look up the schema for a step name (empty string → initial schema)
    fn schema_of(&self, step_name: &str) -> Vec<(String, ColumnType)> {
        if step_name.is_empty() {
            return self.initial.clone();
        }
        self.schemas
            .get(step_name)
            .cloned()
            .unwrap_or_else(|| self.initial.clone())
    }

    // ── step → (CTE body SQL, output schema) ─────────────────────────────

    fn emit(
        &self,
        kind:     &StepKind,
        schema:   &[(String, ColumnType)],
        order_by: &mut Vec<(String, SortOrder)>,
    ) -> (String, Vec<(String, ColumnType)>) {
        match kind {

            // Source ──────────────────────────────────────────────────────
            // Excel.Workbook(path, sheet)  →  SELECT * FROM "sheet"
            StepKind::Source { sheet, .. } => {
                let body = format!("    SELECT *\n    FROM {}", qi(sheet));
                (body, schema.to_vec())
            }

            // PromoteHeaders ──────────────────────────────────────────────
            // already done by build_table; pass through in SQL
            StepKind::PromoteHeaders { input } => {
                let body = format!("    SELECT * FROM {}", input);
                (body, schema.to_vec())
            }

            // ChangeTypes ─────────────────────────────────────────────────
            // CAST every listed column; pass others through unchanged
            StepKind::ChangeTypes { input, columns } => {
                let select = schema.iter()
                    .map(|(name, cur_type)| {
                        if let Some((_, new_type)) = columns.iter().find(|(n, _)| n == name) {
                            format!(
                                "        CAST({} AS {}) AS {}",
                                qi(name), sql_type(new_type), qi(name)
                            )
                        } else {
                            format!("        {}", qi(name))
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(",\n");

                let body = format!("    SELECT\n{}\n    FROM {}", select, input);

                let new_schema = schema.iter()
                    .map(|(name, t)| {
                        let nt = columns.iter()
                            .find(|(n, _)| n == name)
                            .map(|(_, nt)| nt.clone())
                            .unwrap_or_else(|| t.clone());
                        (name.clone(), nt)
                    })
                    .collect();

                (body, new_schema)
            }

            // Filter ──────────────────────────────────────────────────────
            // Table.SelectRows(input, each cond)  →  WHERE cond
            StepKind::Filter { input, condition } => {
                let body = format!(
                    "    SELECT *\n    FROM {}\n    WHERE {}",
                    input,
                    emit_expr(condition)
                );
                (body, schema.to_vec())
            }

            // AddColumn ───────────────────────────────────────────────────
            // Table.AddColumn(input, "col", each expr)  →  SELECT *, expr AS "col"
            StepKind::AddColumn { input, col_name, expression } => {
                let expr_sql  = emit_expr(expression);
                let body = format!(
                    "    SELECT\n        *,\n        {} AS {}\n    FROM {}",
                    expr_sql, qi(col_name), input
                );
                let inferred = infer_type(expression, schema);
                let mut new_schema = schema.to_vec();
                new_schema.push((col_name.clone(), inferred));
                (body, new_schema)
            }

            // RemoveColumns ───────────────────────────────────────────────
            // enumerate every surviving column explicitly
            StepKind::RemoveColumns { input, columns } => {
                let kept: Vec<&(String, ColumnType)> = schema.iter()
                    .filter(|(name, _)| !columns.contains(name))
                    .collect();

                let select = kept.iter()
                    .map(|(name, _)| format!("        {}", qi(name)))
                    .collect::<Vec<_>>()
                    .join(",\n");

                let body = format!("    SELECT\n{}\n    FROM {}", select, input);
                let new_schema = kept.iter().map(|(n, t)| (n.clone(), t.clone())).collect();
                (body, new_schema)
            }

            // RenameColumns ───────────────────────────────────────────────
            // col AS "new_name" for renamed ones; bare name for the rest
            StepKind::RenameColumns { input, renames } => {
                let select = schema.iter()
                    .map(|(name, _)| {
                        if let Some((_, new)) = renames.iter().find(|(old, _)| old == name) {
                            format!("        {} AS {}", qi(name), qi(new))
                        } else {
                            format!("        {}", qi(name))
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(",\n");

                let body = format!("    SELECT\n{}\n    FROM {}", select, input);

                let new_schema = schema.iter()
                    .map(|(name, t)| {
                        let new_name = renames.iter()
                            .find(|(old, _)| old == name)
                            .map(|(_, new)| new.clone())
                            .unwrap_or_else(|| name.clone());
                        (new_name, t.clone())
                    })
                    .collect();

                (body, new_schema)
            }

            // Sort ────────────────────────────────────────────────────────
            // ORDER BY is collected and appended to the final SELECT
            StepKind::Sort { input, by } => {
                *order_by = by.clone();
                let body = format!("    SELECT * FROM {}", input);
                (body, schema.to_vec())
            }
        }
    }
}

// ── expression → SQL ─────────────────────────────────────────────────────

fn emit_expr(node: &ExprNode) -> String {
    match &node.expr {
        Expr::Column(name)    => qi(name),
        Expr::IntLit(n)       => n.to_string(),
        Expr::FloatLit(n)     => n.to_string(),
        Expr::BoolLit(true)   => "TRUE".into(),
        Expr::BoolLit(false)  => "FALSE".into(),
        Expr::StringLit(s)    => format!("'{}'", s.replace('\'', "''")),
        Expr::BinaryOp { left, op, right } => format!(
            "({} {} {})",
            emit_expr(left),
            op_sql(op),
            emit_expr(right)
        ),
    }
}

fn op_sql(op: &Operator) -> &'static str {
    match op {
        Operator::Eq    => "=",
        Operator::NotEq => "<>",
        Operator::Gt    => ">",
        Operator::Lt    => "<",
        Operator::GtEq  => ">=",
        Operator::LtEq  => "<=",
        Operator::Add   => "+",
        Operator::Sub   => "-",
        Operator::Mul   => "*",
        Operator::Div   => "/",
    }
}

// ── type inference for AddColumn ─────────────────────────────────────────

fn infer_type(node: &ExprNode, schema: &[(String, ColumnType)]) -> ColumnType {
    match &node.expr {
        Expr::IntLit(_)    => ColumnType::Integer,
        Expr::FloatLit(_)  => ColumnType::Float,
        Expr::BoolLit(_)   => ColumnType::Boolean,
        Expr::StringLit(_) => ColumnType::Text,
        Expr::Column(name) => schema.iter()
            .find(|(n, _)| n == name)
            .map(|(_, t)| t.clone())
            .unwrap_or(ColumnType::Text),
        Expr::BinaryOp { left, op, right } => {
            if op.is_comparison() {
                return ColumnType::Boolean;
            }
            match (infer_type(left, schema), infer_type(right, schema)) {
                (ColumnType::Float, _) | (_, ColumnType::Float) => ColumnType::Float,
                (ColumnType::Integer, ColumnType::Integer)       => ColumnType::Integer,
                _                                                 => ColumnType::Text,
            }
        }
    }
}

// ── small helpers ─────────────────────────────────────────────────────────

/// Double-quote a SQL identifier.
fn qi(name: &str) -> String {
    format!("\"{}\"", name)
}

fn sql_type(t: &ColumnType) -> &'static str {
    match t {
        ColumnType::Integer => "BIGINT",
        ColumnType::Float   => "FLOAT",
        ColumnType::Boolean => "BOOLEAN",
        ColumnType::Text    => "TEXT",
        ColumnType::Date    => "DATE",
        ColumnType::Null    => "NULL",
    }
}

fn sort_dir(order: &SortOrder) -> &'static str {
    match order {
        SortOrder::Ascending  => "ASC",
        SortOrder::Descending => "DESC",
    }
}

/// Return the name of the input step for any StepKind.
fn step_input(kind: &StepKind) -> &str {
    match kind {
        StepKind::Source { .. }               => "",
        StepKind::PromoteHeaders  { input }
        | StepKind::ChangeTypes   { input, .. }
        | StepKind::Filter        { input, .. }
        | StepKind::AddColumn     { input, .. }
        | StepKind::RemoveColumns { input, .. }
        | StepKind::RenameColumns { input, .. }
        | StepKind::Sort          { input, .. } => input.as_str(),
    }
}

// ── tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use pq_lexer::Lexer;
    use pq_parser::Parser;
    use pq_pipeline::{build_table, RawWorkbook};

    fn table() -> Table {
        build_table(RawWorkbook {
            source: "workbook.xlsx".into(),
            sheet:  "Sales".into(),
            rows:   vec![
                vec!["Name".into(), "Age".into(), "Salary".into(), "Active".into()],
                vec!["Alice".into(), "30".into(), "50000.50".into(), "true".into()],
                vec!["Bob".into(),   "25".into(), "40000.00".into(), "false".into()],
            ],
        })
    }

    fn parse_and_gen(formula: &str) -> String {
        let t      = table();
        let tokens = Lexer::new(formula).tokenize().unwrap();
        let prog   = Parser::new(tokens).parse().unwrap();
        generate_sql(&prog, &t)
    }

    #[test]
    fn test_source_only() {
        let sql = parse_and_gen(
            r#"let Source = Excel.Workbook("workbook.xlsx", "Sales") in Source"#
        );
        assert!(sql.contains("SELECT * FROM \"Sales\""));
    }

    #[test]
    fn test_filter() {
        let sql = parse_and_gen(r#"
            let
                Source   = Excel.Workbook("workbook.xlsx", "Sales"),
                Filtered = Table.SelectRows(Source, each Age > 25)
            in Filtered
        "#);
        assert!(sql.contains("WHERE (\"Age\" > 25)"));
    }

    #[test]
    fn test_add_column() {
        let sql = parse_and_gen(r#"
            let
                Source    = Excel.Workbook("workbook.xlsx", "Sales"),
                WithBonus = Table.AddColumn(Source, "Bonus", each Salary + 1000.0)
            in WithBonus
        "#);
        assert!(sql.contains("AS \"Bonus\""));
        assert!(sql.contains("\"Salary\""));
    }

    #[test]
    fn test_remove_columns() {
        let sql = parse_and_gen(r#"
            let
                Source  = Excel.Workbook("workbook.xlsx", "Sales"),
                Removed = Table.RemoveColumns(Source, {"Active"})
            in Removed
        "#);
        assert!(!sql.contains("\"Active\""));
        assert!(sql.contains("\"Name\""));
    }

    #[test]
    fn test_rename_columns() {
        let sql = parse_and_gen(r#"
            let
                Source  = Excel.Workbook("workbook.xlsx", "Sales"),
                Renamed = Table.RenameColumns(Source, {{"Name", "FullName"}})
            in Renamed
        "#);
        assert!(sql.contains("\"Name\" AS \"FullName\""));
    }

    #[test]
    fn test_sort() {
        let sql = parse_and_gen(r#"
            let
                Source = Excel.Workbook("workbook.xlsx", "Sales"),
                Sorted = Table.Sort(Source, {{"Age", Order.Ascending}})
            in Sorted
        "#);
        assert!(sql.contains("ORDER BY \"Age\" ASC"));
    }

    #[test]
    fn test_full_pipeline() {
        let sql = parse_and_gen(r#"
            let
                Source          = Excel.Workbook("workbook.xlsx", "Sales"),
                PromotedHeaders = Table.PromoteHeaders(Source),
                ChangedTypes    = Table.TransformColumnTypes(PromotedHeaders, {{"Age", Int64.Type}, {"Salary", Number.Type}}),
                Filtered        = Table.SelectRows(ChangedTypes, each Age > 25),
                WithBonus       = Table.AddColumn(Filtered, "Bonus", each Salary + 1000.0),
                Removed         = Table.RemoveColumns(WithBonus, {"Active"}),
                Renamed         = Table.RenameColumns(Removed, {{"Name", "FullName"}}),
                Sorted          = Table.Sort(Renamed, {{"Age", Order.Ascending}})
            in Sorted
        "#);
        assert!(sql.contains("WITH"));
        assert!(sql.contains("CAST(\"Age\" AS BIGINT)"));
        assert!(sql.contains("WHERE (\"Age\" > 25)"));
        assert!(sql.contains("AS \"Bonus\""));
        assert!(sql.contains("ORDER BY \"Age\" ASC"));
        assert!(sql.contains("\"Name\" AS \"FullName\""));
        assert!(!sql.contains("\"Active\""));
    }
}

