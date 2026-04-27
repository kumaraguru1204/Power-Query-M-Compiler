use pq_engine::Engine;

/// Minimal JSON table used as context
const DUMMY_JSON: &str = r#"{
    "source": "dummy.xlsx",
    "sheet":  "Sheet1",
    "rows": [
        ["Name","Nested"],
        ["Alice", ""],
        ["Bob",   ""]
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

// ── basic expand ──────────────────────────────────────────────────────────

#[test]
fn t01_basic_expand() {
    assert!(run_query(r#"
        Table.ExpandTableColumn(Source, "Nested", {"Age"})
    "#).is_ok());
}

// ── Gap 1 — optional newColumnNames ───────────────────────────────────────

#[test]
fn t02_with_new_column_names() {
    assert!(run_query(r#"
        Table.ExpandTableColumn(
            Source,
            "Nested",
            {"Age","City"},
            {"Age_New","City_New"}
        )
    "#).is_ok());
}

#[test]
fn t03_new_column_names_variable() {
    assert!(run_query(r#"
        let
            cols = {"Age","City"},
            newCols = {"A","C"}
        in
            Table.ExpandTableColumn(Source, "Nested", cols, newCols)
    "#).is_ok());
}

#[test]
fn t04_missing_new_column_names() {
    assert!(run_query(r#"
        Table.ExpandTableColumn(Source, "Nested", {"Age","City"})
    "#).is_ok());
}

// ── Gap 2 — nested first argument ─────────────────────────────────────────

#[test]
fn t05_nested_selectrows() {
    assert!(run_query(r#"
        Table.ExpandTableColumn(
            Table.SelectRows(Source, each true),
            "Nested",
            {"Age"}
        )
    "#).is_ok());
}

#[test]
fn t06_nested_transformcolumns() {
    assert!(run_query(r#"
        Table.ExpandTableColumn(
            Table.TransformColumns(Source, {{"Name", each _}}),
            "Nested",
            {"City"}
        )
    "#).is_ok());
}

#[test]
fn t07_nested_group() {
    assert!(run_query(r#"
        Table.ExpandTableColumn(
            Table.Group(Source, {"Name"}, {{"Nested", each _, type table}}),
            "Nested",
            {"Name"}
        )
    "#).is_ok());
}

// ── Gap 3 — column as identifier / expression ─────────────────────────────

#[test]
fn t08_column_as_variable() {
    assert!(run_query(r#"
        let
            col = "Nested"
        in
            Table.ExpandTableColumn(Source, col, {"Age"})
    "#).is_ok());
}

#[test]
fn t09_column_as_expression() {
    assert!(run_query(r#"
        Table.ExpandTableColumn(Source, "Nest" & "ed", {"Age"})
    "#).is_ok());
}

#[test]
fn t10_column_from_function() {
    assert!(run_query(r#"
        Table.ExpandTableColumn(Source, Text.Start("NestedX",6), {"Age"})
    "#).is_ok());
}

// ── edge cases ────────────────────────────────────────────────────────────

#[test]
fn t11_empty_column_list() {
    assert!(run_query(r#"
        Table.ExpandTableColumn(Source, "Nested", {})
    "#).is_ok());
}

#[test]
fn t12_nested_pipeline() {
    assert!(run_query(r#"
        Table.ExpandTableColumn(
            Table.SelectRows(
                Table.TransformColumns(Source, {{"Name", each _}}),
                each true
            ),
            "Nested",
            {"Age","City"}
        )
    "#).is_ok());
}

#[test]
fn t13_expand_after_range() {
    assert!(run_query(r#"
        Table.ExpandTableColumn(
            Table.Range(Source, 0, 1),
            "Nested",
            {"Age"}
        )
    "#).is_ok());
}

#[test]
fn t14_null_new_column_names() {
    assert!(run_query(r#"
        Table.ExpandTableColumn(Source, "Nested", {"Age"}, null)
    "#).is_ok());
}

#[test]
fn t15_variable_all_args() {
    assert!(run_query(r#"
        let
            col = "Nested",
            cols = {"Age","City"},
            newCols = {"A","C"}
        in
            Table.ExpandTableColumn(Source, col, cols, newCols)
    "#).is_ok());
}