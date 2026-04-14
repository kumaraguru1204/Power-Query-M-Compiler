use pq_ast::{
    Program,
    expr::{Expr, ExprNode},
    step::{StepKind, SortOrder},
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
            let uh = match use_headers {
                None        => "null",
                Some(true)  => "true",
                Some(false) => "false",
            };
            let dt = match delay_types {
                None        => "null",
                Some(true)  => "true",
                Some(false) => "false",
            };
            format!("Excel.Workbook(File.Contents(\"{}\"), {}, {})", path, uh, dt)
        }

        StepKind::PromoteHeaders { input } => {
            format!("Table.PromoteHeaders({})", input)
        }

        StepKind::ChangeTypes { input, columns } => {
            let type_list = format_type_list(columns);
            format!("Table.TransformColumnTypes({}, {})", input, type_list)
        }

        // condition is Each(inner) — format_expr produces "each <inner>"
        StepKind::Filter { input, condition } => {
            format!("Table.SelectRows({}, {})", input, format_expr(condition))
        }

        // expression is Each(inner) — format_expr produces "each <inner>"
        StepKind::AddColumn { input, col_name, expression } => {
            format!(
                "Table.AddColumn({}, \"{}\", {})",
                input, col_name, format_expr(expression)
            )
        }

        StepKind::RemoveColumns { input, columns } => {
            let col_list = format_col_list(columns);
            format!("Table.RemoveColumns({}, {})", input, col_list)
        }

        StepKind::RenameColumns { input, renames } => {
            let rename_list = format_rename_list(renames);
            format!("Table.RenameColumns({}, {})", input, rename_list)
        }

        StepKind::Sort { input, by } => {
            let sort_list = format_sort_list(by);
            format!("Table.Sort({}, {})", input, sort_list)
        }

        // Each transform expression is Each(inner) — format_expr handles it.
        StepKind::TransformColumns { input, transforms } => {
            let pairs = transforms
                .iter()
                .map(|(col, expr, opt_type)| {
                    if let Some(t) = opt_type {
                        format!("{{\"{}\", {}, {}}}", col, format_expr(expr), t.to_m_type())
                    } else {
                        format!("{{\"{}\", {}}}", col, format_expr(expr))
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("Table.TransformColumns({}, {{{}}})", input, pairs)
        }

        // Aggregate expressions are Each(inner) — format_expr handles them.
        StepKind::Group { input, by, aggregates } => {
            let key_list = format_col_list(by);
            let agg_list = aggregates
                .iter()
                .map(|a| format!(
                    "{{\"{}\", {}, {}}}",
                    a.name,
                    format_expr(&a.expression),
                    a.col_type.to_m_type()
                ))
                .collect::<Vec<_>>()
                .join(", ");
            format!("Table.Group({}, {}, {{{}}})", input, key_list, agg_list)
        }

        StepKind::ListGenerate { initial, condition, next, selector } => {
            let mut s = format!(
                "List.Generate({}, {}, {}",
                format_expr(initial),
                format_expr(condition),
                format_expr(next),
            );
            if let Some(sel) = selector {
                s.push_str(", ");
                s.push_str(&format_expr(sel));
            }
            s.push(')');
            s
        }

        StepKind::Passthrough { input, func_name } => {
            format!("{}({})", func_name, input)
        }

        StepKind::ListTransform { list_expr, transform } => {
            format!(
                "List.Transform({}, {})",
                format_expr(list_expr),
                format_expr(transform)
            )
        }

        // ── New row operations ────────────────────────────────────────────

        StepKind::FirstN { input, count } => {
            format!("Table.FirstN({}, {})", input, format_expr(count))
        }

        StepKind::LastN { input, count } => {
            format!("Table.LastN({}, {})", input, format_expr(count))
        }

        StepKind::Skip { input, count } => {
            format!("Table.Skip({}, {})", input, format_expr(count))
        }

        StepKind::Range { input, offset, count } => {
            format!("Table.Range({}, {}, {})", input, format_expr(offset), format_expr(count))
        }

        StepKind::RemoveFirstN { input, count } => {
            format!("Table.RemoveFirstN({}, {})", input, format_expr(count))
        }

        StepKind::RemoveLastN { input, count } => {
            format!("Table.RemoveLastN({}, {})", input, format_expr(count))
        }

        StepKind::RemoveRows { input, offset, count } => {
            format!("Table.RemoveRows({}, {}, {})", input, format_expr(offset), format_expr(count))
        }

        StepKind::ReverseRows { input } => {
            format!("Table.ReverseRows({})", input)
        }

        StepKind::Distinct { input, columns } => {
            let col_list = format_col_list(columns);
            format!("Table.Distinct({}, {})", input, col_list)
        }

        StepKind::Repeat { input, count } => {
            format!("Table.Repeat({}, {})", input, format_expr(count))
        }

        StepKind::AlternateRows { input, offset, skip, take } => {
            format!("Table.AlternateRows({}, {}, {}, {})",
                input, format_expr(offset), format_expr(skip), format_expr(take))
        }

        StepKind::FindText { input, text } => {
            format!("Table.FindText({}, \"{}\")", input, text)
        }

        StepKind::FillDown { input, columns } => {
            let col_list = format_col_list(columns);
            format!("Table.FillDown({}, {})", input, col_list)
        }

        StepKind::FillUp { input, columns } => {
            let col_list = format_col_list(columns);
            format!("Table.FillUp({}, {})", input, col_list)
        }

        StepKind::AddIndexColumn { input, col_name, start, step } => {
            if *start == 0 && *step == 1 {
                format!("Table.AddIndexColumn({}, \"{}\")", input, col_name)
            } else if *step == 1 {
                format!("Table.AddIndexColumn({}, \"{}\", {})", input, col_name, start)
            } else {
                format!("Table.AddIndexColumn({}, \"{}\", {}, {})", input, col_name, start, step)
            }
        }

        StepKind::DuplicateColumn { input, src_col, new_col } => {
            format!("Table.DuplicateColumn({}, \"{}\", \"{}\")", input, src_col, new_col)
        }

        StepKind::Unpivot { input, columns, attr_col, val_col } => {
            let col_list = format_col_list(columns);
            format!("Table.Unpivot({}, {}, \"{}\", \"{}\")", input, col_list, attr_col, val_col)
        }

        StepKind::UnpivotOtherColumns { input, keep_cols, attr_col, val_col } => {
            let col_list = format_col_list(keep_cols);
            format!("Table.UnpivotOtherColumns({}, {}, \"{}\", \"{}\")", input, col_list, attr_col, val_col)
        }

        StepKind::Transpose { input } => {
            format!("Table.Transpose({})", input)
        }

        StepKind::CombineTables { inputs } => {
            let list = inputs.iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            format!("Table.Combine({{{}}})", list)
        }

        StepKind::RemoveRowsWithErrors { input, columns } => {
            let col_list = format_col_list(columns);
            format!("Table.RemoveRowsWithErrors({}, {})", input, col_list)
        }

        StepKind::SelectRowsWithErrors { input, columns } => {
            let col_list = format_col_list(columns);
            format!("Table.SelectRowsWithErrors({}, {})", input, col_list)
        }

        StepKind::TransformRows { input, transform } => {
            format!("Table.TransformRows({}, {})", input, format_expr(transform))
        }

        StepKind::MatchesAllRows { input, condition } => {
            format!("Table.MatchesAllRows({}, {})", input, format_expr(condition))
        }

        StepKind::MatchesAnyRows { input, condition } => {
            format!("Table.MatchesAnyRows({}, {})", input, format_expr(condition))
        }

        StepKind::PrefixColumns { input, prefix } => {
            format!("Table.PrefixColumns({}, \"{}\")", input, prefix)
        }

        StepKind::DemoteHeaders { input } => {
            format!("Table.DemoteHeaders({})", input)
        }

        // ── Column operations ────────────────────────────────────────────
        StepKind::SelectColumns { input, columns } => {
            let col_list = format_col_list(columns);
            format!("Table.SelectColumns({}, {})", input, col_list)
        }

        StepKind::ReorderColumns { input, columns } => {
            let col_list = format_col_list(columns);
            format!("Table.ReorderColumns({}, {})", input, col_list)
        }

        StepKind::TransformColumnNames { input, transform } => {
            format!("Table.TransformColumnNames({}, {})", input, format_expr(transform))
        }

        StepKind::CombineColumns { input, columns, combiner, new_col } => {
            let col_list = format_col_list(columns);
            format!("Table.CombineColumns({}, {}, {}, \"{}\")", input, col_list, format_expr(combiner), new_col)
        }

        StepKind::SplitColumn { input, col_name, splitter } => {
            format!("Table.SplitColumn({}, \"{}\", {})", input, col_name, format_expr(splitter))
        }

        StepKind::ExpandTableColumn { input, col_name, columns } => {
            let col_list = format_col_list(columns);
            format!("Table.ExpandTableColumn({}, \"{}\", {})", input, col_name, col_list)
        }

        StepKind::ExpandRecordColumn { input, col_name, fields } => {
            let col_list = format_col_list(fields);
            format!("Table.ExpandRecordColumn({}, \"{}\", {})", input, col_name, col_list)
        }

        StepKind::Pivot { input, pivot_col, attr_col, val_col } => {
            let col_list = format_col_list(pivot_col);
            format!("Table.Pivot({}, {}, \"{}\", \"{}\")", input, col_list, attr_col, val_col)
        }

        // ── Information functions ────────────────────────────────────────
        StepKind::RowCount { input } => {
            format!("Table.RowCount({})", input)
        }

        StepKind::ColumnCount { input } => {
            format!("Table.ColumnCount({})", input)
        }

        StepKind::TableColumnNames { input } => {
            format!("Table.ColumnNames({})", input)
        }

        StepKind::TableIsEmpty { input } => {
            format!("Table.IsEmpty({})", input)
        }

        StepKind::TableSchema { input } => {
            format!("Table.Schema({})", input)
        }

        // ── Membership functions ─────────────────────────────────────────
        StepKind::HasColumns { input, columns } => {
            let col_list = format_col_list(columns);
            format!("Table.HasColumns({}, {})", input, col_list)
        }

        StepKind::TableIsDistinct { input } => {
            format!("Table.IsDistinct({})", input)
        }

        // ── Joins ────────────────────────────────────────────────────────
        StepKind::Join { left, left_keys, right, right_keys, join_kind } => {
            let lk = format_col_list(left_keys);
            let rk = format_col_list(right_keys);
            format!("Table.Join({}, {}, {}, {}, {})", left, lk, right, rk, join_kind)
        }

        StepKind::NestedJoin { left, left_keys, right, right_keys, new_col, join_kind } => {
            let lk = format_col_list(left_keys);
            let rk = format_col_list(right_keys);
            format!("Table.NestedJoin({}, {}, {}, {}, \"{}\", {})", left, lk, right, rk, new_col, join_kind)
        }

        // ── Ordering ─────────────────────────────────────────────────────
        StepKind::AddRankColumn { input, col_name, by } => {
            let sort_list = format_sort_list(by);
            format!("Table.AddRankColumn({}, \"{}\", {})", input, col_name, sort_list)
        }

        StepKind::TableMax { input, col_name } => {
            format!("Table.Max({}, \"{}\")", input, col_name)
        }

        StepKind::TableMin { input, col_name } => {
            format!("Table.Min({}, \"{}\")", input, col_name)
        }

        StepKind::TableMaxN { input, count, col_name } => {
            format!("Table.MaxN({}, {}, \"{}\")", input, format_expr(count), col_name)
        }

        StepKind::TableMinN { input, count, col_name } => {
            format!("Table.MinN({}, {}, \"{}\")", input, format_expr(count), col_name)
        }

        // ── Value operations ─────────────────────────────────────────────
        StepKind::ReplaceValue { input, old_value, new_value, replacer } => {
            format!("Table.ReplaceValue({}, {}, {}, {})", input, format_expr(old_value), format_expr(new_value), format_expr(replacer))
        }

        StepKind::ReplaceErrorValues { input, replacements } => {
            let pairs = replacements.iter()
                .map(|(col, expr, opt_type)| {
                    if let Some(t) = opt_type {
                        format!("{{\"{}\", {}, {}}}", col, format_expr(expr), t.to_m_type())
                    } else {
                        format!("{{\"{}\", {}}}", col, format_expr(expr))
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("Table.ReplaceErrorValues({}, {{{}}})", input, pairs)
        }

        StepKind::InsertRows { input, offset } => {
            format!("Table.InsertRows({}, {})", input, offset)
        }
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
    format!("let\n{}\nin\n    {}", lines, program.output)
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