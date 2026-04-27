use pq_engine::Engine;

/// Minimal JSON table used as context
const DUMMY_JSON: &str = r#"{
    "source": "dummy.xlsx",
    "sheet":  "Sheet1",
    "rows": [
        ["Name","Dept","Salary"],
        ["Alice","HR","50000"],
        ["Bob","IT","40000"],
        ["Charlie","HR","60000"],
        ["David","IT","45000"]
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

// ── basic grouping ─────────────────────────────────────────────────────────

#[test]
fn t01_basic_group() {
    assert!(run_query(r#"
        Table.Group(Source, {"Dept"}, {{"Count", each _, type table}})
    "#).is_ok());
}

// ── Gap 1 — single-string key ─────────────────────────────────────────────

#[test]
fn t02_single_string_key() {
    assert!(run_query(r#"
        Table.Group(Source, "Dept", {{"Count", each _, type table}})
    "#).is_ok());
}

// ── Gap 2 — single aggregate triple (no outer braces) ─────────────────────

#[test]
fn t03_single_aggregate_no_outer_list() {
    assert!(run_query(r#"
        Table.Group(Source, {"Dept"}, {"AllRows", each _, type table})
    "#).is_ok());
}

// ── multiple aggregates (control case) ─────────────────────────────────────

#[test]
fn t04_multiple_aggregates() {
    assert!(run_query(r#"
        Table.Group(
            Source,
            {"Dept"},
            {
                {"A", each _, type table},
                {"B", each _, type table}
            }
        )
    "#).is_ok());
}

// ── Gap 3 — optional groupKind ────────────────────────────────────────────

#[test]
fn t05_with_groupkind() {
    assert!(run_query(r#"
        Table.Group(
            Source,
            {"Dept"},
            {{"AllRows", each _, type table}},
            GroupKind.Local
        )
    "#).is_ok());
}

#[test]
fn t06_groupkind_variable() {
    assert!(run_query(r#"
        let
            gk = GroupKind.Local
        in
            Table.Group(
                Source,
                {"Dept"},
                {{"AllRows", each _, type table}},
                gk
            )
    "#).is_ok());
}

// ── Gap 4 — optional comparer ─────────────────────────────────────────────

#[test]
fn t07_with_comparer() {
    assert!(run_query(r#"
        Table.Group(
            Source,
            {"Dept"},
            {{"AllRows", each _, type table}},
            null,
            Comparer.OrdinalIgnoreCase
        )
    "#).is_ok());
}

#[test]
fn t08_comparer_variable() {
    assert!(run_query(r#"
        let
            cmp = Comparer.OrdinalIgnoreCase
        in
            Table.Group(
                Source,
                {"Dept"},
                {{"AllRows", each _, type table}},
                null,
                cmp
            )
    "#).is_ok());
}

#[test]
fn t09_comparer_record() {
    assert!(run_query(r#"
        Table.Group(
            Source,
            {"Dept"},
            {{"AllRows", each _, type table}},
            null,
            [Comparer = Comparer.OrdinalIgnoreCase]
        )
    "#).is_ok());
}

// ── Gap 5 — nested first argument ─────────────────────────────────────────

#[test]
fn t10_nested_selectrows() {
    assert!(run_query(r#"
        Table.Group(
            Table.SelectRows(Source, each _[Dept] <> "IT"),
            {"Dept"},
            {{"AllRows", each _, type table}}
        )
    "#).is_ok());
}

#[test]
fn t11_nested_transformcolumns() {
    assert!(run_query(r#"
        Table.Group(
            Table.TransformColumns(Source, {{"Salary", each _}}),
            {"Dept"},
            {{"AllRows", each _, type table}}
        )
    "#).is_ok());
}

// ── edge cases ────────────────────────────────────────────────────────────

#[test]
fn t12_empty_table() {
    assert!(run_query(r#"
        let
            Source = Excel.Workbook(File.Contents("dummy.xlsx")),
            Empty = Table.SelectRows(Source, each false)
        in
            Table.Group(Empty, {"Dept"}, {{"AllRows", each _, type table}})
    "#).is_ok());
}

#[test]
fn t13_null_groupkind() {
    assert!(run_query(r#"
        Table.Group(
            Source,
            {"Dept"},
            {{"AllRows", each _, type table}},
            null
        )
    "#).is_ok());
}

#[test]
fn t14_null_comparer() {
    assert!(run_query(r#"
        Table.Group(
            Source,
            {"Dept"},
            {{"AllRows", each _, type table}},
            null,
            null
        )
    "#).is_ok());
}

#[test]
fn t15_single_key_and_single_aggregate_combined() {
    assert!(run_query(r#"
        Table.Group(
            Source,
            "Dept",
            {"AllRows", each _, type table}
        )
    "#).is_ok());
}