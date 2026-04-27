use pq_engine::Engine;

/// Minimal JSON table used as context
const DUMMY_JSON: &str = r#"{
    "source": "dummy.xlsx",
    "sheet":  "Sheet1",
    "rows": [
        ["Name","Dept","Age"],
        ["Alice","HR","30"],
        ["Bob","IT","25"],
        ["Charlie","HR","35"],
        ["David","IT","28"]
    ]
}"#;

/// Run full pipeline
fn run_query(query: &str) -> Result<(), String> {
    let formula = normalize(query);
    Engine::run_with_formula(DUMMY_JSON, &formula)
        .map(|_| ())
        .map_err(|e| Engine::render_error(&e, query))
}

/// Normalize into let-in
fn normalize(query: &str) -> String {
    let trimmed = query.trim();
    if trimmed.to_lowercase().starts_with("let") {
        trimmed.to_string()
    } else {
        format!("let\n    Source = 0\nin\n    {}", trimmed)
    }
}

// ── nested first argument ─────────────────────────────────────────────────

// basic nested select

#[test]
fn t01_nested_selectrows() {
    assert!(run_query(r#"
        Table.RowCount(
            Table.SelectRows(Source, each _[Dept] = "HR")
        )
    "#).is_ok());
}

// nested transform

#[test]
fn t02_nested_transformcolumns() {
    assert!(run_query(r#"
        Table.RowCount(
            Table.TransformColumns(Source, {{"Age", each _}})
        )
    "#).is_ok());
}

// nested range

#[test]
fn t03_nested_range() {
    assert!(run_query(r#"
        Table.RowCount(
            Table.Range(Source, 1, 2)
        )
    "#).is_ok());
}

// nested firstN

#[test]
fn t04_nested_firstn() {
    assert!(run_query(r#"
        Table.RowCount(
            Table.FirstN(Source, 3)
        )
    "#).is_ok());
}

// nested lastN

#[test]
fn t05_nested_lastn() {
    assert!(run_query(r#"
        Table.RowCount(
            Table.LastN(Source, 2)
        )
    "#).is_ok());
}

// nested group

#[test]
fn t06_nested_group() {
    assert!(run_query(r#"
        Table.RowCount(
            Table.Group(Source, {"Dept"}, {{"All", each _, type table}})
        )
    "#).is_ok());
}

// nested let expression

#[test]
#[ignore = "inline let-in expression as call argument not supported by the parser (top-level keyword only)"]
fn t07_nested_let_expression() {
    assert!(run_query(r#"
        Table.RowCount(
            let
                T = Table.SelectRows(Source, each _[Dept] = "IT")
            in
                T
        )
    "#).is_ok());
}

// double nesting

#[test]
fn t08_double_nested_pipeline() {
    assert!(run_query(r#"
        Table.RowCount(
            Table.SelectRows(
                Table.FirstN(Source, 3),
                each _[Dept] <> "HR"
            )
        )
    "#).is_ok());
}

// nested empty table

#[test]
fn t09_nested_empty_table() {
    assert!(run_query(r#"
        Table.RowCount(
            Table.SelectRows(Source, each false)
        )
    "#).is_ok());
}

// nested identity transform

#[test]
fn t10_nested_identity_transform() {
    assert!(run_query(r#"
        Table.RowCount(
            Table.TransformColumns(Source, {{"Name", each _}})
        )
    "#).is_ok());
}