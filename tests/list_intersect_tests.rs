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

// ── A — inline list of inline lists ───────────────────────────────────────

#[test]
fn t01_basic_intersect() {
    assert!(run_query(r#"List.Intersect({{"A","B","C"},{"B","C","D"}})"#).is_ok());
}

#[test]
fn t02_three_lists_intersect() {
    assert!(run_query(r#"List.Intersect({{"A","B"},{"B","C"},{"B","D"}})"#).is_ok());
}

// ── B — step ref pointing to list-of-lists ────────────────────────────────

#[test]
fn t03_step_ref_list_of_lists() {
    assert!(run_query(r#"
        let
            L = {{"A","B","C"},{"B","C"}}
        in
            List.Intersect(L)
    "#).is_ok());
}

// ── C — inline list with step refs ────────────────────────────────────────

#[test]
fn t04_inline_list_with_step_refs() {
    assert!(run_query(r#"
        let
            A = {"A","B","C"},
            B = {"B","C"}
        in
            List.Intersect({A, B})
    "#).is_ok());
}

// ── D — nested function producing list-of-lists ───────────────────────────

#[test]
fn t05_nested_list_transform() {
    assert!(run_query(r#"
        List.Intersect(
            List.Transform(
                {{"A","B"}, {"B","C"}},
                each _
            )
        )
    "#).is_ok());
}

// ── E — equationCriteria variations ───────────────────────────────────────

#[test]
fn t06_numeric_equation_criteria() {
    assert!(run_query(r#"
        List.Intersect({{"a","b"},{"B"}}, 0)
    "#).is_ok());
}

#[test]
fn t07_function_reference_comparer() {
    assert!(run_query(r#"
        List.Intersect(
            {{"A","B","C"},{"b"}},
            Comparer.OrdinalIgnoreCase
        )
    "#).is_ok());
}

#[test]
fn t08_each_key_extractor() {
    assert!(run_query(r#"
        List.Intersect(
            {{"apple","banana"},{"A","B"}},
            each Text.Start(_,1)
        )
    "#).is_ok());
}

#[test]
fn t09_two_arg_lambda_comparer() {
    assert!(run_query(r#"
        List.Intersect(
            {{"A","B","C"},{"b"}},
            (x,y) => Text.Lower(x) = Text.Lower(y)
        )
    "#).is_ok());
}

#[test]
fn t10_options_record_comparer() {
    assert!(run_query(r#"
        List.Intersect(
            {{"A","B","C"},{"b"}},
            [Comparer = Comparer.OrdinalIgnoreCase]
        )
    "#).is_ok());
}

// ── edge cases ────────────────────────────────────────────────────────────

#[test]
fn t11_with_nulls() {
    assert!(run_query(r#"
        List.Intersect({{"A", null, "B"}, {null, "B"}})
    "#).is_ok());
}

#[test]
fn t12_empty_inner_list() {
    assert!(run_query(r#"
        List.Intersect({{"A","B"}, {}})
    "#).is_ok());
}

#[test]
fn t13_single_list_input() {
    assert!(run_query(r#"
        List.Intersect({{"A","B","C"}})
    "#).is_ok());
}

#[test]
fn t14_all_empty_lists() {
    assert!(run_query(r#"
        List.Intersect({{}, {}})
    "#).is_ok());
}