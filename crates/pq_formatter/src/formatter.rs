use pq_ast::{
    Program,
    expr::{Expr, ExprNode},
    step::{StepKind, SortOrder, MissingFieldKind, JoinKind},
    call_arg::CallArg,
};
use pq_grammar::operators::UnaryOp;
use pq_pipeline::Table;
use pq_types::ColumnType;

// ── expression formatting ─────────────────────────────────────────────────

/// Render an expression as clean M-like syntax.
pub fn format_expr(node: &ExprNode) -> String {
    match &node.expr {
        // ── literals ──────────────────────────────────────────────────────
        Expr::IntLit(n)       => n.to_string(),
        Expr::FloatLit(n)     => {
            if n.fract() == 0.0 { format!("{:.1}", n) } else { n.to_string() }
        }
        Expr::BoolLit(b)      => b.to_string(),
        Expr::StringLit(s)    => format!("\"{}\"", s),
        Expr::NullLit         => "null".into(),

        // ── identifiers ───────────────────────────────────────────────────
        Expr::Identifier(name)    => name.clone(),
        Expr::ColumnAccess(name)  => format!("[{}]", name),        Expr::FieldAccess { record, field } => format!("{}[{}]", format_expr(record), field),
        // ── compound ──────────────────────────────────────────────────────
        Expr::BinaryOp { left, op, right } => {
            format!("{} {} {}", format_expr(left), op, format_expr(right))
        }
        Expr::UnaryOp { op, operand } => {
            match op {
                UnaryOp::Not => format!("not {}", format_expr(operand)),
                UnaryOp::Neg => format!("-{}", format_expr(operand)),
            }
        }

        // ── higher-order ──────────────────────────────────────────────────
        Expr::FunctionCall { name, args } => {
            let args_str = args.iter().map(format_expr).collect::<Vec<_>>().join(", ");
            format!("{}({})", name, args_str)
        }
        // `each` is stored as Lambda { params: ["_"], .. }; round-trip as `each <body>`.
        // Zero-param lambda: `() => body`.
        // Multi-param lambda: `(a, b) => body`.
        Expr::Lambda { params, body } => {
            if params == &["_"] {
                format!("each {}", format_expr(body))
            } else if params.is_empty() {
                format!("() => {}", format_expr(body))
            } else {
                format!("({}) => {}", params.join(", "), format_expr(body))
            }
        }

        // ── collections ───────────────────────────────────────────────────
        Expr::List(items) => {
            let inner = items.iter().map(format_expr).collect::<Vec<_>>().join(", ");
            format!("{{{}}}", inner)
        }
        Expr::Record(fields) => {
            let inner = fields
                .iter()
                .map(|(k, v)| format!("{} = {}", k, format_expr(v)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{}]", inner)
        }
    }
}

// ── step formatting ───────────────────────────────────────────────────────

/// Render a single step as a named M-like binding.
fn format_step(name: &str, kind: &StepKind) -> String {
    let body = format_step_body(kind);
    format!("    {} = {}", name, body)
}

/// Render the right-hand side of a step binding.
fn format_step_body(kind: &StepKind) -> String {
    match kind {
        StepKind::Source { path, use_headers, delay_types } => {
            let uh = use_headers.map(|b| if b { ", true" } else { ", false" }).unwrap_or("");
            let dt = delay_types.map(|b| if b { ", true" } else { ", false" }).unwrap_or("");
            format!("Excel.Workbook(File.Contents(\"{}\"){}{})", path, uh, dt)
        }
        StepKind::NavigateSheet { input, item, sheet_kind, field } => {
            format!("{}{{[Item=\"{}\", Kind=\"{}\"]}}\u{005B}{}\u{005D}", input, item, sheet_kind, field)
        }
        StepKind::ValueBinding { expr } => format_expr(expr),
        StepKind::FunctionCall { name, args } => {
            let args_str: Vec<String> = args.iter()
                .filter_map(|a| match a {
                    CallArg::OptInt(None) | CallArg::NullableBool(None)
                    | CallArg::OptMissingField(None) | CallArg::OptCulture(None, _) => None,
                    _ => Some(format_call_arg(a)),
                })
                .collect();
            format!("{}({})", name, args_str.join(", "))
        }
    }
}

fn format_call_arg(arg: &CallArg) -> String {
    match arg {
        CallArg::StepRef(s)       => s.clone(),
        CallArg::StepRefList(v)   => format!("{{{}}}", v.join(", ")),
        CallArg::Expr(e)          => format_expr(e),
        CallArg::Str(s)           => format!("\"{}\"", s),
        CallArg::ColList(v)       => format_col_list(v),
        CallArg::RenameList(v)    => format_rename_list(v),
        CallArg::TypeList(v)      => format_type_list(v),
        CallArg::BareTypeList(v)  => {
            let items = v.iter().map(|t| format!("type {}", t.to_m_type())).collect::<Vec<_>>().join(", ");
            format!("{{{}}}", items)
        }
        CallArg::SortList(v)      => format_sort_list(v),
        CallArg::JoinKindArg(j)   => format!("{}", j),
        CallArg::AggList(v)       => {
            let items = v.iter().map(|a| {
                format!("{{\"{}\" {}, type {}}}", a.name, format_expr(&a.expression), a.col_type.to_m_type())
            }).collect::<Vec<_>>().join(", ");
            format!("{{{}}}", items)
        }
        CallArg::TransformList(v) => {
            let items = v.iter().map(|(n, e, t)| {
                if let Some(ty) = t {
                    format!("{{\"{}\" {} type {}}}", n, format_expr(e), ty.to_m_type())
                } else {
                    format!("{{\"{}\" {}}}", n, format_expr(e))
                }
            }).collect::<Vec<_>>().join(", ");
            format!("{{{}}}", items)
        }
        CallArg::Int(n)               => n.to_string(),
        CallArg::OptInt(n)            => n.map(|i| i.to_string()).unwrap_or_else(|| "null".into()),
        CallArg::NullableBool(b)      => b.map(|b| b.to_string()).unwrap_or_else(|| "null".into()),
        CallArg::OptMissingField(m)   => m.as_ref().map(|mf| match mf {
            MissingFieldKind::Error   => "MissingField.Error".to_string(),
            MissingFieldKind::Ignore  => "MissingField.Ignore".to_string(),
            MissingFieldKind::UseNull => "MissingField.UseNull".to_string(),
        }).unwrap_or_else(|| "null".into()),
        CallArg::OptCulture(s, _)     => s.as_ref().map(|c| format!("\"{}\"", c)).unwrap_or_else(|| "null".into()),
    }
}


// ── list formatters ───────────────────────────────────────────────────────

fn format_type_list(columns: &[(String, ColumnType)]) -> String {
    let pairs = columns
        .iter()
        .map(|(name, t)| format!("{{\"{}\", {}}}", name, t.to_m_type()))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{{}}}", pairs)
}

fn format_col_list(columns: &[String]) -> String {
    let items = columns
        .iter()
        .map(|c| format!("\"{}\"", c))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{{}}}", items)
}

/// Map a `ColumnType` to its bare M type keyword (the part after `type `).
fn col_type_to_m_bare(ty: &pq_types::ColumnType) -> &'static str {
    use pq_types::ColumnType;
    match ty {
        ColumnType::Text         => "text",
        ColumnType::Float        => "number",
        ColumnType::Integer      => "int64",
        ColumnType::Boolean      => "logical",
        ColumnType::Date         => "date",
        ColumnType::DateTime     => "datetime",
        ColumnType::DateTimeZone => "datetimezone",
        ColumnType::Duration     => "duration",
        ColumnType::Time         => "time",
        ColumnType::Currency     => "currency",
        ColumnType::Binary       => "binary",
        ColumnType::Null         => "null",
        ColumnType::Function(_)
        | ColumnType::List(_)    => "any",
    }
}

fn format_rename_list(renames: &[(String, String)]) -> String {
    let pairs = renames
        .iter()
        .map(|(old, new)| format!("{{\"{}\", \"{}\"}}", old, new))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{{}}}", pairs)
}

fn format_sort_list(by: &[(String, SortOrder)]) -> String {
    let pairs = by
        .iter()
        .map(|(col, order)| format!("{{\"{}\", {}}}", col, order))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{{}}}", pairs)
}

// ── program formatting ────────────────────────────────────────────────────

/// Render a full Program as clean M-like syntax.
pub fn format_program(program: &Program) -> String {
    let lines = program
        .steps
        .iter()
        .map(|b| format_step(&b.name, &b.step.kind))
        .collect::<Vec<_>>()
        .join(",\n");
    let output = match &program.output_expr {
        Some(expr) => format_expr(expr),
        None       => program.output.clone(),
    };
    format!("let\n{}\nin\n    {}", lines, output)
}

/// Generate a base M-like formula from a Table (the starting point).
pub fn format_table(table: &Table) -> String {
    let mut lines = vec![];

    lines.push(format!(
        "    Source = Excel.Workbook(File.Contents(\"{}\"), null, true)",
        table.source
    ));
    lines.push("    PromotedHeaders = Table.PromoteHeaders(Source)".into());

    let type_list = format_type_list(
        &table.columns.iter()
            .map(|c| (c.name.clone(), c.col_type.clone()))
            .collect::<Vec<_>>()
    );
    lines.push(format!(
        "    ChangedTypes = Table.TransformColumnTypes(PromotedHeaders, {})",
        type_list
    ));

    format!("let\n{}\nin\n    ChangedTypes", lines.join(",\n"))
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
            source: "workbook.xlsx".into(),
            sheet:  "Sales".into(),
            rows:   vec![
                vec!["Name".into(), "Age".into(), "Salary".into()],
                vec!["Alice".into(), "30".into(), "50000.50".into()],
            ],
        })
    }

    fn parse(input: &str) -> Program {
        let tokens = Lexer::new(input).tokenize().unwrap();
        Parser::new(tokens).parse().unwrap()
    }

    #[test]
    fn test_format_table() {
        let table   = make_table();
        let formula = format_table(&table);
        assert!(formula.contains("Excel.Workbook"));
        assert!(formula.contains("Table.PromoteHeaders"));
        assert!(formula.contains("Table.TransformColumnTypes"));
    }

    #[test]
    fn test_format_program_roundtrip() {
        let input = r#"
            let
                Source   = Excel.Workbook(File.Contents("workbook.xlsx"), null, true),
                Filtered = Table.SelectRows(Source, each Age > 25)
            in
                Filtered
        "#;
        let program   = parse(input);
        let formatted = format_program(&program);
        let reparsed  = parse(&formatted);
        assert_eq!(reparsed.steps.len(), program.steps.len());
        assert_eq!(reparsed.output, program.output);
    }

    #[test]
    fn test_format_each_wraps_correctly() {
        let program   = parse(r#"
            let
                Source   = Excel.Workbook(File.Contents("workbook.xlsx"), null, true),
                Filtered = Table.SelectRows(Source, each Age > 25)
            in Filtered
        "#);
        let formatted = format_program(&program);
        assert!(formatted.contains("each Age > 25"));
    }

    #[test]
    fn test_format_add_column() {
        let program  = parse(r#"
            let
                Source    = Excel.Workbook(File.Contents("workbook.xlsx"), null, true),
                WithBonus = Table.AddColumn(Source, "Bonus", each Salary + 1000.0)
            in
                WithBonus
        "#);
        let formatted = format_program(&program);
        assert!(formatted.contains("Table.AddColumn"));
        assert!(formatted.contains("each Salary + 1000"));
    }

    #[test]
    fn test_format_remove_columns() {
        let program  = parse(r#"
            let
                Source  = Excel.Workbook(File.Contents("workbook.xlsx"), null, true),
                Removed = Table.RemoveColumns(Source, {"Age", "Salary"})
            in
                Removed
        "#);
        let formatted = format_program(&program);
        assert!(formatted.contains("Table.RemoveColumns"));
        assert!(formatted.contains("\"Age\""));
    }

    #[test]
    fn test_format_rename_columns() {
        let program  = parse(r#"
            let
                Source  = Excel.Workbook(File.Contents("workbook.xlsx"), null, true),
                Renamed = Table.RenameColumns(Source, {{"Age", "Years"}})
            in
                Renamed
        "#);
        let formatted = format_program(&program);
        assert!(formatted.contains("Table.RenameColumns"));
        assert!(formatted.contains("\"Years\""));
    }

    #[test]
    fn test_format_sort() {
        let program  = parse(r#"
            let
                Source = Excel.Workbook(File.Contents("workbook.xlsx"), null, true),
                Sorted = Table.Sort(Source, {{"Age", Order.Ascending}})
            in
                Sorted
        "#);
        let formatted = format_program(&program);
        assert!(formatted.contains("Table.Sort"));
        assert!(formatted.contains("Order.Ascending"));
    }

    #[test]
    fn test_format_column_access() {
        let program  = parse(r#"
            let
                Source   = Excel.Workbook(File.Contents("workbook.xlsx"), null, true),
                Filtered = Table.SelectRows(Source, each [Age] > 25)
            in Filtered
        "#);
        let formatted = format_program(&program);
        assert!(formatted.contains("[Age]"));
    }

    #[test]
    fn test_float_always_has_decimal() {
        let program  = parse(r#"
            let
                Source    = Excel.Workbook(File.Contents("workbook.xlsx"), null, true),
                WithBonus = Table.AddColumn(Source, "Bonus", each Salary + 5000.0)
            in
                WithBonus
        "#);
        let formatted = format_program(&program);
        assert!(formatted.contains("5000.0"));
    }
}