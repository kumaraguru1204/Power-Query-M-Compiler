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

/// Wrap bare M expressions in `let … in …` so the parser accepts them.
fn normalize(query: &str) -> String {
    let trimmed = query.trim();
    if trimmed.to_lowercase().starts_with("let") {
        trimmed.to_string()
    } else {
        format!("let\n    _Dummy = 0\nin\n    {}", trimmed)
    }
}

// ── A — inline list + scalar ──────────────────────────────────────────────

#[test]
fn t01_basic_contains_true() {
    assert!(run_query(r#"List.Contains({"A","B","C"}, "B")"#).is_ok());
}

#[test]
fn t02_basic_contains_false() {
    assert!(run_query(r#"List.Contains({"A","B","C"}, "X")"#).is_ok());
}

#[test]
fn t03_with_duplicates() {
    assert!(run_query(r#"List.Contains({"A","B","B"}, "B")"#).is_ok());
}

#[test]
fn t04_with_null() {
    assert!(run_query(r#"List.Contains({"A", null, "B"}, null)"#).is_ok());
}

// ── B — step ref as list ──────────────────────────────────────────────────

#[test]
fn t05_step_ref_list() {
    assert!(run_query(r#"
        let
            L = {"HR","IT","Finance"}
        in
            List.Contains(L, "IT")
    "#).is_ok());
}

#[test]
fn t06_step_ref_not_found() {
    assert!(run_query(r#"
        let
            L = {"A","B","C"}
        in
            List.Contains(L, "Z")
    "#).is_ok());
}

// ── C — nested call producing list ────────────────────────────────────────

#[test]
fn t07_nested_list_select() {
    assert!(run_query(r#"
        List.Contains(
            List.Select({"A","B","C"}, each _ <> "C"),
            "B"
        )
    "#).is_ok());
}

#[test]
fn t08_nested_list_transform() {
    assert!(run_query(r#"
        List.Contains(
            List.Transform({"a","b"}, each _ & "_x"),
            "b_x"
        )
    "#).is_ok());
}

// ── D — value as nested call ──────────────────────────────────────────────

#[test]
fn t09_value_nested_expression() {
    assert!(run_query(r#"
        List.Contains(
            {"A","B","C"},
            Text.Upper("b")
        )
    "#).is_ok());
}

#[test]
fn t10_value_from_lambda_context() {
    assert!(run_query(r#"
        List.Contains(
            {"A","B","C"},
            (x) => x
        )
    "#).is_ok());
}

// ── E — equationCriteria variations ───────────────────────────────────────

#[test]
fn t11_numeric_equation_criteria() {
    assert!(run_query(r#"
        List.Contains({"a","b","c"}, "B", 0)
    "#).is_ok());
}

#[test]
fn t12_function_reference_comparer() {
    assert!(run_query(r#"
        List.Contains(
            {"A","B","C"},
            "b",
            Comparer.OrdinalIgnoreCase
        )
    "#).is_ok());
}

#[test]
fn t13_each_key_extractor() {
    assert!(run_query(r#"
        List.Contains(
            {"apple","banana"},
            "A",
            each Text.Start(_,1)
        )
    "#).is_ok());
}

#[test]
fn t14_two_arg_lambda_comparer() {
    assert!(run_query(r#"
        List.Contains(
            {"A","B","C"},
            "b",
            (x,y) => Text.Lower(x) = Text.Lower(y)
        )
    "#).is_ok());
}

#[test]
fn t15_options_record_comparer() {
    assert!(run_query(r#"
        List.Contains(
            {"A","B","C"},
            "b",
            [Comparer = Comparer.OrdinalIgnoreCase]
        )
    "#).is_ok());
}

// ── edge cases ────────────────────────────────────────────────────────────

#[test]
fn t16_empty_list() {
    assert!(run_query(r#"List.Contains({}, "A")"#).is_ok());
}

#[test]
fn t17_all_nulls() {
    assert!(run_query(r#"List.Contains({null, null}, null)"#).is_ok());
}

#[test]
fn t18_mixed_types() {
    assert!(run_query(r#"List.Contains({"A", 1, null}, 1)"#).is_ok());
}