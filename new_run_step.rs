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
