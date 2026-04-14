use std::collections::HashMap;

use pq_ast::{
    Program,
    expr::{Expr, ExprNode},
    step::{StepKind, SortOrder},
};
use pq_grammar::operators::{Operator, UnaryOp};
use pq_pipeline::{Column, Table};
use pq_types::{infer_type, ColumnType};

use crate::value::Value;

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

        env.remove(&program.output)
            .ok_or_else(|| ExecError::UnknownStep(program.output.clone()))
    }

    // ── step dispatch ─────────────────────────────────────────────────────

    fn run_step(
        kind:   &StepKind,
        env:    &HashMap<String, Table>,
        source: &Table,
    ) -> ExecResult<Table> {
        match kind {

            // Source — just return the input table with updated source path
            StepKind::Source { path, .. } => {
                let mut t   = source.clone();
                t.source    = path.clone();
                Ok(t)
            }

            // PromoteHeaders — headers are already promoted by build_table, no-op
            StepKind::PromoteHeaders { input } => {
                Ok(Self::lookup(input, env, source)?.clone())
            }

            // ChangeTypes — update the col_type metadata on each listed column
            StepKind::ChangeTypes { input, columns } => {
                let mut t = Self::lookup(input, env, source)?.clone();
                for (col_name, new_type) in columns {
                    if let Some(col) = t.columns.iter_mut().find(|c| c.name == *col_name) {
                        col.col_type = new_type.clone();
                    }
                }
                Ok(t)
            }

            // condition is Each(inner_bool) — eval_expr unwraps Each
            StepKind::Filter { input, condition } => {
                let t         = Self::lookup(input, env, source)?;
                let row_count = t.row_count();
                let keep: Vec<usize> = (0..row_count)
                    .filter(|&i| matches!(
                        Self::eval_expr(condition, t, i),
                        Ok(Value::Bool(true))
                    ))
                    .collect();
                Ok(Self::select_rows(t, &keep))
            }

            // expression is Each(inner) — eval_expr unwraps Each
            StepKind::AddColumn { input, col_name, expression } => {
                let t         = Self::lookup(input, env, source)?;
                let row_count = t.row_count();
                let values: Vec<String> = (0..row_count)
                    .map(|i| Self::eval_expr(expression, t, i)
                        .map(|v| v.to_raw_string())
                        .unwrap_or_default())
                    .collect();
                let col_type = infer_type(&values);
                let mut result = t.clone();
                result.columns.push(Column { name: col_name.clone(), col_type, values });
                Ok(result)
            }

            StepKind::RemoveColumns { input, columns } => {
                let mut t = Self::lookup(input, env, source)?.clone();
                t.columns.retain(|c| !columns.contains(&c.name));
                Ok(t)
            }

            StepKind::RenameColumns { input, renames } => {
                let mut t = Self::lookup(input, env, source)?.clone();
                for col in t.columns.iter_mut() {
                    if let Some((_, new)) = renames.iter().find(|(old, _)| *old == col.name) {
                        col.name = new.clone();
                    }
                }
                Ok(t)
            }

            StepKind::Sort { input, by } => {
                let t         = Self::lookup(input, env, source)?;
                let row_count = t.row_count();
                let mut indices: Vec<usize> = (0..row_count).collect();

                indices.sort_by(|&a, &b| {
                    for (col_name, order) in by {
                        let av = Self::cell(t, col_name, a);
                        let bv = Self::cell(t, col_name, b);
                        let ct = t.get_column(col_name)
                            .map(|c| &c.col_type)
                            .unwrap_or(&ColumnType::Text);

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

            // Each transform expression is Each(inner) — eval_expr handles it.
            StepKind::TransformColumns { input, transforms } => {
                let t         = Self::lookup(input, env, source)?;
                let row_count = t.row_count();
                let mut result = t.clone();
                for (col_name, expr, _col_type) in transforms {
                    if let Some(col) = result.columns.iter_mut().find(|c| c.name == *col_name) {
                        col.values = (0..row_count)
                            .map(|i| Self::eval_expr(expr, t, i)
                                .map(|v| v.to_raw_string())
                                .unwrap_or_default())
                            .collect();
                    }
                }
                Ok(result)
            }

            // Basic grouping: group rows by key columns, compute aggregates.
            StepKind::Group { input, by, aggregates } => {
                let t = Self::lookup(input, env, source)?;

                // Collect unique key combinations in insertion order.
                let row_count = t.row_count();
                let mut key_order: Vec<Vec<String>> = Vec::new();
                let mut key_rows: HashMap<Vec<String>, Vec<usize>> = HashMap::new();

                for i in 0..row_count {
                    let key: Vec<String> = by.iter()
                        .map(|col| Self::cell(t, col, i).to_string())
                        .collect();
                    let entry = key_rows.entry(key.clone()).or_insert_with(|| {
                        key_order.push(key.clone());
                        Vec::new()
                    });
                    entry.push(i);
                }

                // Build output columns: first the group-by keys, then aggregates.
                let mut out_cols: Vec<Column> = by.iter().map(|col_name| {
                    let col_type = t.get_column(col_name)
                        .map(|c| c.col_type.clone())
                        .unwrap_or(ColumnType::Text);
                    Column {
                        name:     col_name.clone(),
                        col_type,
                        values:   key_order.iter()
                            .map(|k| k[by.iter().position(|c| c == col_name).unwrap()].clone())
                            .collect(),
                    }
                }).collect();

                for agg in aggregates {
                    let values: Vec<String> = key_order.iter().map(|key| {
                        let row_indices = &key_rows[key];
                        // Evaluate the aggregate expression on the first row as a proxy.
                        // Full aggregation (SUM, COUNT, etc.) requires function dispatch;
                        // for now return the value for the first row in each group.
                        row_indices.first()
                            .and_then(|&i| Self::eval_expr(&agg.expression, t, i).ok())
                            .map(|v| v.to_raw_string())
                            .unwrap_or_default()
                    }).collect();
                    out_cols.push(Column {
                        name:     agg.name.clone(),
                        col_type: agg.col_type.clone(),
                        values,
                    });
                }

                Ok(Table {
                    source:  t.source.clone(),
                    sheet:   t.sheet.clone(),
                    columns: out_cols,
                })
            }

            // Passthrough — forward input unchanged.
            StepKind::Passthrough { input, .. } => {
                if input.is_empty() {
                    Ok(Table { source: source.source.clone(), sheet: source.sheet.clone(), columns: vec![] })
                } else {
                    Ok(Self::lookup(input, env, source)?.clone())
                }
            }

            // ── FirstN ───────────────────────────────────────────────────
            StepKind::FirstN { input, count } => {
                let t = Self::lookup(input, env, source)?;
                let n = Self::eval_expr(count, t, 0).ok()
                    .and_then(|v| match v { Value::Int(i) => Some(i as usize), Value::Float(f) => Some(f as usize), _ => None })
                    .unwrap_or(1);
                let rc = t.row_count();
                let indices: Vec<usize> = (0..n.min(rc)).collect();
                Ok(Self::select_rows(t, &indices))
            }

            // ── LastN ────────────────────────────────────────────────────
            StepKind::LastN { input, count } => {
                let t = Self::lookup(input, env, source)?;
                let n = Self::eval_expr(count, t, 0).ok()
                    .and_then(|v| match v { Value::Int(i) => Some(i as usize), Value::Float(f) => Some(f as usize), _ => None })
                    .unwrap_or(1);
                let rc = t.row_count();
                let start = rc.saturating_sub(n);
                let indices: Vec<usize> = (start..rc).collect();
                Ok(Self::select_rows(t, &indices))
            }

            // ── Skip ─────────────────────────────────────────────────────
            StepKind::Skip { input, count } => {
                let t = Self::lookup(input, env, source)?;
                let n = Self::eval_expr(count, t, 0).ok()
                    .and_then(|v| match v { Value::Int(i) => Some(i as usize), Value::Float(f) => Some(f as usize), _ => None })
                    .unwrap_or(1);
                let rc = t.row_count();
                let indices: Vec<usize> = (n.min(rc)..rc).collect();
                Ok(Self::select_rows(t, &indices))
            }

            // ── Range ────────────────────────────────────────────────────
            StepKind::Range { input, offset, count } => {
                let t = Self::lookup(input, env, source)?;
                let off = Self::eval_expr(offset, t, 0).ok()
                    .and_then(|v| match v { Value::Int(i) => Some(i as usize), Value::Float(f) => Some(f as usize), _ => None })
                    .unwrap_or(0);
                let cnt = Self::eval_expr(count, t, 0).ok()
                    .and_then(|v| match v { Value::Int(i) => Some(i as usize), Value::Float(f) => Some(f as usize), _ => None })
                    .unwrap_or(1);
                let rc = t.row_count();
                let indices: Vec<usize> = (off.min(rc)..(off + cnt).min(rc)).collect();
                Ok(Self::select_rows(t, &indices))
            }

            // ── RemoveFirstN ─────────────────────────────────────────────
            StepKind::RemoveFirstN { input, count } => {
                let t = Self::lookup(input, env, source)?;
                let n = Self::eval_expr(count, t, 0).ok()
                    .and_then(|v| match v { Value::Int(i) => Some(i as usize), Value::Float(f) => Some(f as usize), _ => None })
                    .unwrap_or(1);
                let rc = t.row_count();
                let indices: Vec<usize> = (n.min(rc)..rc).collect();
                Ok(Self::select_rows(t, &indices))
            }

            // ── RemoveLastN ──────────────────────────────────────────────
            StepKind::RemoveLastN { input, count } => {
                let t = Self::lookup(input, env, source)?;
                let n = Self::eval_expr(count, t, 0).ok()
                    .and_then(|v| match v { Value::Int(i) => Some(i as usize), Value::Float(f) => Some(f as usize), _ => None })
                    .unwrap_or(1);
                let rc = t.row_count();
                let end = rc.saturating_sub(n);
                let indices: Vec<usize> = (0..end).collect();
                Ok(Self::select_rows(t, &indices))
            }

            // ── RemoveRows ───────────────────────────────────────────────
            StepKind::RemoveRows { input, offset, count } => {
                let t = Self::lookup(input, env, source)?;
                let off = Self::eval_expr(offset, t, 0).ok()
                    .and_then(|v| match v { Value::Int(i) => Some(i as usize), Value::Float(f) => Some(f as usize), _ => None })
                    .unwrap_or(0);
                let cnt = Self::eval_expr(count, t, 0).ok()
                    .and_then(|v| match v { Value::Int(i) => Some(i as usize), Value::Float(f) => Some(f as usize), _ => None })
                    .unwrap_or(1);
                let rc = t.row_count();
                let indices: Vec<usize> = (0..rc).filter(|&i| i < off || i >= off + cnt).collect();
                Ok(Self::select_rows(t, &indices))
            }

            // ── ReverseRows ──────────────────────────────────────────────
            StepKind::ReverseRows { input } => {
                let t = Self::lookup(input, env, source)?;
                let rc = t.row_count();
                let indices: Vec<usize> = (0..rc).rev().collect();
                Ok(Self::select_rows(t, &indices))
            }

            // ── Distinct ─────────────────────────────────────────────────
            StepKind::Distinct { input, columns } => {
                let t = Self::lookup(input, env, source)?;
                let rc = t.row_count();
                let mut seen = std::collections::HashSet::new();
                let mut indices: Vec<usize> = Vec::new();
                for i in 0..rc {
                    let key: Vec<String> = if columns.is_empty() {
                        t.columns.iter().map(|c| c.values.get(i).cloned().unwrap_or_default()).collect()
                    } else {
                        columns.iter().map(|col| Self::cell(t, col, i).to_string()).collect()
                    };
                    if seen.insert(key) {
                        indices.push(i);
                    }
                }
                Ok(Self::select_rows(t, &indices))
            }

            // ── Repeat ───────────────────────────────────────────────────
            StepKind::Repeat { input, count } => {
                let t = Self::lookup(input, env, source)?;
                let n = Self::eval_expr(count, t, 0).ok()
                    .and_then(|v| match v { Value::Int(i) => Some(i as usize), Value::Float(f) => Some(f as usize), _ => None })
                    .unwrap_or(1);
                let rc = t.row_count();
                let indices: Vec<usize> = (0..n).flat_map(|_| 0..rc).collect();
                Ok(Self::select_rows(t, &indices))
            }

            // ── AlternateRows ────────────────────────────────────────────
            StepKind::AlternateRows { input, offset, skip, take } => {
                let t = Self::lookup(input, env, source)?;
                let off = Self::eval_expr(offset, t, 0).ok()
                    .and_then(|v| match v { Value::Int(i) => Some(i as usize), _ => None }).unwrap_or(0);
                let sk  = Self::eval_expr(skip, t, 0).ok()
                    .and_then(|v| match v { Value::Int(i) => Some(i as usize), _ => None }).unwrap_or(1);
                let tk  = Self::eval_expr(take, t, 0).ok()
                    .and_then(|v| match v { Value::Int(i) => Some(i as usize), _ => None }).unwrap_or(1);
                let rc = t.row_count();
                let period = sk + tk;
                let indices: Vec<usize> = (0..rc)
                    .filter(|&i| i >= off && (i - off) % period < tk)
                    .collect();
                Ok(Self::select_rows(t, &indices))
            }

            // ── FindText ─────────────────────────────────────────────────
            StepKind::FindText { input, text } => {
                let t = Self::lookup(input, env, source)?;
                let rc = t.row_count();
                let needle = text.to_lowercase();
                let indices: Vec<usize> = (0..rc)
                    .filter(|&i| {
                        t.columns.iter().any(|c| {
                            c.values.get(i)
                                .map(|v| v.to_lowercase().contains(&needle))
                                .unwrap_or(false)
                        })
                    })
                    .collect();
                Ok(Self::select_rows(t, &indices))
            }

            // ── FillDown ─────────────────────────────────────────────────
            StepKind::FillDown { input, columns } => {
                let mut t = Self::lookup(input, env, source)?.clone();
                for col_name in columns {
                    if let Some(col) = t.columns.iter_mut().find(|c| c.name == *col_name) {
                        let mut last = String::new();
                        for val in col.values.iter_mut() {
                            if val.is_empty() || val == "null" {
                                *val = last.clone();
                            } else {
                                last = val.clone();
                            }
                        }
                    }
                }
                Ok(t)
            }

            // ── FillUp ───────────────────────────────────────────────────
            StepKind::FillUp { input, columns } => {
                let mut t = Self::lookup(input, env, source)?.clone();
                for col_name in columns {
                    if let Some(col) = t.columns.iter_mut().find(|c| c.name == *col_name) {
                        let mut last = String::new();
                        for val in col.values.iter_mut().rev() {
                            if val.is_empty() || val == "null" {
                                *val = last.clone();
                            } else {
                                last = val.clone();
                            }
                        }
                    }
                }
                Ok(t)
            }

            // ── AddIndexColumn ───────────────────────────────────────────
            StepKind::AddIndexColumn { input, col_name, start, step } => {
                let t = Self::lookup(input, env, source)?;
                let rc = t.row_count();
                let values: Vec<String> = (0..rc)
                    .map(|i| (start + (i as i64) * step).to_string())
                    .collect();
                let mut result = t.clone();
                result.columns.push(Column {
                    name: col_name.clone(),
                    col_type: ColumnType::Integer,
                    values,
                });
                Ok(result)
            }

            // ── DuplicateColumn ──────────────────────────────────────────
            StepKind::DuplicateColumn { input, src_col, new_col } => {
                let t = Self::lookup(input, env, source)?;
                let src_values = t.get_column(src_col)
                    .map(|c| c.values.clone())
                    .unwrap_or_default();
                let src_type = t.get_column(src_col)
                    .map(|c| c.col_type.clone())
                    .unwrap_or(ColumnType::Text);
                let mut result = t.clone();
                result.columns.push(Column {
                    name: new_col.clone(),
                    col_type: src_type,
                    values: src_values,
                });
                Ok(result)
            }

            // ── Unpivot ──────────────────────────────────────────────────
            StepKind::Unpivot { input, columns, attr_col, val_col } => {
                let t = Self::lookup(input, env, source)?;
                let rc = t.row_count();
                let keep_cols: Vec<&Column> = t.columns.iter()
                    .filter(|c| !columns.contains(&c.name))
                    .collect();
                let unpivot_cols: Vec<&Column> = t.columns.iter()
                    .filter(|c| columns.contains(&c.name))
                    .collect();

                let mut out_keep: Vec<Column> = keep_cols.iter().map(|c| Column {
                    name: c.name.clone(), col_type: c.col_type.clone(), values: vec![],
                }).collect();
                let mut out_attr = Column { name: attr_col.clone(), col_type: ColumnType::Text, values: vec![] };
                let mut out_val  = Column { name: val_col.clone(),  col_type: ColumnType::Text, values: vec![] };

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

            // ── UnpivotOtherColumns ──────────────────────────────────────
            StepKind::UnpivotOtherColumns { input, keep_cols, attr_col, val_col } => {
                let t = Self::lookup(input, env, source)?;
                let unpivot_names: Vec<String> = t.columns.iter()
                    .filter(|c| !keep_cols.contains(&c.name))
                    .map(|c| c.name.clone())
                    .collect();
                // Delegate to the same logic as Unpivot
                let rc = t.row_count();
                let kept: Vec<&Column> = t.columns.iter()
                    .filter(|c| keep_cols.contains(&c.name))
                    .collect();
                let unpivoted: Vec<&Column> = t.columns.iter()
                    .filter(|c| unpivot_names.contains(&c.name))
                    .collect();

                let mut out_keep: Vec<Column> = kept.iter().map(|c| Column {
                    name: c.name.clone(), col_type: c.col_type.clone(), values: vec![],
                }).collect();
                let mut out_attr = Column { name: attr_col.clone(), col_type: ColumnType::Text, values: vec![] };
                let mut out_val  = Column { name: val_col.clone(),  col_type: ColumnType::Text, values: vec![] };

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

            // ── Transpose ────────────────────────────────────────────────
            StepKind::Transpose { input } => {
                let t = Self::lookup(input, env, source)?;
                let rc = t.row_count();
                let cc = t.columns.len();
                let mut result_cols: Vec<Column> = (0..rc)
                    .map(|i| Column {
                        name: format!("Column{}", i + 1),
                        col_type: ColumnType::Text,
                        values: vec![],
                    })
                    .collect();
                for col in &t.columns {
                    for (i, val) in col.values.iter().enumerate() {
                        if i < result_cols.len() {
                            result_cols[i].values.push(val.clone());
                        }
                    }
                }
                // pad shorter rows
                for rc_col in result_cols.iter_mut() {
                    while rc_col.values.len() < cc {
                        rc_col.values.push(String::new());
                    }
                }
                Ok(Table { source: t.source.clone(), sheet: t.sheet.clone(), columns: result_cols })
            }

            // ── CombineTables ────────────────────────────────────────────
            StepKind::CombineTables { inputs } => {
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
                            let pad: Vec<String> = vec!["".to_string(); t.row_count()];
                            col.values.extend(pad);
                        }
                    }
                }
                Ok(result)
            }

            // ── RemoveRowsWithErrors ─────────────────────────────────────
            StepKind::RemoveRowsWithErrors { input, columns } => {
                let t = Self::lookup(input, env, source)?;
                let rc = t.row_count();
                let indices: Vec<usize> = (0..rc)
                    .filter(|&i| {
                        !columns.iter().any(|col_name| {
                            let val = Self::cell(t, col_name, i);
                            val.is_empty()
                        })
                    })
                    .collect();
                Ok(Self::select_rows(t, &indices))
            }

            // ── SelectRowsWithErrors ─────────────────────────────────────
            StepKind::SelectRowsWithErrors { input, columns } => {
                let t = Self::lookup(input, env, source)?;
                let rc = t.row_count();
                let indices: Vec<usize> = (0..rc)
                    .filter(|&i| {
                        columns.iter().any(|col_name| {
                            let val = Self::cell(t, col_name, i);
                            val.is_empty()
                        })
                    })
                    .collect();
                Ok(Self::select_rows(t, &indices))
            }

            // ── TransformRows ────────────────────────────────────────────
            StepKind::TransformRows { input, transform: _ } => {
                // Apply the row transform; since we can't fully represent records,
                // just evaluate the expression for each row and return the table.
                let t = Self::lookup(input, env, source)?;
                let _rc = t.row_count();
                // Simplified: return input unchanged (full record construction needed)
                Ok(t.clone())
            }

            // ── MatchesAllRows / MatchesAnyRows ──────────────────────────
            // These return boolean, not a table. Return a single-cell table.
            StepKind::MatchesAllRows { input, condition } => {
                let t = Self::lookup(input, env, source)?;
                let rc = t.row_count();
                let result = (0..rc).all(|i| matches!(
                    Self::eval_expr(condition, t, i),
                    Ok(Value::Bool(true))
                ));
                Ok(Table {
                    source: t.source.clone(),
                    sheet: t.sheet.clone(),
                    columns: vec![Column {
                        name: "Value".into(),
                        col_type: ColumnType::Boolean,
                        values: vec![result.to_string()],
                    }],
                })
            }

            StepKind::MatchesAnyRows { input, condition } => {
                let t = Self::lookup(input, env, source)?;
                let rc = t.row_count();
                let result = (0..rc).any(|i| matches!(
                    Self::eval_expr(condition, t, i),
                    Ok(Value::Bool(true))
                ));
                Ok(Table {
                    source: t.source.clone(),
                    sheet: t.sheet.clone(),
                    columns: vec![Column {
                        name: "Value".into(),
                        col_type: ColumnType::Boolean,
                        values: vec![result.to_string()],
                    }],
                })
            }

            // ── PrefixColumns ────────────────────────────────────────────
            StepKind::PrefixColumns { input, prefix } => {
                let mut t = Self::lookup(input, env, source)?.clone();
                for col in t.columns.iter_mut() {
                    col.name = format!("{}.{}", prefix, col.name);
                }
                Ok(t)
            }

            // ── DemoteHeaders ────────────────────────────────────────────
            StepKind::DemoteHeaders { input } => {
                let t = Self::lookup(input, env, source)?;
                let mut result_cols: Vec<Column> = Vec::new();
                for (i, col) in t.columns.iter().enumerate() {
                    let mut values = vec![col.name.clone()];
                    values.extend(col.values.iter().cloned());
                    result_cols.push(Column {
                        name: format!("Column{}", i + 1),
                        col_type: ColumnType::Text,
                        values,
                    });
                }
                Ok(Table { source: t.source.clone(), sheet: t.sheet.clone(), columns: result_cols })
            }

            // ── SelectColumns ────────────────────────────────────────────
            StepKind::SelectColumns { input, columns } => {
                let t = Self::lookup(input, env, source)?;
                let result_cols: Vec<Column> = columns.iter()
                    .filter_map(|name| t.get_column(name).cloned())
                    .collect();
                Ok(Table { source: t.source.clone(), sheet: t.sheet.clone(), columns: result_cols })
            }

            // ── ReorderColumns ───────────────────────────────────────────
            StepKind::ReorderColumns { input, columns } => {
                let t = Self::lookup(input, env, source)?;
                let mut result_cols: Vec<Column> = columns.iter()
                    .filter_map(|name| t.get_column(name).cloned())
                    .collect();
                for col in &t.columns {
                    if !columns.contains(&col.name) {
                        result_cols.push(col.clone());
                    }
                }
                Ok(Table { source: t.source.clone(), sheet: t.sheet.clone(), columns: result_cols })
            }

            // ── TransformColumnNames ─────────────────────────────────────
            StepKind::TransformColumnNames { input, transform } => {
                let t = Self::lookup(input, env, source)?;
                let mut result = t.clone();
                for col in result.columns.iter_mut() {
                    let name_table = Table {
                        source: t.source.clone(),
                        sheet: t.sheet.clone(),
                        columns: vec![Column {
                            name: "_".into(),
                            col_type: ColumnType::Text,
                            values: vec![col.name.clone()],
                        }],
                    };
                    if let Ok(v) = Self::eval_expr(transform, &name_table, 0) {
                        col.name = v.to_raw_string();
                    }
                }
                Ok(result)
            }

            // ── CombineColumns ───────────────────────────────────────────
            StepKind::CombineColumns { input, columns, combiner, new_col } => {
                let t = Self::lookup(input, env, source)?;
                let rc = t.row_count();
                let values: Vec<String> = (0..rc).map(|i| {
                    let parts: Vec<String> = columns.iter()
                        .map(|c| Self::cell(t, c, i).to_string())
                        .collect();
                    let combined = parts.join(", ");
                    let synthetic = Table {
                        source: t.source.clone(),
                        sheet: t.sheet.clone(),
                        columns: vec![Column {
                            name: "_".into(),
                            col_type: ColumnType::Text,
                            values: vec![combined],
                        }],
                    };
                    Self::eval_expr(combiner, &synthetic, 0)
                        .map(|v| v.to_raw_string())
                        .unwrap_or_default()
                }).collect();
                let mut result = t.clone();
                result.columns.retain(|c| !columns.contains(&c.name));
                result.columns.push(Column {
                    name: new_col.clone(),
                    col_type: ColumnType::Text,
                    values,
                });
                Ok(result)
            }

            // ── SplitColumn ──────────────────────────────────────────────
            StepKind::SplitColumn { input, col_name, splitter } => {
                let t = Self::lookup(input, env, source)?;
                let rc = t.row_count();
                let mut max_parts = 0usize;
                let split_results: Vec<Vec<String>> = (0..rc).map(|i| {
                    let cell_val = Self::cell(t, col_name, i).to_string();
                    let synthetic = Table {
                        source: t.source.clone(),
                        sheet: t.sheet.clone(),
                        columns: vec![Column {
                            name: "_".into(),
                            col_type: ColumnType::Text,
                            values: vec![cell_val],
                        }],
                    };
                    let result = Self::eval_expr(splitter, &synthetic, 0)
                        .map(|v| v.to_raw_string())
                        .unwrap_or_default();
                    let parts: Vec<String> = result.split(',').map(|s| s.to_string()).collect();
                    if parts.len() > max_parts { max_parts = parts.len(); }
                    parts
                }).collect();
                let mut result = t.clone();
                result.columns.retain(|c| c.name != *col_name);
                for p in 0..max_parts {
                    let values: Vec<String> = split_results.iter()
                        .map(|parts| parts.get(p).cloned().unwrap_or_default())
                        .collect();
                    result.columns.push(Column {
                        name: format!("{}.{}", col_name, p + 1),
                        col_type: ColumnType::Text,
                        values,
                    });
                }
                Ok(result)
            }

            // ── ExpandTableColumn / ExpandRecordColumn ───────────────────
            StepKind::ExpandTableColumn { input, col_name, columns }
            | StepKind::ExpandRecordColumn { input, col_name, fields: columns } => {
                let t = Self::lookup(input, env, source)?;
                let rc = t.row_count();
                let mut result = t.clone();
                result.columns.retain(|c| c.name != *col_name);
                for c in columns {
                    result.columns.push(Column {
                        name: c.clone(),
                        col_type: ColumnType::Text,
                        values: vec![String::new(); rc],
                    });
                }
                Ok(result)
            }

            // ── Pivot ────────────────────────────────────────────────────
            StepKind::Pivot { input, pivot_col: _pivot_col, attr_col, val_col } => {
                let t = Self::lookup(input, env, source)?;
                let rc = t.row_count();
                let keep_cols: Vec<&Column> = t.columns.iter()
                    .filter(|c| c.name != *attr_col && c.name != *val_col)
                    .collect();

                // Collect unique attribute values
                let mut attr_values: Vec<String> = Vec::new();
                if let Some(attr_c) = t.get_column(attr_col) {
                    for v in &attr_c.values {
                        if !attr_values.contains(v) {
                            attr_values.push(v.clone());
                        }
                    }
                }

                // Group by keep columns
                let mut key_order: Vec<Vec<String>> = Vec::new();
                let mut key_rows: HashMap<Vec<String>, HashMap<String, String>> = HashMap::new();
                for i in 0..rc {
                    let key: Vec<String> = keep_cols.iter()
                        .map(|c| c.values.get(i).cloned().unwrap_or_default())
                        .collect();
                    let attr = Self::cell(t, attr_col, i).to_string();
                    let val = Self::cell(t, val_col, i).to_string();
                    let entry = key_rows.entry(key.clone()).or_insert_with(|| {
                        key_order.push(key.clone());
                        HashMap::new()
                    });
                    entry.insert(attr, val);
                }

                let mut out_cols: Vec<Column> = keep_cols.iter().enumerate().map(|(j, c)| {
                    Column {
                        name: c.name.clone(),
                        col_type: c.col_type.clone(),
                        values: key_order.iter().map(|k| k[j].clone()).collect(),
                    }
                }).collect();

                for attr in &attr_values {
                    let values: Vec<String> = key_order.iter()
                        .map(|k| key_rows[k].get(attr).cloned().unwrap_or_default())
                        .collect();
                    out_cols.push(Column {
                        name: attr.clone(),
                        col_type: ColumnType::Text,
                        values,
                    });
                }

                Ok(Table { source: t.source.clone(), sheet: t.sheet.clone(), columns: out_cols })
            }

            // ── RowCount ─────────────────────────────────────────────────
            StepKind::RowCount { input } => {
                let t = Self::lookup(input, env, source)?;
                Ok(Table {
                    source: t.source.clone(), sheet: t.sheet.clone(),
                    columns: vec![Column { name: "Value".into(), col_type: ColumnType::Integer, values: vec![t.row_count().to_string()] }],
                })
            }

            // ── ColumnCount ──────────────────────────────────────────────
            StepKind::ColumnCount { input } => {
                let t = Self::lookup(input, env, source)?;
                Ok(Table {
                    source: t.source.clone(), sheet: t.sheet.clone(),
                    columns: vec![Column { name: "Value".into(), col_type: ColumnType::Integer, values: vec![t.columns.len().to_string()] }],
                })
            }

            // ── ColumnNames ──────────────────────────────────────────────
            StepKind::TableColumnNames { input } => {
                let t = Self::lookup(input, env, source)?;
                let names: Vec<String> = t.columns.iter().map(|c| c.name.clone()).collect();
                Ok(Table {
                    source: t.source.clone(), sheet: t.sheet.clone(),
                    columns: vec![Column { name: "Value".into(), col_type: ColumnType::Text, values: names }],
                })
            }

            // ── TableIsEmpty ─────────────────────────────────────────────
            StepKind::TableIsEmpty { input } => {
                let t = Self::lookup(input, env, source)?;
                Ok(Table {
                    source: t.source.clone(), sheet: t.sheet.clone(),
                    columns: vec![Column { name: "Value".into(), col_type: ColumnType::Boolean, values: vec![(t.row_count() == 0).to_string()] }],
                })
            }

            // ── TableSchema ──────────────────────────────────────────────
            StepKind::TableSchema { input } => {
                let t = Self::lookup(input, env, source)?;
                let names: Vec<String> = t.columns.iter().map(|c| c.name.clone()).collect();
                let kinds: Vec<String> = t.columns.iter().map(|c| c.col_type.to_string()).collect();
                let nullables: Vec<String> = t.columns.iter().map(|_| "true".to_string()).collect();
                Ok(Table {
                    source: t.source.clone(), sheet: t.sheet.clone(),
                    columns: vec![
                        Column { name: "Name".into(), col_type: ColumnType::Text, values: names },
                        Column { name: "Kind".into(), col_type: ColumnType::Text, values: kinds },
                        Column { name: "IsNullable".into(), col_type: ColumnType::Boolean, values: nullables },
                    ],
                })
            }

            // ── HasColumns ───────────────────────────────────────────────
            StepKind::HasColumns { input, columns } => {
                let t = Self::lookup(input, env, source)?;
                let has_all = columns.iter().all(|c| t.get_column(c).is_some());
                Ok(Table {
                    source: t.source.clone(), sheet: t.sheet.clone(),
                    columns: vec![Column { name: "Value".into(), col_type: ColumnType::Boolean, values: vec![has_all.to_string()] }],
                })
            }

            // ── TableIsDistinct ──────────────────────────────────────────
            StepKind::TableIsDistinct { input } => {
                let t = Self::lookup(input, env, source)?;
                let rc = t.row_count();
                let mut seen = std::collections::HashSet::new();
                let mut is_distinct = true;
                for i in 0..rc {
                    let key: Vec<String> = t.columns.iter()
                        .map(|c| c.values.get(i).cloned().unwrap_or_default())
                        .collect();
                    if !seen.insert(key) {
                        is_distinct = false;
                        break;
                    }
                }
                Ok(Table {
                    source: t.source.clone(), sheet: t.sheet.clone(),
                    columns: vec![Column { name: "Value".into(), col_type: ColumnType::Boolean, values: vec![is_distinct.to_string()] }],
                })
            }

            // ── Join ─────────────────────────────────────────────────────
            StepKind::Join { left, left_keys, right, right_keys, join_kind } => {
                let lt = Self::lookup(left, env, source)?;
                let rt = Self::lookup(right, env, source)?;
                let lrc = lt.row_count();
                let rrc = rt.row_count();

                // Build output columns: all from left, then unique from right
                let mut out_cols: Vec<Column> = lt.columns.iter().map(|c| Column {
                    name: c.name.clone(), col_type: c.col_type.clone(), values: vec![],
                }).collect();
                let right_only: Vec<&Column> = rt.columns.iter()
                    .filter(|c| !lt.columns.iter().any(|lc| lc.name == c.name))
                    .collect();
                for rc in &right_only {
                    out_cols.push(Column { name: rc.name.clone(), col_type: rc.col_type.clone(), values: vec![] });
                }

                let mut left_matched = vec![false; lrc];

                for li in 0..lrc {
                    let lkey: Vec<String> = left_keys.iter()
                        .map(|k| Self::cell(lt, k, li).to_string())
                        .collect();
                    let mut matched = false;
                    for ri in 0..rrc {
                        let rkey: Vec<String> = right_keys.iter()
                            .map(|k| Self::cell(rt, k, ri).to_string())
                            .collect();
                        if lkey == rkey {
                            matched = true;
                            left_matched[li] = true;
                            // Add left values
                            for (j, lc) in lt.columns.iter().enumerate() {
                                out_cols[j].values.push(lc.values.get(li).cloned().unwrap_or_default());
                            }
                            // Add right-only values
                            let lt_col_count = lt.columns.len();
                            for (j, rc_col) in right_only.iter().enumerate() {
                                out_cols[lt_col_count + j].values.push(rc_col.values.get(ri).cloned().unwrap_or_default());
                            }
                        }
                    }
                    if !matched && matches!(join_kind, pq_ast::step::JoinKind::Left | pq_ast::step::JoinKind::Full) {
                        for (j, lc) in lt.columns.iter().enumerate() {
                            out_cols[j].values.push(lc.values.get(li).cloned().unwrap_or_default());
                        }
                        let lt_col_count = lt.columns.len();
                        for j in 0..right_only.len() {
                            out_cols[lt_col_count + j].values.push(String::new());
                        }
                    }
                }

                // Right/Full: add unmatched right rows
                if matches!(join_kind, pq_ast::step::JoinKind::Right | pq_ast::step::JoinKind::Full) {
                    for ri in 0..rrc {
                        let rkey: Vec<String> = right_keys.iter()
                            .map(|k| Self::cell(rt, k, ri).to_string())
                            .collect();
                        let any_match = (0..lrc).any(|li| {
                            let lkey: Vec<String> = left_keys.iter()
                                .map(|k| Self::cell(lt, k, li).to_string())
                                .collect();
                            lkey == rkey
                        });
                        if !any_match {
                            for (j, _) in lt.columns.iter().enumerate() {
                                // For shared key cols, use right key value
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

            // ── NestedJoin ───────────────────────────────────────────────
            StepKind::NestedJoin { left, left_keys, right, right_keys, new_col, .. } => {
                let lt = Self::lookup(left, env, source)?;
                let rt = Self::lookup(right, env, source)?;
                let lrc = lt.row_count();
                let rrc = rt.row_count();
                let mut result = lt.clone();

                // For each left row, find matching right rows and store as "Table" placeholder
                let mut nested_values: Vec<String> = Vec::with_capacity(lrc);
                for li in 0..lrc {
                    let lkey: Vec<String> = left_keys.iter()
                        .map(|k| Self::cell(lt, k, li).to_string())
                        .collect();
                    let matches: Vec<usize> = (0..rrc).filter(|&ri| {
                        let rkey: Vec<String> = right_keys.iter()
                            .map(|k| Self::cell(rt, k, ri).to_string())
                            .collect();
                        lkey == rkey
                    }).collect();
                    nested_values.push(format!("Table({})", matches.len()));
                }

                result.columns.push(Column {
                    name: new_col.clone(),
                    col_type: ColumnType::Text,
                    values: nested_values,
                });
                Ok(result)
            }

            // ── AddRankColumn ────────────────────────────────────────────
            StepKind::AddRankColumn { input, col_name, by } => {
                let t = Self::lookup(input, env, source)?;
                let rc = t.row_count();
                let mut indices: Vec<usize> = (0..rc).collect();

                indices.sort_by(|&a, &b| {
                    for (sort_col, order) in by {
                        let av = Self::cell(t, sort_col, a);
                        let bv = Self::cell(t, sort_col, b);
                        let ct = t.get_column(sort_col)
                            .map(|c| &c.col_type)
                            .unwrap_or(&ColumnType::Text);
                        let cmp = Self::compare_raw(av, bv, ct);
                        let cmp = match order {
                            SortOrder::Ascending  => cmp,
                            SortOrder::Descending => cmp.reverse(),
                        };
                        if cmp != std::cmp::Ordering::Equal { return cmp; }
                    }
                    std::cmp::Ordering::Equal
                });

                let mut ranks = vec![0usize; rc];
                for (rank, &idx) in indices.iter().enumerate() {
                    ranks[idx] = rank + 1;
                }

                let mut result = t.clone();
                result.columns.push(Column {
                    name: col_name.clone(),
                    col_type: ColumnType::Integer,
                    values: ranks.iter().map(|r| r.to_string()).collect(),
                });
                Ok(result)
            }

            // ── TableMax ─────────────────────────────────────────────────
            StepKind::TableMax { input, col_name } => {
                let t = Self::lookup(input, env, source)?;
                let rc = t.row_count();
                if rc == 0 { return Ok(t.clone()); }
                let ct = t.get_column(col_name)
                    .map(|c| &c.col_type)
                    .unwrap_or(&ColumnType::Text);
                let best = (0..rc).max_by(|&a, &b| {
                    Self::compare_raw(Self::cell(t, col_name, a), Self::cell(t, col_name, b), ct)
                }).unwrap_or(0);
                Ok(Self::select_rows(t, &[best]))
            }

            // ── TableMin ─────────────────────────────────────────────────
            StepKind::TableMin { input, col_name } => {
                let t = Self::lookup(input, env, source)?;
                let rc = t.row_count();
                if rc == 0 { return Ok(t.clone()); }
                let ct = t.get_column(col_name)
                    .map(|c| &c.col_type)
                    .unwrap_or(&ColumnType::Text);
                let best = (0..rc).min_by(|&a, &b| {
                    Self::compare_raw(Self::cell(t, col_name, a), Self::cell(t, col_name, b), ct)
                }).unwrap_or(0);
                Ok(Self::select_rows(t, &[best]))
            }

            // ── TableMaxN ────────────────────────────────────────────────
            StepKind::TableMaxN { input, count, col_name } => {
                let t = Self::lookup(input, env, source)?;
                let n = Self::eval_expr(count, t, 0).ok()
                    .and_then(|v| match v { Value::Int(i) => Some(i as usize), _ => None })
                    .unwrap_or(1);
                let rc = t.row_count();
                let ct = t.get_column(col_name).map(|c| &c.col_type).unwrap_or(&ColumnType::Text);
                let mut indices: Vec<usize> = (0..rc).collect();
                indices.sort_by(|&a, &b| {
                    Self::compare_raw(Self::cell(t, col_name, b), Self::cell(t, col_name, a), ct)
                });
                indices.truncate(n);
                Ok(Self::select_rows(t, &indices))
            }

            // ── TableMinN ────────────────────────────────────────────────
            StepKind::TableMinN { input, count, col_name } => {
                let t = Self::lookup(input, env, source)?;
                let n = Self::eval_expr(count, t, 0).ok()
                    .and_then(|v| match v { Value::Int(i) => Some(i as usize), _ => None })
                    .unwrap_or(1);
                let rc = t.row_count();
                let ct = t.get_column(col_name).map(|c| &c.col_type).unwrap_or(&ColumnType::Text);
                let mut indices: Vec<usize> = (0..rc).collect();
                indices.sort_by(|&a, &b| {
                    Self::compare_raw(Self::cell(t, col_name, a), Self::cell(t, col_name, b), ct)
                });
                indices.truncate(n);
                Ok(Self::select_rows(t, &indices))
            }

            // ── ReplaceValue ─────────────────────────────────────────────
            StepKind::ReplaceValue { input, old_value, new_value, .. } => {
                let t = Self::lookup(input, env, source)?;
                let old_v = Self::eval_expr(old_value, t, 0).ok()
                    .map(|v| v.to_raw_string())
                    .unwrap_or_default();
                let new_v = Self::eval_expr(new_value, t, 0).ok()
                    .map(|v| v.to_raw_string())
                    .unwrap_or_default();
                let mut result = t.clone();
                for col in result.columns.iter_mut() {
                    for val in col.values.iter_mut() {
                        if *val == old_v {
                            *val = new_v.clone();
                        }
                    }
                }
                Ok(result)
            }

            // ── ReplaceErrorValues ───────────────────────────────────────
            StepKind::ReplaceErrorValues { input, replacements } => {
                let t = Self::lookup(input, env, source)?;
                let mut result = t.clone();
                for (col_name, replacement, _) in replacements {
                    let repl_val = Self::eval_expr(replacement, t, 0).ok()
                        .map(|v| v.to_raw_string())
                        .unwrap_or_default();
                    if let Some(col) = result.columns.iter_mut().find(|c| c.name == *col_name) {
                        for val in col.values.iter_mut() {
                            if val.is_empty() || val == "null" {
                                *val = repl_val.clone();
                            }
                        }
                    }
                }
                Ok(result)
            }

            // ── InsertRows ───────────────────────────────────────────────
            StepKind::InsertRows { input, .. } => {
                // Simplified: return input unchanged (record construction needs full support)
                let t = Self::lookup(input, env, source)?;
                Ok(t.clone())
            }

            // ListGenerate — produce a list by iterating from initial while
            // condition holds, stepping with next, projecting with selector.
            StepKind::ListGenerate { initial, condition, next, selector } => {
                // Evaluate the seed (zero-arg lambda or plain expression).
                let seed = Self::eval_lambda_or_expr(initial, env, source)?;

                let mut items: Vec<Value> = vec![];
                let mut current = seed;
                loop {
                    // Build a synthetic row with the current value as `_`.
                    let row_table = Self::single_value_table(source, &current);
                    // Check condition.
                    let cond = Self::eval_expr(condition, &row_table, 0)?;
                    if !matches!(cond, Value::Bool(true)) {
                        break;
                    }
                    // Project (selector) or keep raw value.
                    let out = if let Some(sel) = selector {
                        Self::eval_expr(sel, &row_table, 0)?
                    } else {
                        current.clone()
                    };
                    items.push(out);
                    // Step to next value.
                    current = Self::eval_expr(next, &row_table, 0)?;
                }

                let values: Vec<String> = items.iter().map(|v| v.to_raw_string()).collect();
                let col_type = infer_type(&values);
                Ok(Table {
                    source:  source.source.clone(),
                    sheet:   source.sheet.clone(),
                    columns: vec![Column { name: "Value".into(), col_type, values }],
                })
            }

            // ListTransform — apply a per-element lambda to every item in a
            // list expression and return a single-column "Value" table.
            StepKind::ListTransform { list_expr, transform } => {
                // 1. Collect the source items.
                let items = Self::eval_to_list(list_expr, env, source)?;

                // 2. Apply transform to each item by building a synthetic
                //    single-row table where column "_" holds the item value.
                let values: Vec<String> = items.iter().map(|item| {
                    let row_str = item.to_raw_string();
                    let synthetic = Table {
                        source:  source.source.clone(),
                        sheet:   source.sheet.clone(),
                        columns: vec![Column {
                            name:     "_".into(),
                            col_type: ColumnType::Text,
                            values:   vec![row_str],
                        }],
                    };
                    Self::eval_expr(transform, &synthetic, 0)
                        .map(|v| v.to_raw_string())
                        .unwrap_or_default()
                }).collect();

                // 3. Wrap results in a table with a single "Value" column.
                let col_type = infer_type(&values);
                Ok(Table {
                    source:  source.source.clone(),
                    sheet:   source.sheet.clone(),
                    columns: vec![Column { name: "Value".into(), col_type, values }],
                })
            }
        }
    }

    // ── expression evaluator ──────────────────────────────────────────────

    fn eval_expr(node: &ExprNode, table: &Table, row: usize) -> ExecResult<Value> {
        match &node.expr {
            // ── literals ──────────────────────────────────────────────────
            Expr::IntLit(n)    => Ok(Value::Int(*n)),
            Expr::FloatLit(n)  => Ok(Value::Float(*n)),
            Expr::BoolLit(b)   => Ok(Value::Bool(*b)),
            Expr::StringLit(s) => Ok(Value::Text(s.clone())),
            Expr::NullLit      => Ok(Value::Null),

            // ── column references ─────────────────────────────────────────
            // Identifier covers bare names (`Age`, `_`, `Order.Ascending`).
            // `_` is the implicit each param — look it up as a column.
            Expr::Identifier(name) => {
                let raw      = Self::cell(table, name, row);
                let col_type = table.get_column(name)
                    .map(|c| &c.col_type)
                    .unwrap_or(&ColumnType::Text);
                Ok(Self::coerce(raw, col_type))
            }
            Expr::ColumnAccess(name) => {
                let raw      = Self::cell(table, name, row);
                let col_type = table.get_column(name)
                    .map(|c| &c.col_type)
                    .unwrap_or(&ColumnType::Text);
                Ok(Self::coerce(raw, col_type))
            }

            // Field access on a named record variable: `row[ColName]`.
            // `record` is bound to the current row so we can treat `field`
            // as a plain column access on the current table row.
            Expr::FieldAccess { field, .. } => {
                let raw      = Self::cell(table, field, row);
                let col_type = table.get_column(field)
                    .map(|c| &c.col_type)
                    .unwrap_or(&ColumnType::Text);
                Ok(Self::coerce(raw, col_type))
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

            // ── collections — return Null for scalar contexts ─────────────
            Expr::List(_) | Expr::Record(_) => Ok(Value::Null),
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
    /// - `Expr::List` → evaluate every element in the current source context.
    /// - `Expr::Identifier(name)` → look up a previous step and collect every
    ///   value from its first column.
    /// - Anything else → evaluate as a single scalar and wrap in a Vec.
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

            // Fallback: evaluate as scalar and return as a one-element list.
            _ => Ok(vec![Self::eval_expr(expr, source, 0)?]),
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
}
