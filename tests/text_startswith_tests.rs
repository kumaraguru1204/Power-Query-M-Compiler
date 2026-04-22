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
fn t01_basic_true() {
    assert!(run_query(r#"Text.StartsWith("Apple","A")"#).is_ok());
}

#[test]
fn t02_basic_false() {
    assert!(run_query(r#"Text.StartsWith("Apple","B")"#).is_ok());
}

// ── Gap 1 — substring not just literal ─────────────────────────────────────

#[test]
fn t03_substring_from_variable() {
    assert!(run_query(r#"
        let
            s = "A"
        in
            Text.StartsWith("Apple", s)
    "#).is_ok());
}

#[test]
fn t04_substring_from_expression() {
    assert!(run_query(r#"
        Text.StartsWith("Apple", "A" & "")
    "#).is_ok());
}

#[test]
fn t05_substring_from_function() {
    assert!(run_query(r#"
        Text.StartsWith("Apple", Text.Start("ABC",1))
    "#).is_ok());
}

// ── Gap 2 — optional comparer ─────────────────────────────────────────────

#[test]
fn t06_with_ordinal_ignore_case() {
    assert!(run_query(r#"
        Text.StartsWith("Apple", "a", Comparer.OrdinalIgnoreCase)
    "#).is_ok());
}

#[test]
fn t07_with_comparer_variable() {
    assert!(run_query(r#"
        let
            cmp = Comparer.OrdinalIgnoreCase
        in
            Text.StartsWith("Apple", "a", cmp)
    "#).is_ok());
}

#[test]
fn t08_with_options_record() {
    assert!(run_query(r#"
        Text.StartsWith("Apple", "a", [Comparer = Comparer.OrdinalIgnoreCase])
    "#).is_ok());
}

#[test]
fn t09_with_numeric_comparer() {
    assert!(run_query(r#"
        Text.StartsWith("Apple", "a", 0)
    "#).is_ok());
}

// ── Gap 3 — nullable text argument ────────────────────────────────────────

#[test]
fn t10_null_text_input() {
    assert!(run_query(r#"
        Text.StartsWith(null, "A")
    "#).is_ok());
}

#[test]
fn t11_null_substring() {
    assert!(run_query(r#"
        Text.StartsWith("Apple", null)
    "#).is_ok());
}

#[test]
fn t12_both_null() {
    assert!(run_query(r#"
        Text.StartsWith(null, null)
    "#).is_ok());
}

// ── Gap 4 — nullable result type propagation ──────────────────────────────

#[test]
fn t13_nullable_pipeline_usage() {
    assert!(run_query(r#"
        let
            res = Text.StartsWith(null, "A")
        in
            res
    "#).is_ok());
}

#[test]
fn t14_nullable_inside_list() {
    assert!(run_query(r#"
        List.Transform(
            {"Apple", null},
            each Text.StartsWith(_, "A")
        )
    "#).is_ok());
}

// ── edge cases ────────────────────────────────────────────────────────────

#[test]
fn t15_empty_string() {
    assert!(run_query(r#"Text.StartsWith("", "A")"#).is_ok());
}

#[test]
fn t16_empty_substring() {
    assert!(run_query(r#"Text.StartsWith("Apple", "")"#).is_ok());
}

#[test]
fn t17_both_empty() {
    assert!(run_query(r#"Text.StartsWith("", "")"#).is_ok());
}

#[test]
fn t18_nested_calls() {
    assert!(run_query(r#"
        Text.StartsWith(
            Text.Start("Apple",5),
            "A"
        )
    "#).is_ok());
}

#[test]
fn t19_inside_list_select() {
    assert!(run_query(r#"
        List.Select(
            {"Apple","Banana"},
            each Text.StartsWith(_, "A")
        )
    "#).is_ok());
}