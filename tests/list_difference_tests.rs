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

// ── basic set difference ───────────────────────────────────────────────────

#[test]
fn t01_basic_single_remove() {
    assert!(run_query(r#"List.Difference({"A","B","C"},{"B"})"#).is_ok());
}

#[test]
fn t02_remove_two_elements() {
    assert!(run_query(r#"List.Difference({"A","B","C","D"},{"B","D"})"#).is_ok());
}

#[test]
fn t03_remove_from_list_with_duplicates() {
    assert!(run_query(r#"List.Difference({"A","B","B","C"},{"B"})"#).is_ok());
}

#[test]
fn t04_remove_list_contains_duplicates() {
    assert!(run_query(r#"List.Difference({"A","B"},{"B","B"})"#).is_ok());
}

#[test]
fn t05_both_sides_have_duplicates() {
    assert!(run_query(r#"List.Difference({"A","B","B"},{"B","B"})"#).is_ok());
}

// ── let-binding shapes ─────────────────────────────────────────────────────

#[test]
fn t06_let_bound_lists() {
    assert!(run_query(r#"
        let
            L1 = {"A","B","C"},
            L2 = {"B"}
        in
            List.Difference(L1, L2)
    "#).is_ok());
}

// ── nested calls ───────────────────────────────────────────────────────────

#[test]
fn t07_nested_list_select() {
    assert!(run_query(r#"
        List.Difference(
            List.Select({"A","B","C"}, each _ <> "C"),
            {"B"}
        )
    "#).is_ok());
}

// ── third comparer argument ────────────────────────────────────────────────

#[test]
fn t08_numeric_comparer_ordinal() {
    assert!(run_query(r#"List.Difference({"a","b","c"},{"B"}, 0)"#).is_ok());
}

#[test]
fn t09_ordinal_ignore_case_constant() {
    assert!(run_query(r#"List.Difference({"A","B","C"},{"b"}, Comparer.OrdinalIgnoreCase)"#).is_ok());
}

#[test]
fn t10_each_function_as_comparer() {
    let result = run_query(r#"
        List.Difference(
            {"apple","banana","cherry"},
            {"A","B"},
            each Text.Start(_,1)
        )
    "#);
    assert!(result.is_ok(), "t10 failed: {}", result.unwrap_err());
}

#[test]
fn t11_lambda_equality_comparer() {
    assert!(run_query(r#"
        List.Difference(
            {"A","B","C"},
            {"b"},
            (x,y) => Text.Lower(x) = Text.Lower(y)
        )
    "#).is_ok());
}

#[test]
fn t12_record_options_comparer() {
    assert!(run_query(r#"
        List.Difference(
            {"A","B","C"},
            {"b"},
            [Comparer = Comparer.OrdinalIgnoreCase]
        )
    "#).is_ok());
}

// ── edge cases ─────────────────────────────────────────────────────────────

#[test]
fn t13_null_in_list() {
    assert!(run_query(r#"List.Difference({"A", null, "B"}, {null})"#).is_ok());
}

#[test]
fn t14_empty_source_list() {
    assert!(run_query(r#"List.Difference({}, {"A"})"#).is_ok());
}

#[test]
fn t15_empty_remove_list() {
    assert!(run_query(r#"List.Difference({"A","B"}, {})"#).is_ok());
}
