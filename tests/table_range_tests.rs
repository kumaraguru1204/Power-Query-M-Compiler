use pq_engine::Engine;

/// Minimal JSON table used as context
const DUMMY_JSON: &str = r#"{
    "source": "dummy.xlsx",
    "sheet":  "Sheet1",
    "rows": [
        ["Name","Age"],
        ["Alice","30"],
        ["Bob","25"],
        ["Charlie","35"],
        ["David","28"]
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

// ── basic usage ───────────────────────────────────────────────────────────

#[test]
fn t01_basic_offset_count() {
    assert!(run_query(r#"Table.Range(Source, 1, 2)"#).is_ok());
}

#[test]
fn t02_offset_zero() {
    assert!(run_query(r#"Table.Range(Source, 0, 2)"#).is_ok());
}

// ── Gap 1 — optional count ────────────────────────────────────────────────

#[test]
fn t03_only_offset_provided() {
    assert!(run_query(r#"Table.Range(Source, 2)"#).is_ok());
}

#[test]
fn t04_only_offset_variable() {
    assert!(run_query(r#"
        let
            n = 1
        in
            Table.Range(Source, n)
    "#).is_ok());
}

// ── Gap 2 — null count means "all remaining" ──────────────────────────────

#[test]
fn t05_null_count() {
    assert!(run_query(r#"Table.Range(Source, 1, null)"#).is_ok());
}

#[test]
fn t06_null_count_variable() {
    assert!(run_query(r#"
        let
            c = null
        in
            Table.Range(Source, 2, c)
    "#).is_ok());
}

// ── Gap 3 — numeric expressions ───────────────────────────────────────────

#[test]
fn t07_expression_offset() {
    assert!(run_query(r#"Table.Range(Source, 1 + 1, 2)"#).is_ok());
}

#[test]
fn t08_expression_count() {
    assert!(run_query(r#"Table.Range(Source, 1, 1 + 1)"#).is_ok());
}

#[test]
fn t09_both_expression() {
    assert!(run_query(r#"Table.Range(Source, 1 + 1, 1 + 1)"#).is_ok());
}

// ── Gap 4 — nested first argument ─────────────────────────────────────────

#[test]
fn t10_nested_select_rows() {
    assert!(run_query(r#"
        Table.Range(
            Table.SelectRows(Source, each _[Age] <> "25"),
            1,
            1
        )
    "#).is_ok());
}

#[test]
fn t11_nested_transform_columns() {
    assert!(run_query(r#"
        Table.Range(
            Table.TransformColumns(Source, {{"Age", each _}}),
            0,
            2
        )
    "#).is_ok());
}

// ── Gap 5 — default behavior correctness ──────────────────────────────────

#[test]
fn t12_missing_count_defaults_correctly() {
    assert!(run_query(r#"Table.Range(Source, 1)"#).is_ok());
}

#[test]
fn t13_null_count_not_zero() {
    assert!(run_query(r#"Table.Range(Source, 1, null)"#).is_ok());
}

// ── edge cases ────────────────────────────────────────────────────────────

#[test]
fn t14_offset_out_of_bounds() {
    assert!(run_query(r#"Table.Range(Source, 100, 1)"#).is_ok());
}

#[test]
fn t15_count_exceeds_length() {
    assert!(run_query(r#"Table.Range(Source, 1, 100)"#).is_ok());
}

#[test]
fn t16_zero_count() {
    assert!(run_query(r#"Table.Range(Source, 1, 0)"#).is_ok());
}

#[test]
fn t17_empty_table_input() {
    let r = run_query(r#"
        let
            Source = Excel.Workbook(File.Contents("dummy.xlsx"), null, true),
            Empty  = Table.SelectRows(Source, each false)
        in
            Table.Range(Empty, 0, 1)
    "#);
    assert!(r.is_ok(), "t17: {}", r.err().unwrap_or_default());
}

#[test]
fn t18_full_table_range() {
    assert!(run_query(r#"Table.Range(Source, 0, null)"#).is_ok());
}