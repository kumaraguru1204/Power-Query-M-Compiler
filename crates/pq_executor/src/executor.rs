use std::collections::HashMap;
use std::cell::RefCell;

use pq_ast::{
    Program,
    expr::{Expr, ExprNode},
    step::{StepKind, SortOrder, MissingFieldKind, JoinKind},
    call_arg::CallArg,
};
use pq_grammar::operators::{Operator, UnaryOp};
use pq_pipeline::{Column, Table};
use pq_types::{infer_type, ColumnType};

use crate::value::Value;

// Thread-local override for the implicit `_` (each-parameter) binding.
// When set, `Identifier("_")` / `ColumnAccess("_")` resolves to this value
// instead of doing a column lookup.  Used by list-context operations where
// `_` is bound to the current list element (which may be a record or list,
// not just a scalar string).
thread_local! {
    static IMPLICIT_UNDERSCORE: RefCell<Vec<Value>> = const { RefCell::new(Vec::new()) };
}

fn push_underscore(v: Value) {
    IMPLICIT_UNDERSCORE.with(|u| u.borrow_mut().push(v));
}

fn pop_underscore() {
    IMPLICIT_UNDERSCORE.with(|u| { u.borrow_mut().pop(); });
}

fn current_underscore() -> Option<Value> {
    IMPLICIT_UNDERSCORE.with(|u| u.borrow().last().cloned())
}

// ── errors ────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum ExecError {
    UnknownStep(String),
    DivisionByZero,
    TypeMismatch(String),
}

impl std::fmt::Display for ExecError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ExecError::UnknownStep(s)  => write!(f, "unknown step '{}'", s),
            ExecError::DivisionByZero  => write!(f, "division by zero"),
            ExecError::TypeMismatch(s) => write!(f, "type mismatch: {}", s),
        }
    }
}

pub type ExecResult<T> = Result<T, ExecError>;

// ── executor ──────────────────────────────────────────────────────────────

pub struct Executor;

impl Executor {
    /// Execute a complete Program against a source Table.
    ///
    /// Each StepBinding is evaluated in order.
    /// The result of each step is stored in a map keyed by step name.
    /// The final result is the table produced by the `output` step.
    ///
    /// # Example
    /// ```ignore
    /// let output = Engine::run_with_formula(json, formula)?;
    /// let result = Executor::execute(&output.program, output.table)?;
    /// println!("{}", result);  // the transformed table
    /// ```
    pub fn execute(program: &Program, source: Table) -> ExecResult<Table> {
        // step_name → resulting Table after that step ran
        let mut env: HashMap<String, Table> = HashMap::new();

        for binding in &program.steps {
            let result = Self::run_step(&binding.step.kind, &env, &source)?;
            env.insert(binding.name.clone(), result);
        }

        // Non-identifier `in` expression: evaluate it against the env.
        if let Some(expr) = &program.output_expr {
            return Self::eval_in_expression(expr, &env, &source);
        }

        env.remove(&program.output)
            .ok_or_else(|| ExecError::UnknownStep(program.output.clone()))
    }

    fn eval_in_expression(
        expr:   &ExprNode,
        env:    &HashMap<String, Table>,
        source: &Table,
    ) -> ExecResult<Table> {
        let span = expr.span.clone();
        match &expr.expr {
            // List.Select / List.Transform / List.RemoveItems / List.Difference / List.Intersect inside `in` — dispatch via FunctionCall.
            Expr::FunctionCall { name, args }
                if (name == "List.Select" || name == "List.Transform" || name == "List.RemoveItems" || name == "List.Difference" || name == "List.Intersect") && args.len() >= 1 =>
            {
                let call_args: Vec<CallArg> = args.iter()
                    .map(|a| CallArg::Expr(a.clone()))
                    .collect();
                let synthetic = StepKind::FunctionCall { name: name.clone(), args: call_args };
                Self::run_step(&synthetic, env, source)
            }
            _ => {
                let v = Self::eval_expr(expr, source, 0)?;
                let values: Vec<String> = match &v {
                    Value::List(items) => items.iter().map(|x| x.to_raw_string()).collect(),
                    other              => vec![other.to_raw_string()],
                };
                let col_type = infer_type(&values);
                Ok(Table {
                    source:  source.source.clone(),
                    sheet:   source.sheet.clone(),
                    columns: vec![Column { name: "Value".into(), col_type, values }],
                })
            }
        }
        .map(|t| { let _ = span; t })
    }

    // ── step dispatch ─────────────────────────────────────────────────────

    fn run_step(
        kind:   &StepKind,
        env:    &HashMap<String, Table>,
        source: &Table,
    ) -> ExecResult<Table> {
        match kind {
            StepKind::Source { path, .. } => {
                let mut t = source.clone();
                t.source  = path.clone();
                Ok(t)
            }
            StepKind::NavigateSheet { input, .. } => {
                Ok(Self::lookup(input, env, source)?.clone())
            }
            StepKind::ValueBinding { expr } => {
                let value  = Self::eval_expr(expr, source, 0)?;
                let values: Vec<String> = match &value {
                    Value::List(items) => items.iter().map(|v| v.to_raw_string()).collect(),
                    other              => vec![other.to_raw_string()],
                };
                let col_type = infer_type(&values);
                Ok(Table {
                    source:  source.source.clone(),
                    sheet:   source.sheet.clone(),
                    columns: vec![Column { name: "Value".into(), col_type, values }],
                })
            }
            StepKind::FunctionCall { name, args } => {
                Self::eval_function_call(name, args, env, source)
            }
        }
    }

    fn eval_function_call(
        name:   &str,
        args:   &[CallArg],
        env:    &HashMap<String, Table>,
        source: &Table,
    ) -> ExecResult<Table> {
        let input_name = args.first().and_then(|a| a.as_step_ref()).unwrap_or("");
        match name {

            // ── Passthrough functions ─────────────────────────────────────
            "Table.PromoteHeaders"
            | "Table.Buffer"
            | "Table.StopFolding"
            | "Table.ConformToPageReader"
            | "Table.SingleRow"
            | "Table.Keys"
            | "Table.PartitionKey"
            | "Table.ReplaceKeys"
            | "Table.ReplacePartitionKey"
            | "Table.ApproximateRowCount"
            | "Table.AddKey"
            | "Table.FromPartitions" => {
                Ok(Self::lookup(input_name, env, source)?.clone())
            }

            // ── Conversion passthrough (via env) ──────────────────────────
            "Table.ToColumns"
            | "Table.ToList"
            | "Table.ToRecords"
            | "Table.ToRows"
            | "Table.PartitionValues"
            | "Table.Profile" => {
                Ok(env.get(input_name).cloned().unwrap_or_else(|| source.clone()))
            }

            // ── Construction (no source input) ────────────────────────────
            "Table.FromColumns"
            | "Table.FromList"
            | "Table.FromRecords"
            | "Table.FromRows"
            | "Table.FromValue" => Ok(source.clone()),

            // ── Table.Column ──────────────────────────────────────────────
            "Table.Column" => {
                let col_name = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                let t = env.get(input_name).cloned().unwrap_or_else(|| source.clone());
                if let Some(col) = t.columns.iter().find(|c| c.name == col_name).cloned() {
                    Ok(Table { source: t.source, sheet: t.sheet, columns: vec![col] })
                } else {
                    Ok(Table { source: t.source, sheet: t.sheet, columns: vec![] })
                }
            }

            // ── TransformColumnTypes ──────────────────────────────────────
            "Table.TransformColumnTypes" => {
                let mut t = Self::lookup(input_name, env, source)?.clone();
                let row_count = t.row_count();
                let columns = args.get(1).and_then(|a| a.as_type_list()).unwrap_or(&[]);
                let missing_field = args.get(2)
                    .and_then(|a| a.as_opt_culture())
                    .and_then(|(_, mf)| mf.as_ref());
                for (col_name, new_type) in columns {
                    if t.columns.iter().any(|c| c.name == *col_name) {
                        if let Some(col) = t.columns.iter_mut().find(|c| c.name == *col_name) {
                            col.col_type = new_type.clone();
                        }
                    } else {
                        let _ = Self::handle_missing_field(
                            &mut t.columns, col_name, new_type.clone(),
                            missing_field, row_count, "Table.TransformColumnTypes",
                        )?;
                    }
                }
                Ok(t)
            }

            // ── SelectRows ────────────────────────────────────────────────
            "Table.SelectRows" => {
                let t = Self::lookup(input_name, env, source)?;
                let row_count = t.row_count();
                if let Some(cond) = args.get(1).and_then(|a| a.as_expr()) {
                    let keep: Vec<usize> = (0..row_count)
                        .filter(|&i| matches!(Self::eval_expr(cond, t, i), Ok(Value::Bool(true))))
                        .collect();
                    Ok(Self::select_rows(t, &keep))
                } else {
                    Ok(t.clone())
                }
            }

            // ── AddColumn ─────────────────────────────────────────────────
            "Table.AddColumn" => {
                let col_name   = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                let expression = args.get(2).and_then(|a| a.as_expr());
                let t          = Self::lookup(input_name, env, source)?;
                let row_count  = t.row_count();
                let values: Vec<String> = if let Some(expr) = expression {
                    (0..row_count)
                        .map(|i| Self::eval_expr(expr, t, i).map(|v| v.to_raw_string()).unwrap_or_default())
                        .collect()
                } else {
                    vec![String::new(); row_count]
                };
                let col_type = infer_type(&values);
                let mut result = t.clone();
                result.columns.push(Column { name: col_name.to_string(), col_type, values });
                Ok(result)
            }

            // ── RemoveColumns ─────────────────────────────────────────────
            "Table.RemoveColumns" => {
                let columns       = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                let missing_field = args.get(2).and_then(|a| a.as_opt_missing_field()).flatten();
                let mut t = Self::lookup(input_name, env, source)?.clone();
                if matches!(missing_field, Some(MissingFieldKind::Ignore)) {
                    t.columns.retain(|c| !columns.contains(&c.name));
                } else {
                    for col in columns {
                        if !t.columns.iter().any(|c| &c.name == col) {
                            return Err(ExecError::TypeMismatch(format!(
                                "Table.RemoveColumns: column '{}' does not exist in the table", col
                            )));
                        }
                    }
                    t.columns.retain(|c| !columns.contains(&c.name));
                }
                Ok(t)
            }

            // ── RenameColumns ─────────────────────────────────────────────
            "Table.RenameColumns" => {
                let renames = args.get(1).and_then(|a| a.as_rename_list()).unwrap_or(&[]);
                let mut t = Self::lookup(input_name, env, source)?.clone();
                for col in t.columns.iter_mut() {
                    if let Some((_, new)) = renames.iter().find(|(old, _)| *old == col.name) {
                        col.name = new.clone();
                    }
                }
                Ok(t)
            }

            // ── Sort ──────────────────────────────────────────────────────
            "Table.Sort" => {
                let by        = args.get(1).and_then(|a| a.as_sort_list()).unwrap_or(&[]);
                let t         = Self::lookup(input_name, env, source)?;
                let row_count = t.row_count();
                let mut indices: Vec<usize> = (0..row_count).collect();
                indices.sort_by(|&a, &b| {
                    for (col_name, order) in by {
                        let av = Self::cell(t, col_name, a);
                        let bv = Self::cell(t, col_name, b);
                        let ct = t.get_column(col_name).map(|c| &c.col_type).unwrap_or(&ColumnType::Text);
                        let cmp = Self::compare_raw(av, bv, ct);
                        let cmp = match order {
                            SortOrder::Ascending  => cmp,
                            SortOrder::Descending => cmp.reverse(),
                        };
                        if cmp != std::cmp::Ordering::Equal { return cmp; }
                    }
                    std::cmp::Ordering::Equal
                });
                Ok(Self::select_rows(t, &indices))
            }

            // ── TransformColumns ──────────────────────────────────────────
            "Table.TransformColumns" => {
                let transforms        = args.get(1).and_then(|a| a.as_transform_list()).unwrap_or(&[]);
                let default_transform = args.get(2).and_then(|a| a.as_expr());
                let missing_field     = args.get(3).and_then(|a| a.as_opt_missing_field()).flatten();
                let t         = Self::lookup(input_name, env, source)?;
                let row_count = t.row_count();
                let mut result = t.clone();
                let transform_cols: std::collections::HashSet<&str> =
                    transforms.iter().map(|(n, _, _)| n.as_str()).collect();
                for (col_name, expr, _col_type) in transforms {
                    if let Some(col) = result.columns.iter_mut().find(|c| c.name == *col_name) {
                        col.values = (0..row_count)
                            .map(|i| Self::eval_expr(expr, t, i).map(|v| v.to_raw_string()).unwrap_or_default())
                            .collect();
                    } else {
                        let added = Self::handle_missing_field(
                            &mut result.columns, col_name, ColumnType::Text,
                            missing_field, row_count, "Table.TransformColumns",
                        )?;
                        if added {
                            if let Some(col) = result.columns.iter_mut().find(|c| c.name == *col_name) {
                                col.values = (0..row_count)
                                    .map(|i| Self::eval_expr(expr, t, i).map(|v| v.to_raw_string()).unwrap_or_default())
                                    .collect();
                            }
                        }
                    }
                }
                if let Some(def_expr) = default_transform {
                    for col in result.columns.iter_mut() {
                        if !transform_cols.contains(col.name.as_str()) {
                            col.values = (0..row_count)
                                .map(|i| Self::apply_transform(def_expr, col.values.get(i).map(String::as_str).unwrap_or(""), source)
                                    .unwrap_or_default())
                                .collect();
                        }
                    }
                }
                Ok(result)
            }

            // ── Group ─────────────────────────────────────────────────────
            "Table.Group" | "Table.FuzzyGroup" => {
                let by         = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                let aggregates = args.get(2).and_then(|a| a.as_agg_list()).unwrap_or(&[]);
                let t          = Self::lookup(input_name, env, source)?;
                let row_count  = t.row_count();
                let mut key_order: Vec<Vec<String>> = Vec::new();
                let mut key_rows: HashMap<Vec<String>, Vec<usize>> = HashMap::new();
                for i in 0..row_count {
                    let key: Vec<String> = by.iter().map(|col| Self::cell(t, col, i).to_string()).collect();
                    let entry = key_rows.entry(key.clone()).or_insert_with(|| {
                        key_order.push(key.clone());
                        Vec::new()
                    });
                    entry.push(i);
                }
                let mut out_cols: Vec<Column> = by.iter().map(|col_name| {
                    let col_type = t.get_column(col_name).map(|c| c.col_type.clone()).unwrap_or(ColumnType::Text);
                    Column {
                        name: col_name.clone(), col_type,
                        values: key_order.iter()
                            .map(|k| k[by.iter().position(|c| c == col_name).unwrap()].clone())
                            .collect(),
                    }
                }).collect();
                for agg in aggregates {
                    let values: Vec<String> = key_order.iter().map(|key| {
                        let row_indices = &key_rows[key];
                        row_indices.first()
                            .and_then(|&i| Self::eval_expr(&agg.expression, t, i).ok())
                            .map(|v| v.to_raw_string())
                            .unwrap_or_default()
                    }).collect();
                    out_cols.push(Column { name: agg.name.clone(), col_type: agg.col_type.clone(), values });
                }
                Ok(Table { source: t.source.clone(), sheet: t.sheet.clone(), columns: out_cols })
            }

            // ── FirstN ────────────────────────────────────────────────────
            "Table.FirstN" => {
                let t  = Self::lookup(input_name, env, source)?;
                let n  = args.get(1).and_then(|a| a.as_int()).unwrap_or(1) as usize;
                let rc = t.row_count();
                Ok(Self::select_rows(t, &(0..n.min(rc)).collect::<Vec<_>>()))
            }

            // ── LastN ─────────────────────────────────────────────────────
            "Table.LastN" => {
                let t     = Self::lookup(input_name, env, source)?;
                let n     = args.get(1).and_then(|a| a.as_int()).unwrap_or(1) as usize;
                let rc    = t.row_count();
                let start = rc.saturating_sub(n);
                Ok(Self::select_rows(t, &(start..rc).collect::<Vec<_>>()))
            }

            // ── Skip ──────────────────────────────────────────────────────
            "Table.Skip" => {
                let t  = Self::lookup(input_name, env, source)?;
                let n  = args.get(1).and_then(|a| a.as_int()).unwrap_or(1) as usize;
                let rc = t.row_count();
                Ok(Self::select_rows(t, &(n.min(rc)..rc).collect::<Vec<_>>()))
            }

            // ── Range ─────────────────────────────────────────────────────
            "Table.Range" => {
                let t   = Self::lookup(input_name, env, source)?;
                let off = args.get(1).and_then(|a| a.as_int()).unwrap_or(0) as usize;
                let cnt = args.get(2).and_then(|a| a.as_int()).unwrap_or(1) as usize;
                let rc  = t.row_count();
                Ok(Self::select_rows(t, &(off.min(rc)..(off + cnt).min(rc)).collect::<Vec<_>>()))
            }

            // ── RemoveFirstN ──────────────────────────────────────────────
            "Table.RemoveFirstN" => {
                let t  = Self::lookup(input_name, env, source)?;
                let n  = args.get(1).and_then(|a| a.as_int()).unwrap_or(1) as usize;
                let rc = t.row_count();
                Ok(Self::select_rows(t, &(n.min(rc)..rc).collect::<Vec<_>>()))
            }

            // ── RemoveLastN ───────────────────────────────────────────────
            "Table.RemoveLastN" => {
                let t   = Self::lookup(input_name, env, source)?;
                let n   = args.get(1).and_then(|a| a.as_int()).unwrap_or(1) as usize;
                let rc  = t.row_count();
                let end = rc.saturating_sub(n);
                Ok(Self::select_rows(t, &(0..end).collect::<Vec<_>>()))
            }

            // ── RemoveRows ────────────────────────────────────────────────
            "Table.RemoveRows" => {
                let t   = Self::lookup(input_name, env, source)?;
                let off = args.get(1).and_then(|a| a.as_int()).unwrap_or(0) as usize;
                let cnt = args.get(2).and_then(|a| a.as_int()).unwrap_or(1) as usize;
                let rc  = t.row_count();
                let indices: Vec<usize> = (0..rc).filter(|&i| i < off || i >= off + cnt).collect();
                Ok(Self::select_rows(t, &indices))
            }

            // ── ReverseRows ───────────────────────────────────────────────
            "Table.ReverseRows" => {
                let t  = Self::lookup(input_name, env, source)?;
                let rc = t.row_count();
                Ok(Self::select_rows(t, &(0..rc).rev().collect::<Vec<_>>()))
            }

            // ── Distinct ──────────────────────────────────────────────────
            "Table.Distinct" => {
                let columns = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                let t  = Self::lookup(input_name, env, source)?;
                let rc = t.row_count();
                let mut seen = std::collections::HashSet::new();
                let mut indices: Vec<usize> = Vec::new();
                for i in 0..rc {
                    let key: Vec<String> = if columns.is_empty() {
                        t.columns.iter().map(|c| c.values.get(i).cloned().unwrap_or_default()).collect()
                    } else {
                        columns.iter().map(|col| Self::cell(t, col, i).to_string()).collect()
                    };
                    if seen.insert(key) { indices.push(i); }
                }
                Ok(Self::select_rows(t, &indices))
            }

            // ── Repeat ────────────────────────────────────────────────────
            "Table.Repeat" => {
                let t  = Self::lookup(input_name, env, source)?;
                let n  = args.get(1).and_then(|a| a.as_int()).unwrap_or(1) as usize;
                let rc = t.row_count();
                let indices: Vec<usize> = (0..n).flat_map(|_| 0..rc).collect();
                Ok(Self::select_rows(t, &indices))
            }

            // ── AlternateRows ─────────────────────────────────────────────
            "Table.AlternateRows" => {
                let t   = Self::lookup(input_name, env, source)?;
                let off = args.get(1).and_then(|a| a.as_int()).unwrap_or(0) as usize;
                let sk  = args.get(2).and_then(|a| a.as_int()).unwrap_or(1) as usize;
                let tk  = args.get(3).and_then(|a| a.as_int()).unwrap_or(1) as usize;
                let rc  = t.row_count();
                let period = sk + tk;
                let indices: Vec<usize> = (0..rc)
                    .filter(|&i| i >= off && (i - off) % period < tk)
                    .collect();
                Ok(Self::select_rows(t, &indices))
            }

            // ── FindText ──────────────────────────────────────────────────
            "Table.FindText" => {
                let text   = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                let t      = Self::lookup(input_name, env, source)?;
                let rc     = t.row_count();
                let needle = text.to_lowercase();
                let indices: Vec<usize> = (0..rc)
                    .filter(|&i| t.columns.iter().any(|c| {
                        c.values.get(i).map(|v| v.to_lowercase().contains(&needle)).unwrap_or(false)
                    }))
                    .collect();
                Ok(Self::select_rows(t, &indices))
            }

            // ── FillDown ──────────────────────────────────────────────────
            "Table.FillDown" => {
                let columns = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                let mut t   = Self::lookup(input_name, env, source)?.clone();
                for col_name in columns {
                    if let Some(col) = t.columns.iter_mut().find(|c| c.name == *col_name) {
                        let mut last = String::new();
                        for val in col.values.iter_mut() {
                            if val.is_empty() || val == "null" { *val = last.clone(); }
                            else { last = val.clone(); }
                        }
                    }
                }
                Ok(t)
            }

            // ── FillUp ────────────────────────────────────────────────────
            "Table.FillUp" => {
                let columns = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                let mut t   = Self::lookup(input_name, env, source)?.clone();
                for col_name in columns {
                    if let Some(col) = t.columns.iter_mut().find(|c| c.name == *col_name) {
                        let mut last = String::new();
                        for val in col.values.iter_mut().rev() {
                            if val.is_empty() || val == "null" { *val = last.clone(); }
                            else { last = val.clone(); }
                        }
                    }
                }
                Ok(t)
            }

            // ── AddIndexColumn ────────────────────────────────────────────
            "Table.AddIndexColumn" => {
                let col_name = args.get(1).and_then(|a| a.as_str()).unwrap_or("Index");
                let start    = args.get(2).and_then(|a| a.as_int()).unwrap_or(0);
                let step     = args.get(3).and_then(|a| a.as_int()).unwrap_or(1);
                let t        = Self::lookup(input_name, env, source)?;
                let rc       = t.row_count();
                let values: Vec<String> = (0..rc).map(|i| (start + (i as i64) * step).to_string()).collect();
                let mut result = t.clone();
                result.columns.push(Column { name: col_name.to_string(), col_type: ColumnType::Integer, values });
                Ok(result)
            }

            // ── DuplicateColumn ───────────────────────────────────────────
            "Table.DuplicateColumn" => {
                let src_col = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                let new_col = args.get(2).and_then(|a| a.as_str()).unwrap_or("");
                let t       = Self::lookup(input_name, env, source)?;
                let src_values = t.get_column(src_col).map(|c| c.values.clone()).unwrap_or_default();
                let src_type   = t.get_column(src_col).map(|c| c.col_type.clone()).unwrap_or(ColumnType::Text);
                let mut result = t.clone();
                result.columns.push(Column { name: new_col.to_string(), col_type: src_type, values: src_values });
                Ok(result)
            }

            // ── Unpivot ───────────────────────────────────────────────────
            "Table.Unpivot" => {
                let columns  = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                let attr_col = args.get(2).and_then(|a| a.as_str()).unwrap_or("Attribute");
                let val_col  = args.get(3).and_then(|a| a.as_str()).unwrap_or("Value");
                let t        = Self::lookup(input_name, env, source)?;
                let rc       = t.row_count();
                let keep_cols: Vec<&Column>    = t.columns.iter().filter(|c| !columns.contains(&c.name)).collect();
                let unpivot_cols: Vec<&Column> = t.columns.iter().filter(|c|  columns.contains(&c.name)).collect();
                let mut out_keep: Vec<Column>  = keep_cols.iter().map(|c| Column { name: c.name.clone(), col_type: c.col_type.clone(), values: vec![] }).collect();
                let mut out_attr = Column { name: attr_col.to_string(), col_type: ColumnType::Text, values: vec![] };
                let mut out_val  = Column { name: val_col.to_string(),  col_type: ColumnType::Text, values: vec![] };
                for i in 0..rc {
                    for uc in &unpivot_cols {
                        for (j, kc) in keep_cols.iter().enumerate() {
                            out_keep[j].values.push(kc.values.get(i).cloned().unwrap_or_default());
                        }
                        out_attr.values.push(uc.name.clone());
                        out_val.values.push(uc.values.get(i).cloned().unwrap_or_default());
                    }
                }
                let mut result_cols = out_keep;
                result_cols.push(out_attr);
                result_cols.push(out_val);
                Ok(Table { source: t.source.clone(), sheet: t.sheet.clone(), columns: result_cols })
            }

            // ── UnpivotOtherColumns ───────────────────────────────────────
            "Table.UnpivotOtherColumns" => {
                let keep_cols = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                let attr_col  = args.get(2).and_then(|a| a.as_str()).unwrap_or("Attribute");
                let val_col   = args.get(3).and_then(|a| a.as_str()).unwrap_or("Value");
                let t         = Self::lookup(input_name, env, source)?;
                let unpivot_names: Vec<String> = t.columns.iter()
                    .filter(|c| !keep_cols.contains(&c.name))
                    .map(|c| c.name.clone())
                    .collect();
                let rc = t.row_count();
                let kept: Vec<&Column>      = t.columns.iter().filter(|c|  keep_cols.contains(&c.name)).collect();
                let unpivoted: Vec<&Column> = t.columns.iter().filter(|c| unpivot_names.contains(&c.name)).collect();
                let mut out_keep: Vec<Column> = kept.iter().map(|c| Column { name: c.name.clone(), col_type: c.col_type.clone(), values: vec![] }).collect();
                let mut out_attr = Column { name: attr_col.to_string(), col_type: ColumnType::Text, values: vec![] };
                let mut out_val  = Column { name: val_col.to_string(),  col_type: ColumnType::Text, values: vec![] };
                for i in 0..rc {
                    for uc in &unpivoted {
                        for (j, kc) in kept.iter().enumerate() {
                            out_keep[j].values.push(kc.values.get(i).cloned().unwrap_or_default());
                        }
                        out_attr.values.push(uc.name.clone());
                        out_val.values.push(uc.values.get(i).cloned().unwrap_or_default());
                    }
                }
                let mut result_cols = out_keep;
                result_cols.push(out_attr);
                result_cols.push(out_val);
                Ok(Table { source: t.source.clone(), sheet: t.sheet.clone(), columns: result_cols })
            }

            // ── Transpose ─────────────────────────────────────────────────
            "Table.Transpose" => {
                let t  = Self::lookup(input_name, env, source)?;
                let rc = t.row_count();
                let cc = t.columns.len();
                let mut result_cols: Vec<Column> = (0..rc)
                    .map(|i| Column { name: format!("Column{}", i + 1), col_type: ColumnType::Text, values: vec![] })
                    .collect();
                for col in &t.columns {
                    for (i, val) in col.values.iter().enumerate() {
                        if i < result_cols.len() { result_cols[i].values.push(val.clone()); }
                    }
                }
                for rc_col in result_cols.iter_mut() {
                    while rc_col.values.len() < cc { rc_col.values.push(String::new()); }
                }
                Ok(Table { source: t.source.clone(), sheet: t.sheet.clone(), columns: result_cols })
            }

            // ── Combine ───────────────────────────────────────────────────
            "Table.Combine" => {
                let inputs = args.first().and_then(|a| a.as_step_ref_list()).unwrap_or(&[]);
                if inputs.is_empty() {
                    return Ok(Table { source: source.source.clone(), sheet: source.sheet.clone(), columns: vec![] });
                }
                let first = Self::lookup(&inputs[0], env, source)?;
                let mut result = first.clone();
                for inp in &inputs[1..] {
                    let t = Self::lookup(inp, env, source)?;
                    for col in result.columns.iter_mut() {
                        if let Some(src_col) = t.get_column(&col.name) {
                            col.values.extend(src_col.values.iter().cloned());
                        } else {
                            col.values.extend(vec!["".to_string(); t.row_count()]);
                        }
                    }
                }
                Ok(result)
            }

            // ── RemoveRowsWithErrors ──────────────────────────────────────
            "Table.RemoveRowsWithErrors" => {
                let columns = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                let t  = Self::lookup(input_name, env, source)?;
                let rc = t.row_count();
                let indices: Vec<usize> = (0..rc)
                    .filter(|&i| !columns.iter().any(|cn| Self::cell(t, cn, i).is_empty()))
                    .collect();
                Ok(Self::select_rows(t, &indices))
            }

            // ── SelectRowsWithErrors ──────────────────────────────────────
            "Table.SelectRowsWithErrors" => {
                let columns = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                let t  = Self::lookup(input_name, env, source)?;
                let rc = t.row_count();
                let indices: Vec<usize> = (0..rc)
                    .filter(|&i| columns.iter().any(|cn| Self::cell(t, cn, i).is_empty()))
                    .collect();
                Ok(Self::select_rows(t, &indices))
            }

            // ── TransformRows ─────────────────────────────────────────────
            "Table.TransformRows" => {
                Ok(Self::lookup(input_name, env, source)?.clone())
            }

            // ── MatchesAllRows ────────────────────────────────────────────
            "Table.MatchesAllRows" => {
                let t  = Self::lookup(input_name, env, source)?;
                let rc = t.row_count();
                let result = if let Some(cond) = args.get(1).and_then(|a| a.as_expr()) {
                    (0..rc).all(|i| matches!(Self::eval_expr(cond, t, i), Ok(Value::Bool(true))))
                } else { false };
                Ok(Table { source: t.source.clone(), sheet: t.sheet.clone(),
                    columns: vec![Column { name: "Value".into(), col_type: ColumnType::Boolean, values: vec![result.to_string()] }] })
            }

            // ── MatchesAnyRows ────────────────────────────────────────────
            "Table.MatchesAnyRows" => {
                let t  = Self::lookup(input_name, env, source)?;
                let rc = t.row_count();
                let result = if let Some(cond) = args.get(1).and_then(|a| a.as_expr()) {
                    (0..rc).any(|i| matches!(Self::eval_expr(cond, t, i), Ok(Value::Bool(true))))
                } else { false };
                Ok(Table { source: t.source.clone(), sheet: t.sheet.clone(),
                    columns: vec![Column { name: "Value".into(), col_type: ColumnType::Boolean, values: vec![result.to_string()] }] })
            }

            // ── PrefixColumns ─────────────────────────────────────────────
            "Table.PrefixColumns" => {
                let prefix = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                let mut t  = Self::lookup(input_name, env, source)?.clone();
                for col in t.columns.iter_mut() {
                    col.name = format!("{}.{}", prefix, col.name);
                }
                Ok(t)
            }

            // ── DemoteHeaders ─────────────────────────────────────────────
            "Table.DemoteHeaders" => {
                let t = Self::lookup(input_name, env, source)?;
                let mut result_cols: Vec<Column> = Vec::new();
                for (i, col) in t.columns.iter().enumerate() {
                    let mut values = vec![col.name.clone()];
                    values.extend(col.values.iter().cloned());
                    result_cols.push(Column { name: format!("Column{}", i + 1), col_type: ColumnType::Text, values });
                }
                Ok(Table { source: t.source.clone(), sheet: t.sheet.clone(), columns: result_cols })
            }

            // ── SelectColumns ─────────────────────────────────────────────
            "Table.SelectColumns" => {
                let columns       = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                let missing_field = args.get(2).and_then(|a| a.as_opt_missing_field()).flatten();
                let t         = Self::lookup(input_name, env, source)?;
                let row_count = t.row_count();
                let mut result_cols: Vec<Column> = Vec::new();
                for col_name in columns {
                    match t.get_column(col_name) {
                        Some(col) => result_cols.push(col.clone()),
                        None => {
                            let added = Self::handle_missing_field(
                                &mut result_cols, col_name, ColumnType::Text,
                                missing_field, row_count, "Table.SelectColumns",
                            )?;
                            let _ = added;
                        }
                    }
                }
                Ok(Table { source: t.source.clone(), sheet: t.sheet.clone(), columns: result_cols })
            }

            // ── ReorderColumns ────────────────────────────────────────────
            "Table.ReorderColumns" => {
                let columns = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                let t       = Self::lookup(input_name, env, source)?;
                let mut result_cols: Vec<Column> = columns.iter()
                    .filter_map(|col_name| t.get_column(col_name).cloned())
                    .collect();
                for col in &t.columns {
                    if !columns.contains(&col.name) { result_cols.push(col.clone()); }
                }
                Ok(Table { source: t.source.clone(), sheet: t.sheet.clone(), columns: result_cols })
            }

            // ── TransformColumnNames ──────────────────────────────────────
            "Table.TransformColumnNames" => {
                let transform = args.get(1).and_then(|a| a.as_expr());
                let t         = Self::lookup(input_name, env, source)?;
                let mut result = t.clone();
                if let Some(transform_expr) = transform {
                    for col in result.columns.iter_mut() {
                        let name_table = Table {
                            source: t.source.clone(), sheet: t.sheet.clone(),
                            columns: vec![Column { name: "_".into(), col_type: ColumnType::Text, values: vec![col.name.clone()] }],
                        };
                        if let Ok(v) = Self::eval_expr(transform_expr, &name_table, 0) {
                            col.name = v.to_raw_string();
                        }
                    }
                }
                Ok(result)
            }

            // ── CombineColumns ────────────────────────────────────────────
            "Table.CombineColumns" => {
                let columns  = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                let combiner = args.get(2).and_then(|a| a.as_expr());
                let new_col  = args.get(3).and_then(|a| a.as_str()).unwrap_or("Combined");
                let t        = Self::lookup(input_name, env, source)?;
                let rc       = t.row_count();
                let values: Vec<String> = (0..rc).map(|i| {
                    let parts: Vec<String> = columns.iter().map(|c| Self::cell(t, c, i).to_string()).collect();
                    let combined = parts.join(", ");
                    if let Some(comb) = combiner {
                        let synthetic = Table {
                            source: t.source.clone(), sheet: t.sheet.clone(),
                            columns: vec![Column { name: "_".into(), col_type: ColumnType::Text, values: vec![combined] }],
                        };
                        Self::eval_expr(comb, &synthetic, 0).map(|v| v.to_raw_string()).unwrap_or_default()
                    } else {
                        parts.join("")
                    }
                }).collect();
                let mut result = t.clone();
                result.columns.retain(|c| !columns.contains(&c.name));
                result.columns.push(Column { name: new_col.to_string(), col_type: ColumnType::Text, values });
                Ok(result)
            }

            // ── SplitColumn ───────────────────────────────────────────────
            "Table.SplitColumn" => {
                let col_name = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                let splitter = args.get(2).and_then(|a| a.as_expr());
                let t        = Self::lookup(input_name, env, source)?;
                let rc       = t.row_count();
                let mut max_parts = 0usize;
                let split_results: Vec<Vec<String>> = (0..rc).map(|i| {
                    let cell_val = Self::cell(t, col_name, i).to_string();
                    let result_str = if let Some(spl) = splitter {
                        let synthetic = Table {
                            source: t.source.clone(), sheet: t.sheet.clone(),
                            columns: vec![Column { name: "_".into(), col_type: ColumnType::Text, values: vec![cell_val] }],
                        };
                        Self::eval_expr(spl, &synthetic, 0).map(|v| v.to_raw_string()).unwrap_or_default()
                    } else { String::new() };
                    let parts: Vec<String> = result_str.split(',').map(|s| s.to_string()).collect();
                    if parts.len() > max_parts { max_parts = parts.len(); }
                    parts
                }).collect();
                let mut result = t.clone();
                result.columns.retain(|c| c.name != col_name);
                for p in 0..max_parts {
                    let values: Vec<String> = split_results.iter().map(|parts| parts.get(p).cloned().unwrap_or_default()).collect();
                    result.columns.push(Column { name: format!("{}.{}", col_name, p + 1), col_type: ColumnType::Text, values });
                }
                Ok(result)
            }

            // ── ExpandTableColumn / ExpandRecordColumn ────────────────────
            "Table.ExpandTableColumn" | "Table.ExpandRecordColumn" => {
                let col_name = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                let columns  = args.get(2).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                let t        = Self::lookup(input_name, env, source)?;
                let rc       = t.row_count();
                let mut result = t.clone();
                result.columns.retain(|c| c.name != col_name);
                for c in columns {
                    result.columns.push(Column { name: c.clone(), col_type: ColumnType::Text, values: vec![String::new(); rc] });
                }
                Ok(result)
            }

            // ── Pivot ─────────────────────────────────────────────────────
            "Table.Pivot" => {
                let _pivot_col = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                let attr_col   = args.get(2).and_then(|a| a.as_str()).unwrap_or("Attribute");
                let val_col    = args.get(3).and_then(|a| a.as_str()).unwrap_or("Value");
                let t          = Self::lookup(input_name, env, source)?;
                let rc         = t.row_count();
                let keep_cols: Vec<&Column> = t.columns.iter()
                    .filter(|c| c.name != attr_col && c.name != val_col)
                    .collect();
                let mut attr_values: Vec<String> = Vec::new();
                if let Some(attr_c) = t.get_column(attr_col) {
                    for v in &attr_c.values {
                        if !attr_values.contains(v) { attr_values.push(v.clone()); }
                    }
                }
                let mut key_order: Vec<Vec<String>> = Vec::new();
                let mut key_rows: HashMap<Vec<String>, HashMap<String, String>> = HashMap::new();
                for i in 0..rc {
                    let key: Vec<String> = keep_cols.iter().map(|c| c.values.get(i).cloned().unwrap_or_default()).collect();
                    let attr = Self::cell(t, attr_col, i).to_string();
                    let val  = Self::cell(t, val_col, i).to_string();
                    let entry = key_rows.entry(key.clone()).or_insert_with(|| { key_order.push(key.clone()); HashMap::new() });
                    entry.insert(attr, val);
                }
                let mut out_cols: Vec<Column> = keep_cols.iter().enumerate().map(|(j, c)| Column {
                    name: c.name.clone(), col_type: c.col_type.clone(),
                    values: key_order.iter().map(|k| k[j].clone()).collect(),
                }).collect();
                for attr in &attr_values {
                    let values: Vec<String> = key_order.iter().map(|k| key_rows[k].get(attr).cloned().unwrap_or_default()).collect();
                    out_cols.push(Column { name: attr.clone(), col_type: ColumnType::Text, values });
                }
                Ok(Table { source: t.source.clone(), sheet: t.sheet.clone(), columns: out_cols })
            }

            // ── RowCount ──────────────────────────────────────────────────
            "Table.RowCount" => {
                let t = Self::lookup(input_name, env, source)?;
                Ok(Table { source: t.source.clone(), sheet: t.sheet.clone(),
                    columns: vec![Column { name: "Value".into(), col_type: ColumnType::Integer, values: vec![t.row_count().to_string()] }] })
            }

            // ── ColumnCount ───────────────────────────────────────────────
            "Table.ColumnCount" => {
                let t = Self::lookup(input_name, env, source)?;
                Ok(Table { source: t.source.clone(), sheet: t.sheet.clone(),
                    columns: vec![Column { name: "Value".into(), col_type: ColumnType::Integer, values: vec![t.columns.len().to_string()] }] })
            }

            // ── ColumnNames ───────────────────────────────────────────────
            "Table.ColumnNames" => {
                let t = Self::lookup(input_name, env, source)?;
                let names: Vec<String> = t.columns.iter().map(|c| c.name.clone()).collect();
                Ok(Table { source: t.source.clone(), sheet: t.sheet.clone(),
                    columns: vec![Column { name: "Value".into(), col_type: ColumnType::Text, values: names }] })
            }

            // ── ColumnsOfType ─────────────────────────────────────────────
            "Table.ColumnsOfType" => {
                let type_filter = args.get(1).and_then(|a| a.as_bare_type_list()).unwrap_or(&[]);
                let t = Self::lookup(input_name, env, source)?;
                let names: Vec<String> = t.columns.iter()
                    .filter(|c| type_filter.iter().any(|ft| columns_of_type_match(&c.col_type, ft)))
                    .map(|c| c.name.clone())
                    .collect();
                Ok(Table { source: t.source.clone(), sheet: t.sheet.clone(),
                    columns: vec![Column { name: "Value".into(), col_type: ColumnType::Text, values: names }] })
            }

            // ── IsEmpty ───────────────────────────────────────────────────
            "Table.IsEmpty" => {
                let t = Self::lookup(input_name, env, source)?;
                Ok(Table { source: t.source.clone(), sheet: t.sheet.clone(),
                    columns: vec![Column { name: "Value".into(), col_type: ColumnType::Boolean, values: vec![(t.row_count() == 0).to_string()] }] })
            }

            // ── Schema ────────────────────────────────────────────────────
            "Table.Schema" => {
                let t = Self::lookup(input_name, env, source)?;
                let names:     Vec<String> = t.columns.iter().map(|c| c.name.clone()).collect();
                let kinds:     Vec<String> = t.columns.iter().map(|c| c.col_type.to_string()).collect();
                let nullables: Vec<String> = t.columns.iter().map(|_| "true".to_string()).collect();
                Ok(Table { source: t.source.clone(), sheet: t.sheet.clone(),
                    columns: vec![
                        Column { name: "Name".into(),       col_type: ColumnType::Text,    values: names },
                        Column { name: "Kind".into(),       col_type: ColumnType::Text,    values: kinds },
                        Column { name: "IsNullable".into(), col_type: ColumnType::Boolean, values: nullables },
                    ]
                })
            }

            // ── HasColumns ────────────────────────────────────────────────
            "Table.HasColumns" => {
                let columns = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                let t       = Self::lookup(input_name, env, source)?;
                let has_all = columns.iter().all(|c| t.get_column(c).is_some());
                Ok(Table { source: t.source.clone(), sheet: t.sheet.clone(),
                    columns: vec![Column { name: "Value".into(), col_type: ColumnType::Boolean, values: vec![has_all.to_string()] }] })
            }

            // ── IsDistinct ────────────────────────────────────────────────
            "Table.IsDistinct" => {
                let t  = Self::lookup(input_name, env, source)?;
                let rc = t.row_count();
                let mut seen = std::collections::HashSet::new();
                let mut is_distinct = true;
                for i in 0..rc {
                    let key: Vec<String> = t.columns.iter().map(|c| c.values.get(i).cloned().unwrap_or_default()).collect();
                    if !seen.insert(key) { is_distinct = false; break; }
                }
                Ok(Table { source: t.source.clone(), sheet: t.sheet.clone(),
                    columns: vec![Column { name: "Value".into(), col_type: ColumnType::Boolean, values: vec![is_distinct.to_string()] }] })
            }

            // ── Join ──────────────────────────────────────────────────────
            "Table.Join" | "Table.FuzzyJoin" => {
                let left_keys  = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                let right_name = args.get(2).and_then(|a| a.as_step_ref()).unwrap_or("");
                let right_keys = args.get(3).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                let join_kind  = args.get(4).and_then(|a| a.as_join_kind()).cloned().unwrap_or(JoinKind::Inner);
                let lt  = Self::lookup(input_name, env, source)?;
                let rt  = Self::lookup(right_name, env, source)?;
                let lrc = lt.row_count();
                let rrc = rt.row_count();
                let mut out_cols: Vec<Column> = lt.columns.iter().map(|c| Column { name: c.name.clone(), col_type: c.col_type.clone(), values: vec![] }).collect();
                let right_only: Vec<&Column> = rt.columns.iter().filter(|c| !lt.columns.iter().any(|lc| lc.name == c.name)).collect();
                for rc_col in &right_only {
                    out_cols.push(Column { name: rc_col.name.clone(), col_type: rc_col.col_type.clone(), values: vec![] });
                }
                for li in 0..lrc {
                    let lkey: Vec<String> = left_keys.iter().map(|k| Self::cell(lt, k, li).to_string()).collect();
                    let mut matched = false;
                    for ri in 0..rrc {
                        let rkey: Vec<String> = right_keys.iter().map(|k| Self::cell(rt, k, ri).to_string()).collect();
                        if lkey == rkey {
                            matched = true;
                            for (j, lc) in lt.columns.iter().enumerate() {
                                out_cols[j].values.push(lc.values.get(li).cloned().unwrap_or_default());
                            }
                            let lt_col_count = lt.columns.len();
                            for (j, rc_col) in right_only.iter().enumerate() {
                                out_cols[lt_col_count + j].values.push(rc_col.values.get(ri).cloned().unwrap_or_default());
                            }
                        }
                    }
                    if !matched && matches!(join_kind, JoinKind::Left | JoinKind::Full) {
                        for (j, lc) in lt.columns.iter().enumerate() {
                            out_cols[j].values.push(lc.values.get(li).cloned().unwrap_or_default());
                        }
                        let lt_col_count = lt.columns.len();
                        for j in 0..right_only.len() {
                            out_cols[lt_col_count + j].values.push(String::new());
                        }
                    }
                }
                if matches!(join_kind, JoinKind::Right | JoinKind::Full) {
                    for ri in 0..rrc {
                        let rkey: Vec<String> = right_keys.iter().map(|k| Self::cell(rt, k, ri).to_string()).collect();
                        let any_match = (0..lrc).any(|li| {
                            let lkey: Vec<String> = left_keys.iter().map(|k| Self::cell(lt, k, li).to_string()).collect();
                            lkey == rkey
                        });
                        if !any_match {
                            for (j, _) in lt.columns.iter().enumerate() {
                                let col_name = &out_cols[j].name;
                                if let Some(rc_col) = rt.get_column(col_name) {
                                    out_cols[j].values.push(rc_col.values.get(ri).cloned().unwrap_or_default());
                                } else {
                                    out_cols[j].values.push(String::new());
                                }
                            }
                            let lt_col_count = lt.columns.len();
                            for (j, rc_col) in right_only.iter().enumerate() {
                                out_cols[lt_col_count + j].values.push(rc_col.values.get(ri).cloned().unwrap_or_default());
                            }
                        }
                    }
                }
                Ok(Table { source: lt.source.clone(), sheet: lt.sheet.clone(), columns: out_cols })
            }

            // ── NestedJoin / FuzzyNestedJoin ──────────────────────────────
            "Table.NestedJoin" | "Table.FuzzyNestedJoin" => {
                let left_keys  = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                let right_name = args.get(2).and_then(|a| a.as_step_ref()).unwrap_or("");
                let right_keys = args.get(3).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                let new_col    = args.get(4).and_then(|a| a.as_str()).unwrap_or("NewColumn");
                let lt  = Self::lookup(input_name, env, source)?;
                let rt  = Self::lookup(right_name, env, source)?;
                let lrc = lt.row_count();
                let rrc = rt.row_count();
                let mut result = lt.clone();
                let mut nested_values: Vec<String> = Vec::with_capacity(lrc);
                for li in 0..lrc {
                    let lkey: Vec<String> = left_keys.iter().map(|k| Self::cell(lt, k, li).to_string()).collect();
                    let match_count = (0..rrc).filter(|&ri| {
                        let rkey: Vec<String> = right_keys.iter().map(|k| Self::cell(rt, k, ri).to_string()).collect();
                        lkey == rkey
                    }).count();
                    nested_values.push(format!("Table({})", match_count));
                }
                result.columns.push(Column { name: new_col.to_string(), col_type: ColumnType::Text, values: nested_values });
                Ok(result)
            }

            // ── AddRankColumn ─────────────────────────────────────────────
            "Table.AddRankColumn" => {
                let col_name = args.get(1).and_then(|a| a.as_str()).unwrap_or("Rank");
                let by       = args.get(2).and_then(|a| a.as_sort_list()).unwrap_or(&[]);
                let t        = Self::lookup(input_name, env, source)?;
                let rc       = t.row_count();
                let mut indices: Vec<usize> = (0..rc).collect();
                indices.sort_by(|&a, &b| {
                    for (sort_col, order) in by {
                        let av = Self::cell(t, sort_col, a);
                        let bv = Self::cell(t, sort_col, b);
                        let ct = t.get_column(sort_col).map(|c| &c.col_type).unwrap_or(&ColumnType::Text);
                        let cmp = Self::compare_raw(av, bv, ct);
                        let cmp = match order { SortOrder::Ascending => cmp, SortOrder::Descending => cmp.reverse() };
                        if cmp != std::cmp::Ordering::Equal { return cmp; }
                    }
                    std::cmp::Ordering::Equal
                });
                let mut ranks = vec![0usize; rc];
                for (rank, &idx) in indices.iter().enumerate() { ranks[idx] = rank + 1; }
                let mut result = t.clone();
                result.columns.push(Column { name: col_name.to_string(), col_type: ColumnType::Integer, values: ranks.iter().map(|r| r.to_string()).collect() });
                Ok(result)
            }

            // ── Max ───────────────────────────────────────────────────────
            "Table.Max" => {
                let col_name = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                let t        = Self::lookup(input_name, env, source)?;
                let rc       = t.row_count();
                if rc == 0 { return Ok(t.clone()); }
                let ct = t.get_column(col_name).map(|c| &c.col_type).unwrap_or(&ColumnType::Text);
                let best = (0..rc).max_by(|&a, &b| Self::compare_raw(Self::cell(t, col_name, a), Self::cell(t, col_name, b), ct)).unwrap_or(0);
                Ok(Self::select_rows(t, &[best]))
            }

            // ── Min ───────────────────────────────────────────────────────
            "Table.Min" => {
                let col_name = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                let t        = Self::lookup(input_name, env, source)?;
                let rc       = t.row_count();
                if rc == 0 { return Ok(t.clone()); }
                let ct = t.get_column(col_name).map(|c| &c.col_type).unwrap_or(&ColumnType::Text);
                let best = (0..rc).min_by(|&a, &b| Self::compare_raw(Self::cell(t, col_name, a), Self::cell(t, col_name, b), ct)).unwrap_or(0);
                Ok(Self::select_rows(t, &[best]))
            }

            // ── MaxN ──────────────────────────────────────────────────────
            "Table.MaxN" => {
                let n        = args.get(1).and_then(|a| a.as_int()).unwrap_or(1) as usize;
                let col_name = args.get(2).and_then(|a| a.as_str()).unwrap_or("");
                let t        = Self::lookup(input_name, env, source)?;
                let rc       = t.row_count();
                let ct = t.get_column(col_name).map(|c| &c.col_type).unwrap_or(&ColumnType::Text);
                let mut indices: Vec<usize> = (0..rc).collect();
                indices.sort_by(|&a, &b| Self::compare_raw(Self::cell(t, col_name, b), Self::cell(t, col_name, a), ct));
                indices.truncate(n);
                Ok(Self::select_rows(t, &indices))
            }

            // ── MinN ──────────────────────────────────────────────────────
            "Table.MinN" => {
                let n        = args.get(1).and_then(|a| a.as_int()).unwrap_or(1) as usize;
                let col_name = args.get(2).and_then(|a| a.as_str()).unwrap_or("");
                let t        = Self::lookup(input_name, env, source)?;
                let rc       = t.row_count();
                let ct = t.get_column(col_name).map(|c| &c.col_type).unwrap_or(&ColumnType::Text);
                let mut indices: Vec<usize> = (0..rc).collect();
                indices.sort_by(|&a, &b| Self::compare_raw(Self::cell(t, col_name, a), Self::cell(t, col_name, b), ct));
                indices.truncate(n);
                Ok(Self::select_rows(t, &indices))
            }

            // ── ReplaceValue ──────────────────────────────────────────────
            "Table.ReplaceValue" => {
                let old_value = args.get(1).and_then(|a| a.as_expr());
                let new_value = args.get(2).and_then(|a| a.as_expr());
                let t         = Self::lookup(input_name, env, source)?;
                let old_v = old_value.and_then(|e| Self::eval_expr(e, t, 0).ok()).map(|v| v.to_raw_string()).unwrap_or_default();
                let new_v = new_value.and_then(|e| Self::eval_expr(e, t, 0).ok()).map(|v| v.to_raw_string()).unwrap_or_default();
                let mut result = t.clone();
                for col in result.columns.iter_mut() {
                    for val in col.values.iter_mut() {
                        if *val == old_v { *val = new_v.clone(); }
                    }
                }
                Ok(result)
            }

            // ── ReplaceErrorValues ────────────────────────────────────────
            "Table.ReplaceErrorValues" => {
                let replacements = args.get(1).and_then(|a| a.as_transform_list()).unwrap_or(&[]);
                let t = Self::lookup(input_name, env, source)?;
                let mut result = t.clone();
                for (col_name, replacement, _) in replacements {
                    let repl_val = Self::eval_expr(replacement, t, 0).ok().map(|v| v.to_raw_string()).unwrap_or_default();
                    if let Some(col) = result.columns.iter_mut().find(|c| c.name == *col_name) {
                        for val in col.values.iter_mut() {
                            if val.is_empty() || val == "null" { *val = repl_val.clone(); }
                        }
                    }
                }
                Ok(result)
            }

            // ── InsertRows / ReplaceRows ──────────────────────────────────
            "Table.InsertRows" | "Table.ReplaceRows" => {
                Ok(Self::lookup(input_name, env, source)?.clone())
            }

            // ── List.Generate ─────────────────────────────────────────────
            "List.Generate" => {
                let initial   = args.get(0).and_then(|a| a.as_expr());
                let condition = args.get(1).and_then(|a| a.as_expr());
                let next      = args.get(2).and_then(|a| a.as_expr());
                let selector  = args.get(3).and_then(|a| a.as_expr());
                if let (Some(ini), Some(cond), Some(nxt)) = (initial, condition, next) {
                    let seed = Self::eval_lambda_or_expr(ini, env, source)?;
                    let mut items: Vec<Value> = vec![];
                    let mut current = seed;
                    loop {
                        let row_table = Self::single_value_table(source, &current);
                        let c = Self::eval_expr(cond, &row_table, 0)?;
                        if !matches!(c, Value::Bool(true)) { break; }
                        let out = if let Some(sel) = selector {
                            Self::eval_expr(sel, &row_table, 0)?
                        } else { current.clone() };
                        items.push(out);
                        current = Self::eval_expr(nxt, &row_table, 0)?;
                    }
                    let values: Vec<String> = items.iter().map(|v| v.to_raw_string()).collect();
                    let col_type = infer_type(&values);
                    Ok(Table { source: source.source.clone(), sheet: source.sheet.clone(),
                        columns: vec![Column { name: "Value".into(), col_type, values }] })
                } else {
                    Ok(source.clone())
                }
            }

            // ── List.Select ───────────────────────────────────────────────
            "List.Select" => {
                let predicate = args.get(1).and_then(|a| a.as_expr());
                if let Some(pred) = predicate {
                    let items = match args.get(0) {
                        Some(CallArg::Expr(e))       => Self::eval_to_list(e, env, source)?,
                        Some(CallArg::StepRef(step)) => {
                            let table = Self::lookup(step, env, source)?;
                            table.columns.first()
                                .map(|col| col.values.iter().map(|v| Value::Text(v.clone())).collect())
                                .unwrap_or_default()
                        }
                        _ => vec![],
                    };
                    let kept: Vec<Value> = items.into_iter()
                        .filter(|item| Self::apply_predicate_value(pred, item, source))
                        .collect();
                    let raws: Vec<String> = kept.iter().map(|v| v.to_raw_string()).collect();
                    let col_type = infer_type(&raws);
                    Ok(Table { source: source.source.clone(), sheet: source.sheet.clone(),
                        columns: vec![Column { name: "Value".into(), col_type, values: raws }] })
                } else { Ok(source.clone()) }
            }

            // ── List.Transform ────────────────────────────────────────────
            "List.Transform" => {
                let transform = args.get(1).and_then(|a| a.as_expr());
                if let Some(tr) = transform {
                    let items = match args.get(0) {
                        Some(CallArg::Expr(e))       => Self::eval_to_list(e, env, source)?,
                        Some(CallArg::StepRef(step)) => {
                            let table = Self::lookup(step, env, source)?;
                            table.columns.first()
                                .map(|col| col.values.iter().map(|v| Value::Text(v.clone())).collect())
                                .unwrap_or_default()
                        }
                        _ => vec![],
                    };
                    let results: Vec<Value> = items.iter()
                        .map(|item| Self::apply_transform_value(tr, item, source).unwrap_or(Value::Null))
                        .collect();
                    let values: Vec<String> = results.iter().map(|v| v.to_raw_string()).collect();
                    let col_type = infer_type(&values);
                    Ok(Table { source: source.source.clone(), sheet: source.sheet.clone(),
                        columns: vec![Column { name: "Value".into(), col_type, values }] })
                } else { Ok(source.clone()) }
            }

            // ── List.Intersect ────────────────────────────────────────────
            // N-ary multiset intersection of a list-of-lists.
            // Result multiplicity = MIN multiplicity across all inner lists.
            // Order follows the first inner list.
            // NOTE: equationCriteria (arg 2) is currently ignored at runtime.
            "List.Intersect" => {
                use std::collections::HashMap;
                // Evaluate arg 0 to a flat Vec<Value>; each element should itself
                // be a Value::List (the inner list).
                let outer: Vec<Value> = match args.get(0) {
                    Some(CallArg::Expr(e))       => Self::eval_to_list(e, env, source)?,
                    Some(CallArg::StepRef(step)) => {
                        let table = Self::lookup(step, env, source)?;
                        table.columns.first()
                            .map(|col| col.values.iter().map(|v| Value::Text(v.clone())).collect())
                            .unwrap_or_default()
                    }
                    _ => vec![],
                };
                // Extract each inner Value into a Vec<String>.
                let inner_lists: Vec<Vec<String>> = outer.iter().map(|v| {
                    match v {
                        Value::List(items) => items.iter().map(|x| x.to_raw_string()).collect(),
                        other => vec![other.to_raw_string()],
                    }
                }).collect();

                if inner_lists.is_empty() {
                    return Ok(Table {
                        source:  source.source.clone(),
                        sheet:   source.sheet.clone(),
                        columns: vec![Column { name: "Value".into(), col_type: infer_type(&[]), values: vec![] }],
                    });
                }

                // Build multiplicity maps and compute MIN across all lists.
                let count_map = |list: &[String]| -> HashMap<String, usize> {
                    let mut m: HashMap<String, usize> = HashMap::new();
                    for v in list { *m.entry(v.clone()).or_insert(0) += 1; }
                    m
                };
                let mut mins = count_map(&inner_lists[0]);
                for l in &inner_lists[1..] {
                    let c = count_map(l);
                    mins.retain(|k, v| {
                        let other = *c.get(k).unwrap_or(&0);
                        *v = (*v).min(other);
                        *v > 0
                    });
                }
                // Emit elements in first-list order, consuming the MIN budget.
                let mut values: Vec<String> = Vec::new();
                for v in &inner_lists[0] {
                    if let Some(n) = mins.get_mut(v) {
                        if *n > 0 { values.push(v.clone()); *n -= 1; }
                    }
                }
                let col_type = infer_type(&values);
                Ok(Table {
                    source:  source.source.clone(),
                    sheet:   source.sheet.clone(),
                    columns: vec![Column { name: "Value".into(), col_type, values }],
                })
            }

            // ── List.Difference ───────────────────────────────────────────
            // Multiset (bag) difference: each occurrence in list2 removes
            // ONE matching occurrence from list1. Order of list1 is preserved.
            // NOTE: equationCriteria (arg 3) is currently ignored at runtime —
            // only default equality is supported.
            "List.Difference" => {
                let list1: Vec<Value> = match args.get(0) {
                    Some(CallArg::Expr(e))       => Self::eval_to_list(e, env, source)?,
                    Some(CallArg::StepRef(step)) => {
                        let table = Self::lookup(step, env, source)?;
                        table.columns.first()
                            .map(|col| col.values.iter().map(|v| Value::Text(v.clone())).collect())
                            .unwrap_or_default()
                    }
                    _ => vec![],
                };
                let list2: Vec<Value> = match args.get(1) {
                    Some(CallArg::Expr(e))       => Self::eval_to_list(e, env, source)?,
                    Some(CallArg::StepRef(step)) => {
                        let table = Self::lookup(step, env, source)?;
                        table.columns.first()
                            .map(|col| col.values.iter().map(|v| Value::Text(v.clone())).collect())
                            .unwrap_or_default()
                    }
                    _ => vec![],
                };
                // Build a multiset count of list2 values.
                use std::collections::HashMap;
                let mut counts: HashMap<String, usize> = HashMap::new();
                for v in &list2 {
                    *counts.entry(v.to_raw_string()).or_insert(0) += 1;
                }
                // Walk list1 in order; keep an item only if its bucket is exhausted.
                let mut values: Vec<String> = Vec::with_capacity(list1.len());
                for v in &list1 {
                    let key = v.to_raw_string();
                    match counts.get_mut(&key) {
                        Some(n) if *n > 0 => { *n -= 1; } // consume one occurrence, drop item
                        _ => values.push(key),
                    }
                }
                let col_type = infer_type(&values);
                Ok(Table {
                    source: source.source.clone(),
                    sheet:  source.sheet.clone(),
                    columns: vec![Column { name: "Value".into(), col_type, values }],
                })
            }

            // ── List.RemoveItems ──────────────────────────────────────────
            "List.RemoveItems" => {
                let list1: Vec<Value> = match args.get(0) {
                    Some(CallArg::Expr(e))       => Self::eval_to_list(e, env, source)?,
                    Some(CallArg::StepRef(step)) => {
                        let table = Self::lookup(step, env, source)?;
                        table.columns.first()
                            .map(|col| col.values.iter().map(|v| Value::Text(v.clone())).collect())
                            .unwrap_or_default()
                    }
                    _ => vec![],
                };
                let list2: Vec<Value> = match args.get(1) {
                    Some(CallArg::Expr(e))       => Self::eval_to_list(e, env, source)?,
                    Some(CallArg::StepRef(step)) => {
                        let table = Self::lookup(step, env, source)?;
                        table.columns.first()
                            .map(|col| col.values.iter().map(|v| Value::Text(v.clone())).collect())
                            .unwrap_or_default()
                    }
                    _ => vec![],
                };
                let removal: std::collections::HashSet<String> =
                    list2.iter().map(|v| v.to_raw_string()).collect();
                let values: Vec<String> = list1.iter()
                    .map(|v| v.to_raw_string())
                    .filter(|s| !removal.contains(s))
                    .collect();
                let col_type = infer_type(&values);
                Ok(Table {
                    source: source.source.clone(),
                    sheet:  source.sheet.clone(),
                    columns: vec![Column { name: "Value".into(), col_type, values }],
                })
            }

            // ── Fallback ──────────────────────────────────────────────────
            _ => {
                if !input_name.is_empty() {
                    Ok(Self::lookup(input_name, env, source)?.clone())
                } else {
                    Ok(source.clone())
                }
            }
        }
    }

    // -- expression evaluator -------------------------------------------------

    fn eval_expr(node: &ExprNode, table: &Table, row: usize) -> ExecResult<Value> {
        match &node.expr {
            // -- literals --------------------------------------------------------
            Expr::IntLit(n)    => Ok(Value::Int(*n)),
            Expr::FloatLit(n)  => Ok(Value::Float(*n)),
            Expr::BoolLit(b)   => Ok(Value::Bool(*b)),
            Expr::StringLit(s) => Ok(Value::Text(s.clone())),
            Expr::NullLit      => Ok(Value::Null),

            // -- column references -----------------------------------------------
            // Identifier covers bare names (`Age`, `_`, `Order.Ascending`).
            // `_` is the implicit each param -- when an explicit list-context
            // value is bound (push_underscore), use it; otherwise look up the
            // current row by the identifier name (legacy table-row behaviour).
            Expr::Identifier(name) => {
                if name == "_" {
                    if let Some(v) = current_underscore() {
                        return Ok(v);
                    }
                }
                let raw      = Self::cell(table, name, row);
                let col_type = table.get_column(name)
                    .map(|c| &c.col_type)
                    .unwrap_or(&ColumnType::Text);
                Ok(Self::coerce(raw, col_type))
            }
            Expr::ColumnAccess(name) => {
                if name == "_" {
                    if let Some(v) = current_underscore() {
                        return Ok(v);
                    }
                }
                let raw      = Self::cell(table, name, row);
                let col_type = table.get_column(name)
                    .map(|c| &c.col_type)
                    .unwrap_or(&ColumnType::Text);
                Ok(Self::coerce(raw, col_type))
            }

            // Field access on a record-like operand: `record[FieldName]`.
            // 1. If the record evaluates to a `Value::Record`, look up `field`.
            // 2. Otherwise fall back to the legacy behaviour of treating
            //    `field` as a column name on the current table row — preserves
            //    `(row) => row[Age]` semantics where `row` is bound to the
            //    implicit table row (not yet a real Record value).
            Expr::FieldAccess { record, field } => {
                let rec_val = Self::eval_expr(record, table, row)?;
                match rec_val {
                    Value::Record(map) => Ok(map.get(field).cloned().unwrap_or(Value::Null)),
                    _ => {
                        let raw      = Self::cell(table, field, row);
                        let col_type = table.get_column(field)
                            .map(|c| &c.col_type)
                            .unwrap_or(&ColumnType::Text);
                        Ok(Self::coerce(raw, col_type))
                    }
                }
            }

            // ── lambda — evaluate body (covers both `each` and explicit lambdas)
            Expr::Lambda { body, .. } => Self::eval_expr(body, table, row),

            // ── binary op ────────────────────────────────────────────────
            Expr::BinaryOp { left, op, right } => {
                match op {
                    // Short-circuit logical operators
                    Operator::And => {
                        let lv = Self::eval_expr(left, table, row)?;
                        if lv == Value::Bool(false) { return Ok(Value::Bool(false)); }
                        Self::eval_expr(right, table, row)
                    }
                    Operator::Or => {
                        let lv = Self::eval_expr(left, table, row)?;
                        if lv == Value::Bool(true) { return Ok(Value::Bool(true)); }
                        Self::eval_expr(right, table, row)
                    }
                    _ => {
                        let lv = Self::eval_expr(left,  table, row)?;
                        let rv = Self::eval_expr(right, table, row)?;
                        Self::apply_op(lv, op, rv)
                    }
                }
            }

            // ── unary op ──────────────────────────────────────────────────
            Expr::UnaryOp { op, operand } => {
                let v = Self::eval_expr(operand, table, row)?;
                match op {
                    UnaryOp::Not => match v {
                        Value::Bool(b) => Ok(Value::Bool(!b)),
                        _ => Err(ExecError::TypeMismatch(
                            format!("'not' requires Bool, got {:?}", v)
                        )),
                    },
                    UnaryOp::Neg => match v {
                        Value::Int(n)   => Ok(Value::Int(-n)),
                        Value::Float(f) => Ok(Value::Float(-f)),
                        _ => Err(ExecError::TypeMismatch(
                            format!("unary '-' requires numeric, got {:?}", v)
                        )),
                    },
                }
            }

            // ── function calls — best-effort evaluation ───────────────────
            Expr::FunctionCall { name, args } => {
                let evaled: Vec<Value> = args.iter()
                    .map(|a| Self::eval_expr(a, table, row))
                    .collect::<Result<_, _>>()?;
                Self::call_function(name, evaled)
            }

            // ── collections ──────────────────────────────────────────────
            // Build first-class List / Record values so downstream
            // operations (FieldAccess, list ops) can operate on them.
            Expr::List(items) => {
                let evaled: Vec<Value> = items.iter()
                    .map(|item| Self::eval_expr(item, table, row))
                    .collect::<Result<_, _>>()?;
                Ok(Value::List(evaled))
            }
            Expr::Record(fields) => {
                use std::collections::BTreeMap;
                let mut map = BTreeMap::new();
                for (k, v) in fields {
                    map.insert(k.clone(), Self::eval_expr(v, table, row)?);
                }
                Ok(Value::Record(map))
            }
        }
    }

    /// Dispatch built-in M function calls that appear inside expressions.
    fn call_function(name: &str, args: Vec<Value>) -> ExecResult<Value> {
        match name {
            "Text.Upper" => match args.into_iter().next() {
                Some(Value::Text(s)) => Ok(Value::Text(s.to_uppercase())),
                _ => Ok(Value::Null),
            },
            "Text.Lower" => match args.into_iter().next() {
                Some(Value::Text(s)) => Ok(Value::Text(s.to_lowercase())),
                _ => Ok(Value::Null),
            },
            "Text.Trim" => match args.into_iter().next() {
                Some(Value::Text(s)) => Ok(Value::Text(s.trim().to_string())),
                _ => Ok(Value::Null),
            },
            "Text.Length" => match args.into_iter().next() {
                // Accept any value: coerce to its text representation first,
                // matching M's behaviour where Text.Length(42) = 2.
                Some(v) => Ok(Value::Int(v.to_raw_string().chars().count() as i64)),
                None    => Ok(Value::Null),
            },
            "Number.From" => match args.into_iter().next() {
                Some(Value::Int(n))   => Ok(Value::Float(n as f64)),
                Some(Value::Float(f)) => Ok(Value::Float(f)),
                Some(Value::Text(s))  => Ok(s.parse::<f64>().map(Value::Float).unwrap_or(Value::Null)),
                _ => Ok(Value::Null),
            },
            "Logical.From" => match args.into_iter().next() {
                Some(Value::Bool(b))  => Ok(Value::Bool(b)),
                Some(Value::Text(s))  => Ok(Value::Bool(s == "true")),
                _ => Ok(Value::Null),
            },

            // ── Text functions ────────────────────────────────────────────
            "Text.From" => match args.into_iter().next() {
                Some(v) => Ok(Value::Text(v.to_raw_string())),
                None    => Ok(Value::Null),
            },
            "Text.TrimStart" => match args.into_iter().next() {
                Some(Value::Text(s)) => Ok(Value::Text(s.trim_start().to_string())),
                _ => Ok(Value::Null),
            },
            "Text.TrimEnd" => match args.into_iter().next() {
                Some(Value::Text(s)) => Ok(Value::Text(s.trim_end().to_string())),
                _ => Ok(Value::Null),
            },
            "Text.PadStart" => {
                let mut it = args.into_iter();
                let text  = it.next().map(|v| v.to_raw_string()).unwrap_or_default();
                let width = match it.next() { Some(Value::Int(n)) => n as usize, Some(Value::Float(f)) => f as usize, _ => text.len() };
                let pad   = it.next().map(|v| v.to_raw_string()).unwrap_or_else(|| " ".to_string());
                let pad_c = pad.chars().next().unwrap_or(' ');
                let cur   = text.chars().count();
                if cur >= width { Ok(Value::Text(text)) }
                else {
                    let padding: String = std::iter::repeat(pad_c).take(width - cur).collect();
                    Ok(Value::Text(format!("{}{}", padding, text)))
                }
            },
            "Text.PadEnd" => {
                let mut it = args.into_iter();
                let text  = it.next().map(|v| v.to_raw_string()).unwrap_or_default();
                let width = match it.next() { Some(Value::Int(n)) => n as usize, Some(Value::Float(f)) => f as usize, _ => text.len() };
                let pad   = it.next().map(|v| v.to_raw_string()).unwrap_or_else(|| " ".to_string());
                let pad_c = pad.chars().next().unwrap_or(' ');
                let cur   = text.chars().count();
                if cur >= width { Ok(Value::Text(text)) }
                else {
                    let padding: String = std::iter::repeat(pad_c).take(width - cur).collect();
                    Ok(Value::Text(format!("{}{}", text, padding)))
                }
            },
            "Text.Contains" => {
                let mut it = args.into_iter();
                let text = it.next().map(|v| v.to_raw_string()).unwrap_or_default();
                let sub  = it.next().map(|v| v.to_raw_string()).unwrap_or_default();
                Ok(Value::Bool(text.contains(&sub)))
            },
            "Text.StartsWith" => {
                let mut it = args.into_iter();
                let text   = it.next().map(|v| v.to_raw_string()).unwrap_or_default();
                let prefix = it.next().map(|v| v.to_raw_string()).unwrap_or_default();
                Ok(Value::Bool(text.starts_with(&prefix)))
            },
            "Text.EndsWith" => {
                let mut it = args.into_iter();
                let text   = it.next().map(|v| v.to_raw_string()).unwrap_or_default();
                let suffix = it.next().map(|v| v.to_raw_string()).unwrap_or_default();
                Ok(Value::Bool(text.ends_with(&suffix)))
            },
            "Text.Start" => {
                let mut it = args.into_iter();
                let text  = it.next().map(|v| v.to_raw_string()).unwrap_or_default();
                let count = match it.next() { Some(Value::Int(n)) => n as usize, Some(Value::Float(f)) => f as usize, _ => 0 };
                let chars: Vec<char> = text.chars().collect();
                let end = count.min(chars.len());
                Ok(Value::Text(chars[..end].iter().collect()))
            },
            "Text.End" => {
                let mut it = args.into_iter();
                let text  = it.next().map(|v| v.to_raw_string()).unwrap_or_default();
                let count = match it.next() { Some(Value::Int(n)) => n as usize, Some(Value::Float(f)) => f as usize, _ => 0 };
                let chars: Vec<char> = text.chars().collect();
                let start = chars.len().saturating_sub(count);
                Ok(Value::Text(chars[start..].iter().collect()))
            },
            "Text.Range" => {
                let mut it = args.into_iter();
                let text   = it.next().map(|v| v.to_raw_string()).unwrap_or_default();
                let offset = match it.next() { Some(Value::Int(n)) => n as usize, Some(Value::Float(f)) => f as usize, _ => 0 };
                let count  = match it.next() { Some(Value::Int(n)) => n as usize, Some(Value::Float(f)) => f as usize, _ => text.len() };
                let chars: Vec<char> = text.chars().collect();
                let end = (offset + count).min(chars.len());
                let start = offset.min(chars.len());
                Ok(Value::Text(chars[start..end].iter().collect()))
            },
            "Text.Replace" => {
                let mut it = args.into_iter();
                let text = it.next().map(|v| v.to_raw_string()).unwrap_or_default();
                let old  = it.next().map(|v| v.to_raw_string()).unwrap_or_default();
                let new  = it.next().map(|v| v.to_raw_string()).unwrap_or_default();
                Ok(Value::Text(text.replace(&old, &new)))
            },
            "Text.Split" => {
                let mut it = args.into_iter();
                let text  = it.next().map(|v| v.to_raw_string()).unwrap_or_default();
                let delim = it.next().map(|v| v.to_raw_string()).unwrap_or_default();
                // Return as a single text with items separated — lists not yet first-class in Value
                let parts: Vec<&str> = text.split(&delim).collect();
                Ok(Value::Text(parts.join(",")))
            },
            "Text.Combine" => {
                let mut it = args.into_iter();
                // First arg should be a list; since we can't fully represent lists,
                // handle the common case of a stringified comma-separated list.
                let list_val = it.next().unwrap_or(Value::Null);
                let sep = it.next().map(|v| v.to_raw_string()).unwrap_or_default();
                let text = list_val.to_raw_string();
                // If the value looks comma-separated, rejoin with separator
                let parts: Vec<&str> = text.split(',').collect();
                if parts.len() > 1 {
                    Ok(Value::Text(parts.join(&sep)))
                } else {
                    Ok(Value::Text(text))
                }
            },

            // ── Number functions ──────────────────────────────────────────
            "Number.Round" => {
                let mut it = args.into_iter();
                let n = match it.next() {
                    Some(Value::Int(i))   => i as f64,
                    Some(Value::Float(f)) => f,
                    _ => return Ok(Value::Null),
                };
                let digits = match it.next() {
                    Some(Value::Int(d))   => d,
                    Some(Value::Float(f)) => f as i64,
                    _ => 0,
                };
                let factor = 10f64.powi(digits as i32);
                Ok(Value::Float((n * factor).round() / factor))
            },
            "Number.RoundUp" => match args.into_iter().next() {
                Some(Value::Int(n))   => Ok(Value::Int(n)),
                Some(Value::Float(f)) => Ok(Value::Float(f.ceil())),
                _ => Ok(Value::Null),
            },
            "Number.RoundDown" => match args.into_iter().next() {
                Some(Value::Int(n))   => Ok(Value::Int(n)),
                Some(Value::Float(f)) => Ok(Value::Float(f.floor())),
                _ => Ok(Value::Null),
            },
            "Number.Abs" => match args.into_iter().next() {
                Some(Value::Int(n))   => Ok(Value::Int(n.abs())),
                Some(Value::Float(f)) => Ok(Value::Float(f.abs())),
                _ => Ok(Value::Null),
            },
            "Number.Sqrt" => match args.into_iter().next() {
                Some(Value::Int(n))   => Ok(Value::Float((n as f64).sqrt())),
                Some(Value::Float(f)) => Ok(Value::Float(f.sqrt())),
                _ => Ok(Value::Null),
            },
            "Number.Power" => {
                let mut it = args.into_iter();
                let base = match it.next() {
                    Some(Value::Int(n))   => n as f64,
                    Some(Value::Float(f)) => f,
                    _ => return Ok(Value::Null),
                };
                let exp = match it.next() {
                    Some(Value::Int(n))   => n as f64,
                    Some(Value::Float(f)) => f,
                    _ => return Ok(Value::Null),
                };
                Ok(Value::Float(base.powf(exp)))
            },
            "Number.Log" => match args.into_iter().next() {
                Some(Value::Int(n))   => Ok(Value::Float((n as f64).ln())),
                Some(Value::Float(f)) => Ok(Value::Float(f.ln())),
                _ => Ok(Value::Null),
            },
            "Number.Mod" => {
                let mut it = args.into_iter();
                let n = it.next();
                let d = it.next();
                match (n, d) {
                    (Some(Value::Int(a)),   Some(Value::Int(b)))   if b != 0 => Ok(Value::Int(a % b)),
                    (Some(Value::Float(a)), Some(Value::Float(b))) if b != 0.0 => Ok(Value::Float(a % b)),
                    (Some(Value::Int(a)),   Some(Value::Float(b))) if b != 0.0 => Ok(Value::Float((a as f64) % b)),
                    (Some(Value::Float(a)), Some(Value::Int(b)))   if b != 0 => Ok(Value::Float(a % (b as f64))),
                    _ => Ok(Value::Null),
                }
            },
            "Number.Sign" => match args.into_iter().next() {
                Some(Value::Int(n))   => Ok(Value::Int(if n > 0 { 1 } else if n < 0 { -1 } else { 0 })),
                Some(Value::Float(f)) => Ok(Value::Int(if f > 0.0 { 1 } else if f < 0.0 { -1 } else { 0 })),
                _ => Ok(Value::Null),
            },

            // ── Logical functions ─────────────────────────────────────────
            "Logical.Not" => match args.into_iter().next() {
                Some(Value::Bool(b)) => Ok(Value::Bool(!b)),
                _ => Ok(Value::Null),
            },
            "Logical.And" => {
                let mut it = args.into_iter();
                match (it.next(), it.next()) {
                    (Some(Value::Bool(a)), Some(Value::Bool(b))) => Ok(Value::Bool(a && b)),
                    _ => Ok(Value::Null),
                }
            },
            "Logical.Or" => {
                let mut it = args.into_iter();
                match (it.next(), it.next()) {
                    (Some(Value::Bool(a)), Some(Value::Bool(b))) => Ok(Value::Bool(a || b)),
                    _ => Ok(Value::Null),
                }
            },
            "Logical.Xor" => {
                let mut it = args.into_iter();
                match (it.next(), it.next()) {
                    (Some(Value::Bool(a)), Some(Value::Bool(b))) => Ok(Value::Bool(a ^ b)),
                    _ => Ok(Value::Null),
                }
            },

            // ── List aggregate functions ──────────────────────────────────
            // These are commonly used inside Table.Group aggregations.
            "List.Sum" => {
                let mut int_sum: i64 = 0;
                let mut float_sum: f64 = 0.0;
                let mut has_float = false;
                for v in &args {
                    match v {
                        Value::Int(n)   => int_sum += n,
                        Value::Float(f) => { float_sum += f; has_float = true; }
                        _ => {}
                    }
                }
                if has_float {
                    Ok(Value::Float(float_sum + int_sum as f64))
                } else {
                    Ok(Value::Int(int_sum))
                }
            },
            "List.Product" => {
                let mut int_prod: i64 = 1;
                let mut float_prod: f64 = 1.0;
                let mut has_float = false;
                for v in &args {
                    match v {
                        Value::Int(n)   => int_prod *= n,
                        Value::Float(f) => { float_prod *= f; has_float = true; }
                        _ => {}
                    }
                }
                if has_float {
                    Ok(Value::Float(float_prod * int_prod as f64))
                } else {
                    Ok(Value::Int(int_prod))
                }
            },
            "List.Count" => Ok(Value::Int(args.len() as i64)),
            "List.NonNullCount" => {
                let count = args.iter().filter(|v| !matches!(v, Value::Null)).count();
                Ok(Value::Int(count as i64))
            },
            "List.IsEmpty" => Ok(Value::Bool(args.is_empty())),
            "List.Average" => {
                let nums: Vec<f64> = args.iter().filter_map(|v| match v {
                    Value::Int(n)   => Some(*n as f64),
                    Value::Float(f) => Some(*f),
                    _ => None,
                }).collect();
                if nums.is_empty() { Ok(Value::Null) }
                else { Ok(Value::Float(nums.iter().sum::<f64>() / nums.len() as f64)) }
            },
            "List.Min" => {
                let mut min: Option<Value> = None;
                for v in args {
                    match (&min, &v) {
                        (None, _) => min = Some(v),
                        (Some(cur), _) => {
                            if matches!(v.cmp_to(cur), Some(std::cmp::Ordering::Less)) {
                                min = Some(v);
                            }
                        }
                    }
                }
                Ok(min.unwrap_or(Value::Null))
            },
            "List.Max" => {
                let mut max: Option<Value> = None;
                for v in args {
                    match (&max, &v) {
                        (None, _) => max = Some(v),
                        (Some(cur), _) => {
                            if matches!(v.cmp_to(cur), Some(std::cmp::Ordering::Greater)) {
                                max = Some(v);
                            }
                        }
                    }
                }
                Ok(max.unwrap_or(Value::Null))
            },
            "List.Median" => {
                let mut nums: Vec<f64> = args.iter().filter_map(|v| match v {
                    Value::Int(n)   => Some(*n as f64),
                    Value::Float(f) => Some(*f),
                    _ => None,
                }).collect();
                if nums.is_empty() { return Ok(Value::Null); }
                nums.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let mid = nums.len() / 2;
                if nums.len() % 2 == 0 {
                    Ok(Value::Float((nums[mid - 1] + nums[mid]) / 2.0))
                } else {
                    Ok(Value::Float(nums[mid]))
                }
            },
            "List.StandardDeviation" => {
                let nums: Vec<f64> = args.iter().filter_map(|v| match v {
                    Value::Int(n)   => Some(*n as f64),
                    Value::Float(f) => Some(*f),
                    _ => None,
                }).collect();
                if nums.len() < 2 { return Ok(Value::Null); }
                let mean = nums.iter().sum::<f64>() / nums.len() as f64;
                let var  = nums.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (nums.len() - 1) as f64;
                Ok(Value::Float(var.sqrt()))
            },
            "List.Covariance" => {
                let mut it = args.into_iter();
                let _list1 = it.next(); // simplified: would need list unpacking
                let _list2 = it.next();
                // Covariance requires paired lists — return Null for now
                Ok(Value::Null)
            },
            "List.Mode" => {
                let mut freq: HashMap<String, (usize, Value)> = HashMap::new();
                for v in args {
                    let key = v.to_raw_string();
                    let entry = freq.entry(key).or_insert((0, v));
                    entry.0 += 1;
                }
                let mode = freq.into_values().max_by_key(|(count, _)| *count);
                Ok(mode.map(|(_, v)| v).unwrap_or(Value::Null))
            },
            "List.Modes" => {
                // Return the single most frequent value (simplified)
                Self::call_function("List.Mode", args)
            },
            "List.AllTrue" => {
                Ok(Value::Bool(args.iter().all(|v| matches!(v, Value::Bool(true)))))
            },
            "List.AnyTrue" => {
                Ok(Value::Bool(args.iter().any(|v| matches!(v, Value::Bool(true)))))
            },
            "List.Contains" => {
                let mut it = args.into_iter();
                let _list = it.next(); // first arg is the list
                let value = it.next().unwrap_or(Value::Null);
                // Simplified: check if value equals the list (not ideal without list type)
                Ok(Value::Bool(_list == Some(value.clone()) || _list.as_ref().map(|v| v.to_raw_string().contains(&value.to_raw_string())).unwrap_or(false)))
            },
            "List.First" => Ok(args.into_iter().next().unwrap_or(Value::Null)),
            "List.Last" => Ok(args.into_iter().last().unwrap_or(Value::Null)),
            "List.Reverse" => {
                // Without a proper list Value type, reverse is a no-op
                Ok(args.into_iter().next().unwrap_or(Value::Null))
            },
            "List.Sort" => {
                // Without a proper list Value type, sort is a no-op
                Ok(args.into_iter().next().unwrap_or(Value::Null))
            },
            "List.Distinct" => {
                // Without a proper list Value type, distinct is a no-op
                Ok(args.into_iter().next().unwrap_or(Value::Null))
            },
            "List.Positions" => {
                // Without a proper list Value type, return Null
                Ok(Value::Null)
            },
            "List.Repeat" => {
                // Without a proper list Value type, return Null
                Ok(args.into_iter().next().unwrap_or(Value::Null))
            },
            "List.Combine" => {
                // Without a proper list Value type, return Null
                Ok(args.into_iter().next().unwrap_or(Value::Null))
            },
            "List.Numbers" => {
                // Typically used at step level, return Null for expression context
                Ok(Value::Null)
            },
            "List.Random" => {
                // Return Null; proper implementation needs list Value type
                Ok(Value::Null)
            },

            // Unknown function — return Null gracefully rather than crashing.
            _ => Ok(Value::Null),
        }
    }

    /// Coerce a raw string cell into a typed Value.
    fn coerce(raw: &str, col_type: &ColumnType) -> Value {
        match col_type {
            ColumnType::Integer => raw.parse::<i64>()
                .map(Value::Int)
                .unwrap_or(Value::Null),
            ColumnType::Float   => raw.parse::<f64>()
                .map(Value::Float)
                .unwrap_or(Value::Null),
            ColumnType::Boolean => match raw {
                "true"  => Value::Bool(true),
                "false" => Value::Bool(false),
                _       => Value::Null,
            },
            _ => Value::Text(raw.to_string()),
        }
    }

    fn apply_op(left: Value, op: &Operator, right: Value) -> ExecResult<Value> {
        match op {
            // ── comparison — always return Bool ───────────────────────────
            Operator::Eq    => Ok(Value::Bool(left == right)),
            Operator::NotEq => Ok(Value::Bool(left != right)),
            Operator::Gt    => Ok(Value::Bool(
                matches!(left.cmp_to(&right), Some(std::cmp::Ordering::Greater))
            )),
            Operator::Lt    => Ok(Value::Bool(
                matches!(left.cmp_to(&right), Some(std::cmp::Ordering::Less))
            )),
            Operator::GtEq  => Ok(Value::Bool(
                matches!(left.cmp_to(&right),
                    Some(std::cmp::Ordering::Greater) | Some(std::cmp::Ordering::Equal))
            )),
            Operator::LtEq  => Ok(Value::Bool(
                matches!(left.cmp_to(&right),
                    Some(std::cmp::Ordering::Less) | Some(std::cmp::Ordering::Equal))
            )),

            // ── arithmetic ────────────────────────────────────────────────
            Operator::Add => Self::numeric_op(left, right, |a, b| a + b, |a, b| a + b),
            Operator::Sub => Self::numeric_op(left, right, |a, b| a - b, |a, b| a - b),
            Operator::Mul => Self::numeric_op(left, right, |a, b| a * b, |a, b| a * b),
            Operator::Div => {
                if right.is_zero() { return Err(ExecError::DivisionByZero); }
                Self::numeric_op(left, right, |a, b| a / b, |a, b| a / b)
            }
            // And/Or are handled with short-circuit logic before apply_op is called.
            Operator::And | Operator::Or => Ok(Value::Null),

            // ── concatenation ─────────────────────────────────────────────
            Operator::Concat => {
                let ls = left.to_raw_string();
                let rs = right.to_raw_string();
                Ok(Value::Text(format!("{}{}", ls, rs)))
            }
        }
    }

    fn numeric_op(
        left:     Value,
        right:    Value,
        int_fn:   impl Fn(i64, i64) -> i64,
        float_fn: impl Fn(f64, f64) -> f64,
    ) -> ExecResult<Value> {
        match (&left, &right) {
            (Value::Int(a),   Value::Int(b))   => Ok(Value::Int(int_fn(*a, *b))),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(float_fn(*a, *b))),
            (Value::Int(a),   Value::Float(b)) => Ok(Value::Float(float_fn(*a as f64, *b))),
            (Value::Float(a), Value::Int(b))   => Ok(Value::Float(float_fn(*a, *b as f64))),
            _ => Err(ExecError::TypeMismatch(
                format!("arithmetic requires numeric operands, got {:?} and {:?}", left, right)
            )),
        }
    }

    // ── row / cell helpers ────────────────────────────────────────────────

    /// Get the raw string value of a cell.
    fn cell<'a>(table: &'a Table, col_name: &str, row: usize) -> &'a str {
        table.get_column(col_name)
            .and_then(|c| c.values.get(row))
            .map(String::as_str)
            .unwrap_or("")
    }

    /// Build a new Table containing only the rows at the given indices,
    /// in that order.  Works for both filtering and sorting.
    fn select_rows(table: &Table, indices: &[usize]) -> Table {
        Table {
            source:  table.source.clone(),
            sheet:   table.sheet.clone(),
            columns: table.columns.iter().map(|col| Column {
                name:     col.name.clone(),
                col_type: col.col_type.clone(),
                values:   indices.iter()
                    .filter_map(|&i| col.values.get(i).cloned())
                    .collect(),
            }).collect(),
        }
    }

    /// Compare two raw string cells for sorting based on the column's type.
    fn compare_raw(a: &str, b: &str, col_type: &ColumnType) -> std::cmp::Ordering {
        match col_type {
            ColumnType::Integer | ColumnType::Currency => {
                a.parse::<i64>().unwrap_or(0)
                    .cmp(&b.parse::<i64>().unwrap_or(0))
            }
            ColumnType::Float => {
                let af = a.parse::<f64>().unwrap_or(0.0);
                let bf = b.parse::<f64>().unwrap_or(0.0);
                af.partial_cmp(&bf).unwrap_or(std::cmp::Ordering::Equal)
            }
            _ => a.cmp(b),
        }
    }

    /// Evaluate an expression that is expected to produce a list of scalar values.
    ///
    /// - `Expr::List` → evaluate every element in the current source context
    ///   (records and nested lists are preserved as `Value::Record`/`Value::List`).
    /// - `Expr::Identifier(name)` → look up a previous step and collect every
    ///   value from its first column.
    /// - Anything else → evaluate the expression; if it produces a `Value::List`,
    ///   unwrap it; otherwise wrap the scalar result in a single-element Vec.
    fn eval_to_list(
        expr:   &ExprNode,
        env:    &HashMap<String, Table>,
        source: &Table,
    ) -> ExecResult<Vec<Value>> {
        match &expr.expr {
            Expr::List(items) => items.iter()
                .map(|item| Self::eval_expr(item, source, 0))
                .collect(),

            Expr::Identifier(name) => {
                let table = Self::lookup(name, env, source)?;
                Ok(table.columns.first()
                    .map(|col| col.values.iter()
                        .map(|v| Value::Text(v.clone()))
                        .collect())
                    .unwrap_or_default())
            }

            // Lambda wrapping (e.g. if the list_expr somehow got wrapped):
            // evaluate the inner body as the list source.
            Expr::Lambda { body, .. } => Self::eval_to_list(body, env, source),

            // Fallback: evaluate as scalar; unwrap Value::List when present.
            _ => match Self::eval_expr(expr, source, 0)? {
                Value::List(items) => Ok(items),
                other              => Ok(vec![other]),
            },
        }
    }

    /// Look up a step's result table by name.
    /// Falls back to the original source if the step is "Source" and
    /// hasn't been inserted yet (shouldn't happen in a well-formed program).
    fn lookup<'a>(
        name:   &str,
        env:    &'a HashMap<String, Table>,
        source: &'a Table,
    ) -> ExecResult<&'a Table> {
        env.get(name)
            .or_else(|| (name == "Source").then_some(source))
            .ok_or_else(|| ExecError::UnknownStep(name.to_string()))
    }

    /// Evaluate a zero-param lambda `() => expr` by calling its body with
    /// an empty row, or evaluate any other expression in the source context.
    fn eval_lambda_or_expr(
        node:   &ExprNode,
        _env:   &HashMap<String, Table>,
        source: &Table,
    ) -> ExecResult<Value> {
        match &node.expr {
            // `() => body` — zero-param lambda: evaluate body ignoring `_`
            Expr::Lambda { params, body } if params.is_empty() => {
                Self::eval_expr(body, source, 0)
            }
            // Any other expression (incl. `each` / `(x) =>`)
            _ => Self::eval_expr(node, source, 0),
        }
    }

    /// Build a one-row, one-column table where the only column is "_" holding
    /// the given value.  Used to provide the implicit `_` binding inside the
    /// `condition` and `next` lambdas of `List.Generate`.
    fn single_value_table(source: &Table, value: &Value) -> Table {
        use pq_types::ColumnType;
        Table {
            source:  source.source.clone(),
            sheet:   source.sheet.clone(),
            columns: vec![Column {
                name:     "_".into(),
                col_type: ColumnType::Text,
                values:   vec![value.to_raw_string()],
            }],
        }
    }

    // ── Shared core utilities ─────────────────────────────────────────────

    /// Apply a callable expression (lambda or function reference) to a single
    /// item value, returning the result as a raw string.
    ///
    /// `_` is bound to `item_raw` via a synthetic single-row table.
    /// This is the canonical per-item evaluation path used by all transform
    /// and filter operations.
    fn apply_transform(
        expr:    &ExprNode,
        item_raw: &str,
        source:  &Table,
    ) -> ExecResult<String> {
        let synthetic = Table {
            source:  source.source.clone(),
            sheet:   source.sheet.clone(),
            columns: vec![Column {
                name:     "_".into(),
                col_type: ColumnType::Text,
                values:   vec![item_raw.to_string()],
            }],
        };
        Self::eval_expr(expr, &synthetic, 0)
            .map(|v| v.to_raw_string())
    }

    /// Apply a callable expression to a list element passed as a `Value`.
    /// The element is bound to the implicit `_` parameter, so records and
    /// lists survive intact (unlike `apply_transform` which stringifies).
    fn apply_transform_value(
        expr:   &ExprNode,
        item:   &Value,
        source: &Table,
    ) -> ExecResult<Value> {
        push_underscore(item.clone());
        let result = Self::eval_expr(expr, source, 0);
        pop_underscore();
        result
    }

    /// Evaluate a predicate expression against a single item value.
    ///
    /// Returns `true` only for `Value::Bool(true)`.  All other outcomes
    /// (null, error, non-boolean) are treated as `false` per M null semantics.
    #[allow(dead_code)]
    fn apply_predicate(
        expr:    &ExprNode,
        item_raw: &str,
        source:  &Table,
    ) -> bool {
        let synthetic = Table {
            source:  source.source.clone(),
            sheet:   source.sheet.clone(),
            columns: vec![Column {
                name:     "_".into(),
                col_type: ColumnType::Text,
                values:   vec![item_raw.to_string()],
            }],
        };
        matches!(Self::eval_expr(expr, &synthetic, 0), Ok(Value::Bool(true)))
    }

    /// Apply a predicate to a `Value` list element via the implicit `_`
    /// binding.  Preserves record / list element shape so nested field
    /// access (e.g. `each _[User][Name] = "B"`) works correctly.
    fn apply_predicate_value(
        expr:   &ExprNode,
        item:   &Value,
        source: &Table,
    ) -> bool {
        push_underscore(item.clone());
        let r = Self::eval_expr(expr, source, 0);
        pop_underscore();
        matches!(r, Ok(Value::Bool(true)))
    }

    /// Handle a missing column according to MissingField semantics.
    ///
    /// - `None` / `Error` → return `Err(TypeMismatch)`
    /// - `Ignore`         → do nothing (caller must skip)
    /// - `UseNull`        → push a null-filled column and return `Ok(true)`
    ///                      indicating a new column was added
    ///
    /// Returns `Ok(true)` if a UseNull column was appended, `Ok(false)` if
    /// the column should be silently skipped (Ignore), or `Err` on Error mode.
    fn handle_missing_field(
        columns:       &mut Vec<Column>,
        col_name:      &str,
        col_type:      ColumnType,
        missing_field: Option<&MissingFieldKind>,
        row_count:     usize,
        context:       &str,
    ) -> ExecResult<bool> {
        match missing_field {
            Some(MissingFieldKind::Ignore) => Ok(false),
            Some(MissingFieldKind::UseNull) => {
                columns.push(Column {
                    name:     col_name.to_string(),
                    col_type,
                    values:   vec![String::new(); row_count],
                });
                Ok(true)
            }
            _ => Err(ExecError::TypeMismatch(format!(
                "{}: column '{}' does not exist in the table",
                context, col_name
            ))),
        }
    }
}

// ── ColumnsOfType matching helper ─────────────────────────────────────────

/// Returns `true` when `col_ty` matches the filter type `filter`.
///
/// Power Query's `type number` covers Integer, Float, and Currency.
/// All other types match exactly.
fn columns_of_type_match(col_ty: &ColumnType, filter: &ColumnType) -> bool {
    match filter {
        // "type number" matches all numeric ColumnTypes
        ColumnType::Float => matches!(col_ty, ColumnType::Float | ColumnType::Integer | ColumnType::Currency),
        _ => col_ty == filter,
    }
}