//! Conformance fixture runner.
//!
//! Walks `specs/conformance/**/*.json` and executes each fixture against the
//! engine. A fixture pins one rule to one input/expected pair so that any
//! divergence between the spec text and the implementation breaks the build.
//!
//! Fixture format (JSON):
//!
//! ```json
//! {
//!   "id": "F07-SelectRows-001",
//!   "rule": "R-TBL-04",
//!   "description": "Filter retains schema",
//!   "data": { "source": "x.xlsx", "sheet": "S",
//!             "rows": [["A","B"], ["1","x"], ["2","y"]] },
//!   "formula": "let Source = ... in #\"Filtered\"",
//!   "expect_ok": {
//!     "columns": ["A","B"],
//!     "types":   ["Integer","Text"],   // optional
//!     "rows":    [["1","x"]]
//!   }
//! }
//! ```
//!
//! For failure cases use `expect_err` instead:
//!
//! ```json
//! { "expect_err": { "category": "Parse", "contains": "expected" } }
//! ```
//!
//! `data` may be omitted to use the default payload. Either `expect_ok` or
//! `expect_err` must be present, not both.

use serde::Deserialize;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use pq_engine::{Engine, EngineError};
use pq_pipeline::Table;

#[derive(Debug, Deserialize)]
struct Fixture {
    id: String,
    rule: String,
    #[serde(default)]
    #[allow(dead_code)]
    description: String,
    #[serde(default)]
    data: Option<serde_json::Value>,
    formula: String,
    #[serde(default)]
    expect_ok: Option<ExpectOk>,
    #[serde(default)]
    expect_err: Option<ExpectErr>,
}

#[derive(Debug, Deserialize)]
struct ExpectOk {
    #[serde(default)]
    columns: Option<Vec<String>>,
    #[serde(default)]
    types: Option<Vec<String>>,
    #[serde(default)]
    rows: Option<Vec<Vec<String>>>,
    #[serde(default)]
    row_count: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ExpectErr {
    /// One of: Json, Lex, Parse, Diagnostics, Execute
    category: String,
    /// Substring that must appear in the rendered error message.
    #[serde(default)]
    contains: Option<String>,
}

fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("specs")
        .join("conformance")
}

fn collect_fixtures() -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::new();
    let root = fixtures_root();
    if !root.exists() {
        return paths;
    }
    for entry in WalkDir::new(&root).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.extension().and_then(|s| s.to_str()) == Some("json") {
            paths.push(p.to_path_buf());
        }
    }
    paths.sort();
    paths
}

fn run_one(path: &Path) -> Result<(), String> {
    let body = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let f: Fixture = serde_json::from_str(&body)
        .map_err(|e| format!("fixture {} is not valid JSON: {}", path.display(), e))?;

    if f.expect_ok.is_some() == f.expect_err.is_some() {
        return Err(format!(
            "{}: exactly one of expect_ok / expect_err must be set",
            f.id
        ));
    }

    let data_str = match &f.data {
        Some(v) => serde_json::to_string(v).unwrap(),
        None => DEFAULT_PAYLOAD.to_string(),
    };

    let result = Engine::run_with_formula(&data_str, &f.formula);

    match (&f.expect_ok, &f.expect_err, result) {
        (Some(exp), None, Ok(out)) => check_ok(&f, exp, &out.result_table),
        (Some(_),   None, Err(e))  => Err(format!(
            "{} ({}): expected ok, got error: {}",
            f.id, f.rule, e
        )),
        (None, Some(exp), Err(e))  => check_err(&f, exp, &e),
        (None, Some(_),   Ok(out)) => Err(format!(
            "{} ({}): expected error, got ok with {} rows",
            f.id,
            f.rule,
            out.result_table.row_count()
        )),
        _ => unreachable!(),
    }
}

fn check_ok(f: &Fixture, exp: &ExpectOk, t: &Table) -> Result<(), String> {
    if let Some(cols) = &exp.columns {
        let actual: Vec<String> = t.columns.iter().map(|c| c.name.clone()).collect();
        if actual != *cols {
            return Err(format!(
                "{} ({}): columns differ.\n  expected: {:?}\n  actual:   {:?}",
                f.id, f.rule, cols, actual
            ));
        }
    }
    if let Some(types) = &exp.types {
        let actual: Vec<String> =
            t.columns.iter().map(|c| format!("{:?}", c.col_type)).collect();
        if actual != *types {
            return Err(format!(
                "{} ({}): types differ.\n  expected: {:?}\n  actual:   {:?}",
                f.id, f.rule, types, actual
            ));
        }
    }
    if let Some(rc) = exp.row_count {
        if t.row_count() != rc {
            return Err(format!(
                "{} ({}): row count differs. expected {}, actual {}",
                f.id, f.rule, rc, t.row_count()
            ));
        }
    }
    if let Some(rows) = &exp.rows {
        let actual: Vec<Vec<String>> = (0..t.row_count())
            .map(|r| t.columns.iter().map(|c| c.values.get(r).cloned().unwrap_or_default()).collect())
            .collect();
        if actual != *rows {
            return Err(format!(
                "{} ({}): rows differ.\n  expected: {:?}\n  actual:   {:?}",
                f.id, f.rule, rows, actual
            ));
        }
    }
    Ok(())
}

fn check_err(f: &Fixture, exp: &ExpectErr, e: &EngineError) -> Result<(), String> {
    let actual_cat = match e {
        EngineError::Json(_)        => "Json",
        EngineError::Lex(_)         => "Lex",
        EngineError::Parse(_)       => "Parse",
        EngineError::Diagnostics(_) => "Diagnostics",
        EngineError::Execute(_)     => "Execute",
    };
    if actual_cat != exp.category {
        return Err(format!(
            "{} ({}): error category differs. expected {}, actual {}",
            f.id, f.rule, exp.category, actual_cat
        ));
    }
    if let Some(needle) = &exp.contains {
        let rendered = format!("{}", e);
        if !rendered.to_lowercase().contains(&needle.to_lowercase()) {
            return Err(format!(
                "{} ({}): error message missing substring {:?}.\n  actual: {}",
                f.id, f.rule, needle, rendered
            ));
        }
    }
    Ok(())
}

const DEFAULT_PAYLOAD: &str = r#"{
  "source": "data.xlsx",
  "sheet": "Sheet1",
  "rows": [
    ["Name","Age","Salary"],
    ["Alice","30","50000"],
    ["Bob","25","40000"],
    ["Charlie","35","60000"]
  ]
}"#;

#[test]
fn conformance_fixtures_pass() {
    let paths = collect_fixtures();
    assert!(!paths.is_empty(), "no conformance fixtures found under specs/conformance");

    let mut failures: Vec<String> = Vec::new();
    let mut count = 0usize;
    for p in &paths {
        count += 1;
        if let Err(msg) = run_one(p) {
            failures.push(format!("[{}] {}", p.display(), msg));
        }
    }

    if !failures.is_empty() {
        panic!(
            "{} of {} conformance fixtures failed:\n\n{}",
            failures.len(),
            count,
            failures.join("\n\n")
        );
    }
    eprintln!("conformance: {} fixtures passed", count);
}
