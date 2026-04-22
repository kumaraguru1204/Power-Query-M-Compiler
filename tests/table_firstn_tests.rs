use pq_engine::Engine;

/// Minimal JSON table used as context
const DUMMY_JSON: &str = r#"{
    "source": "dummy.xlsx",
    "sheet":  "Sheet1",
    "rows": [
        ["Name","Age"],
        ["Alice","30"],
        ["Bob","25"],
        ["Charlie","35"]
    ]
}"#;

/// Run full pipeline
fn run_query(query: &str) -> Result<(), String> {
    let formula = normalize(query);
    Engine::run_with_formula(DUMMY_JSON, &formula)
        .map(|_| ())
        .map_err(|e| Engine::render_error(&e, query))
}

/// Normalize to let-in
fn normalize(query: &str) -> String {
    let trimmed = query.trim();
    if trimmed.to_lowercase().starts_with("let") {
        trimmed.to_string()
    } else {
        format!("let\n    Source = 0\nin\n    {}", trimmed)
    }
}

// ── basic numeric cases ───────────────────────────────────────────────────

#[test]
fn t01_basic_number() {
    assert!(run_query(r#"
        let
            Source = Excel.Workbook(File.Contents("dummy.xlsx"), null, true)
        in
            Table.FirstN(Source, 2)
    "#).is_ok());
}

#[test]
fn t02_zero_rows() {
    assert!(run_query(r#"Table.FirstN(Source, 0)"#).is_ok());
}

// ── Gap 2 — numeric expression as count ───────────────────────────────────

#[test]
fn t03_expression_count() {
    assert!(run_query(r#"Table.FirstN(Source, 1 + 1)"#).is_ok());
}

#[test]
fn t04_expression_with_variable() {
    assert!(run_query(r#"
        let
            n = 2
        in
            Table.FirstN(Source, n)
    "#).is_ok());
}

// ── Gap 1 — predicate / lambda / function-ref ─────────────────────────────

#[test]
fn t05_each_predicate() {
    assert!(run_query(r#"
        Table.FirstN(Source, each _[Age] <> "25")
    "#).is_ok());
}

#[test]
fn t06_explicit_lambda() {
    assert!(run_query(r#"
        Table.FirstN(Source, (x) => x[Age] <> "25")
    "#).is_ok());
}

#[test]
fn t07_function_reference() {
    assert!(run_query(r#"
        let
            f = (x) => x[Age] <> "25"
        in
            Table.FirstN(Source, f)
    "#).is_ok());
}

// ── Gap 3 — nested first argument ─────────────────────────────────────────

#[test]
fn t08_nested_table_select() {
    assert!(run_query(r#"
        Table.FirstN(
            Table.SelectRows(Source, each _[Age] <> "30"),
            2
        )
    "#).is_ok());
}

#[test]
fn t09_nested_transform() {
    assert!(run_query(r#"
        Table.FirstN(
            Table.TransformColumns(Source, {{"Age", each _}}),
            1
        )
    "#).is_ok());
}

// ── Gap 4 — SQL generator cases (predicate form) ──────────────────────────

#[test]
fn t10_predicate_simple_sql() {
    assert!(run_query(r#"
        Table.FirstN(Source, each _[Name] <> "Bob")
    "#).is_ok());
}

#[test]
fn t11_predicate_with_and() {
    assert!(run_query(r#"
        Table.FirstN(Source, each _[Age] <> "25" and _[Name] <> "Alice")
    "#).is_ok());
}

#[test]
fn t12_predicate_nested_field_access() {
    assert!(run_query(r#"
        Table.FirstN(Source, each _[Age] <> "35")
    "#).is_ok());
}

// ── edge cases ────────────────────────────────────────────────────────────

#[test]
fn t13_large_n() {
    assert!(run_query(r#"Table.FirstN(Source, 100)"#).is_ok());
}

#[test]
fn t14_empty_table() {
    assert!(run_query(r#"
        let
            Empty = Table.SelectRows(Source, each false)
        in
            Table.FirstN(Empty, 1)
    "#).is_ok());
}

#[test]
fn t15_predicate_always_true() {
    assert!(run_query(r#"
        Table.FirstN(Source, each true)
    "#).is_ok());
}

#[test]
fn t16_predicate_always_false() {
    assert!(run_query(r#"
        Table.FirstN(Source, each false)
    "#).is_ok());
}