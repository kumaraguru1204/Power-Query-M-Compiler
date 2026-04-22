use pq_engine::Engine;

/// Minimal JSON table used as context for pure expression tests.
const DUMMY_JSON: &str = r#"{
    "source": "dummy.xlsx",
    "sheet":  "Sheet1",
    "rows":   [["Col1"], ["A"]]
}"#;

/// Run the full compiler pipeline on a bare M expression or a full let-in formula.
fn run_query(query: &str) -> Result<(), String> {
    let formula = normalize(query);
    Engine::run_with_formula(DUMMY_JSON, &formula)
        .map(|_| ())
        .map_err(|e| Engine::render_error(&e, query))
}

/// Wrap bare M expressions in `let … in …`
fn normalize(query: &str) -> String {
    let trimmed = query.trim();
    if trimmed.to_lowercase().starts_with("let") {
        trimmed.to_string()
    } else {
        format!("let\n    _Dummy = 0\nin\n    {}", trimmed)
    }
}

// ── basic correctness ─────────────────────────────────────────────────────

#[test]
fn t01_basic_each_identity() {
    assert!(run_query(r#"List.Transform({"A","B"}, each _)"#).is_ok());
}

#[test]
fn t02_each_concat() {
    assert!(run_query(r#"List.Transform({"A","B"}, each _ & "_X")"#).is_ok());
}

// ── Gap 1 — bare function reference ───────────────────────────────────────

#[test]
fn t03_function_reference_text_from() {
    assert!(run_query(r#"List.Transform({"1","2"}, Text.From)"#).is_ok());
}

#[test]
fn t04_function_reference_identity_style() {
    assert!(run_query(r#"
        let
            F = Text.From
        in
            List.Transform({"A","B"}, F)
    "#).is_ok());
}

// ── Gap 2 — explicit lambda (x) => … ──────────────────────────────────────

#[test]
fn t05_explicit_lambda_simple() {
    assert!(run_query(r#"
        List.Transform({"A","B"}, (x) => x & "_L")
    "#).is_ok());
}

#[test]
fn t06_explicit_lambda_numeric_style() {
    assert!(run_query(r#"
        List.Transform({"1","2"}, (x) => x)
    "#).is_ok());
}

// ── Gap 3 — records in input list ─────────────────────────────────────────

#[test]
fn t07_record_access_field() {
    assert!(run_query(r#"
        List.Transform(
            {
                [Name="Alice", Age=30],
                [Name="Bob", Age=25]
            },
            each _[Name]
        )
    "#).is_ok());
}

#[test]
fn t08_record_access_multiple_fields() {
    assert!(run_query(r#"
        List.Transform(
            {
                [Name="A", Age=10],
                [Name="B", Age=20]
            },
            each _[Name] & "_X"
        )
    "#).is_ok());
}

#[test]
fn t09_record_with_missing_field() {
    assert!(run_query(r#"
        List.Transform(
            {
                [Name="A"],
                [Name="B", Age=25]
            },
            each _[Age]
        )
    "#).is_ok());
}

// ── Gap 4 — nested list input ─────────────────────────────────────────────

#[test]
fn t10_nested_list_identity() {
    assert!(run_query(r#"
        List.Transform(
            {{1,2},{3,4}},
            each _
        )
    "#).is_ok());
}

#[test]
fn t11_nested_list_transform_inner() {
    assert!(run_query(r#"
        List.Transform(
            {{1,2},{3,4}},
            each List.Transform(_, each _)
        )
    "#).is_ok());
}

#[test]
fn t12_nested_list_length_style() {
    assert!(run_query(r#"
        List.Transform(
            {{1,2},{3,4}},
            each _
        )
    "#).is_ok());
}

// ── edge cases ────────────────────────────────────────────────────────────

#[test]
fn t13_empty_list() {
    assert!(run_query(r#"List.Transform({}, each _)"#).is_ok());
}

#[test]
fn t14_null_values() {
    assert!(run_query(r#"List.Transform({null, "A"}, each _)"#).is_ok());
}

#[test]
fn t15_constant_lambda() {
    assert!(run_query(r#"List.Transform({"A","B"}, each "CONST")"#).is_ok());
}

#[test]
fn t16_nested_transform_chain() {
    assert!(run_query(r#"
        List.Transform(
            List.Transform({"A","B"}, each _ & "_1"),
            each _ & "_2"
        )
    "#).is_ok());
}