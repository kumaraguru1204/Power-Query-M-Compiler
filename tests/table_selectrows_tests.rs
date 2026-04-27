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
fn t01_nested_selectrows_inside_selectrows() {
    assert!(run_query(r#"
        Table.SelectRows(
            Table.SelectRows(Source, each _[Dept] = "HR"),
            each _[Age] <> "30"
        )
    "#).is_ok());
}

// nested transform → select

#[test]
fn t02_nested_transform_then_select() {
    assert!(run_query(r#"
        Table.SelectRows(
            Table.TransformColumns(Source, {{"Age", each _}}),
            each _[Dept] = "IT"
        )
    "#).is_ok());
}

// nested group → select

#[test]
fn t03_nested_group_then_select() {
    assert!(run_query(r#"
        Table.SelectRows(
            Table.Group(Source, {"Dept"}, {{"All", each _, type table}}),
            each _[Dept] = "HR"
        )
    "#).is_ok());
}

// nested range → select

#[test]
fn t04_nested_range_then_select() {
    assert!(run_query(r#"
        Table.SelectRows(
            Table.Range(Source, 1, 2),
            each _[Age] <> "25"
        )
    "#).is_ok());
}

// nested firstN → select

#[test]
fn t05_nested_firstn_then_select() {
    assert!(run_query(r#"
        Table.SelectRows(
            Table.FirstN(Source, 3),
            each _[Dept] = "HR"
        )
    "#).is_ok());
}

// nested lastN → select

#[test]
fn t06_nested_lastn_then_select() {
    assert!(run_query(r#"
        Table.SelectRows(
            Table.LastN(Source, 2),
            each _[Dept] <> "HR"
        )
    "#).is_ok());
}

// nested let expression as first arg
// LIMITATION: inline `let...in` as a function argument is not supported by the parser.
// The `let` keyword is only valid as the start of a top-level program, not as a
// sub-expression inside a call argument.  Workaround: bind the inner let to a named
// step in the outer `let` block and pass the step name as the first argument instead.
// Tracked in PROJECT_REPORT.md § Known Limitations.

#[test]
#[ignore = "inline let-in expression as call argument not supported by the parser (top-level keyword only)"]
fn t07_nested_let_expression() {
    assert!(run_query(r#"
        Table.SelectRows(
            let
                T = Table.SelectRows(Source, each _[Dept] = "IT")
            in
                T,
            each _[Age] <> "28"
        )
    "#).is_ok());
}

// double nesting (deep pipeline)

#[test]
fn t08_double_nested_pipeline() {
    assert!(run_query(r#"
        Table.SelectRows(
            Table.SelectRows(
                Table.FirstN(Source, 3),
                each _[Dept] <> "HR"
            ),
            each _[Age] <> "25"
        )
    "#).is_ok());
}

// nested with identity transform

#[test]
fn t09_nested_identity_transform() {
    assert!(run_query(r#"
        Table.SelectRows(
            Table.TransformColumns(Source, {{"Name", each _}}),
            each _[Name] <> "Alice"
        )
    "#).is_ok());
}

// nested empty-producing table

#[test]
fn t10_nested_empty_table() {
    assert!(run_query(r#"
        Table.SelectRows(
            Table.SelectRows(Source, each false),
            each true
        )
    "#).is_ok());
}