    fn annotate_step_exprs(
        &mut self,
        kind:   &mut StepKind,
        schema: Option<&[(String, ColumnType)]>,
    ) {
        match kind {
            StepKind::Source { .. } | StepKind::NavigateSheet { .. } => {}
            StepKind::ValueBinding { expr } => { self.infer_expr_mut(expr, schema); }
            StepKind::FunctionCall { name, args } => {
                match name.as_str() {
                    // SelectRows / row-predicate functions: validate Boolean result
                    "Table.SelectRows" => {
                        if let Some(CallArg::Expr(cond)) = args.get_mut(1) {
                            if let Some(t) = self.infer_expr_mut(cond, schema) {
                                let body_ty = match t { ColumnType::Function(inner) => *inner, other => other };
                                if body_ty != ColumnType::Boolean {
                                    self.diagnostics.push(
                                        Diagnostic::error("E405", format!("filter condition must be Boolean, got '{}'", body_ty))
                                            .with_label(cond.span.clone(), "this must produce a Boolean value")
                                            .with_suggestion("use a comparison operator like >, <, =, <>")
                                    );
                                }
                            }
                        }
                    }
                    // TransformColumns: bind `_` to each column's type
                    "Table.TransformColumns" => {
                        if let Some(CallArg::TransformList(transforms)) = args.get_mut(1) {
                            for (col_name, expr, _) in transforms.iter_mut() {
                                let col_type = schema.and_then(|s| Self::lookup_col(Some(s), col_name));
                                let old = self.lambda_param.take();
                                self.lambda_param = col_type;
                                self.infer_expr_mut(expr, schema);
                                self.lambda_param = old;
                            }
                        }
                    }
                    // Default: infer types for all Expr / AggList / TransformList args
                    _ => {
                        for arg in args.iter_mut() {
                            match arg {
                                CallArg::Expr(e) => { self.infer_expr_mut(e, schema); }
                                CallArg::AggList(aggs) => {
                                    for agg in aggs.iter_mut() {
                                        self.infer_expr_mut(&mut agg.expression, schema);
                                    }
                                }
                                CallArg::TransformList(transforms) => {
                                    for (_, expr, _) in transforms.iter_mut() {
                                        self.infer_expr_mut(expr, schema);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }

    fn compute_output_schema(
        &self,
        kind:         &StepKind,
        input_schema: Option<Vec<(String, ColumnType)>>,
    ) -> Option<Vec<(String, ColumnType)>> {
        match kind {
            StepKind::Source { .. } => Some(
                self.table.columns.iter()
                    .map(|c| (c.name.clone(), c.col_type.clone()))
                    .collect()
            ),
            StepKind::NavigateSheet { .. } => input_schema,
            StepKind::ValueBinding { .. } => None,
            StepKind::FunctionCall { name, args } => {
                match name.as_str() {
                    // ── Passthrough schema ───────────────────────────────────
                    "Table.PromoteHeaders"
                    | "Table.SelectRows" | "Table.Sort" | "Table.ReverseRows"
                    | "Table.Distinct" | "Table.Repeat" | "Table.AlternateRows"
                    | "Table.FindText" | "Table.FillDown" | "Table.FillUp"
                    | "Table.FirstN" | "Table.LastN" | "Table.Skip" | "Table.Range"
                    | "Table.RemoveFirstN" | "Table.RemoveLastN" | "Table.RemoveRows"
                    | "Table.RemoveRowsWithErrors" | "Table.SelectRowsWithErrors"
                    | "Table.TransformRows" | "Table.DemoteHeaders"
                    | "Table.Max" | "Table.Min" | "Table.MaxN" | "Table.MinN"
                    | "Table.ReplaceValue" | "Table.ReplaceErrorValues"
                    | "Table.InsertRows" | "Table.ReplaceRows"
                    | "Table.Buffer" | "Table.StopFolding" | "Table.ConformToPageReader"
                    | "Table.TransformColumns"
                    => input_schema,

                    // ── TransformColumnTypes: update column types ────────────
                    "Table.TransformColumnTypes" => {
                        let columns = args.get(1).and_then(|a| a.as_type_list()).unwrap_or(&[]);
                        let mut schema = input_schema?;
                        for (col_name, new_ty) in columns {
                            if let Some(col) = schema.iter_mut().find(|(n, _)| n == col_name) {
                                col.1 = new_ty.clone();
                            }
                            // Missing columns silently skipped (resolver validates)
                        }
                        Some(schema)
                    }

                    // ── AddColumn: add new typed column ─────────────────────
                    "Table.AddColumn" => {
                        let col_name = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                        let expr_ty  = args.get(2).and_then(|a| a.as_expr())
                            .and_then(|e| e.inferred_type.clone())
                            .unwrap_or(ColumnType::Text);
                        let mut schema = input_schema?;
                        schema.push((col_name.to_string(), expr_ty));
                        Some(schema)
                    }

                    // ── RemoveColumns: drop columns ──────────────────────────
                    "Table.RemoveColumns" => {
                        let cols = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        input_schema.map(|s| s.into_iter().filter(|(n, _)| !cols.contains(n)).collect())
                    }

                    // ── RenameColumns: rename columns ────────────────────────
                    "Table.RenameColumns" => {
                        let renames = args.get(1).and_then(|a| a.as_rename_list()).unwrap_or(&[]);
                        input_schema.map(|cols| cols.into_iter().map(|(n, t)| {
                            let new_n = renames.iter().find(|(old, _)| old == &n)
                                .map(|(_, new)| new.clone()).unwrap_or(n);
                            (new_n, t)
                        }).collect())
                    }

                    // ── Group: by columns + aggregate outputs ────────────────
                    "Table.Group" | "Table.FuzzyGroup" => {
                        let by  = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let agg = args.get(2).and_then(|a| a.as_agg_list()).unwrap_or(&[]);
                        let mut schema: Vec<(String, ColumnType)> = by.iter().map(|c| {
                            let ty = input_schema.as_ref()
                                .and_then(|s| s.iter().find(|(n, _)| n == c).map(|(_, t)| t.clone()))
                                .unwrap_or(ColumnType::Text);
                            (c.clone(), ty)
                        }).collect();
                        for a in agg { schema.push((a.name.clone(), a.col_type.clone())); }
                        Some(schema)
                    }

                    // ── AddIndexColumn: add integer index column ─────────────
                    "Table.AddIndexColumn" => {
                        let col_name = args.get(1).and_then(|a| a.as_str()).unwrap_or("Index");
                        let mut schema = input_schema?;
                        schema.push((col_name.to_string(), ColumnType::Integer));
                        Some(schema)
                    }

                    // ── DuplicateColumn: copy column with new name ────────────
                    "Table.DuplicateColumn" => {
                        let src_col = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                        let new_col = args.get(2).and_then(|a| a.as_str()).unwrap_or("");
                        let src_ty = input_schema.as_ref()
                            .and_then(|s| s.iter().find(|(n, _)| n == src_col).map(|(_, t)| t.clone()))
                            .unwrap_or(ColumnType::Text);
                        let mut schema = input_schema?;
                        schema.push((new_col.to_string(), src_ty));
                        Some(schema)
                    }

                    // ── Unpivot ──────────────────────────────────────────────
                    "Table.Unpivot" => {
                        let columns  = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let attr_col = args.get(2).and_then(|a| a.as_str()).unwrap_or("Attribute");
                        let val_col  = args.get(3).and_then(|a| a.as_str()).unwrap_or("Value");
                        input_schema.map(|s| {
                            let mut out: Vec<(String, ColumnType)> = s.into_iter()
                                .filter(|(n, _)| !columns.contains(n)).collect();
                            out.push((attr_col.to_string(), ColumnType::Text));
                            out.push((val_col.to_string(), ColumnType::Text));
                            out
                        })
                    }

                    // ── UnpivotOtherColumns ──────────────────────────────────
                    "Table.UnpivotOtherColumns" => {
                        let keep     = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let attr_col = args.get(2).and_then(|a| a.as_str()).unwrap_or("Attribute");
                        let val_col  = args.get(3).and_then(|a| a.as_str()).unwrap_or("Value");
                        let schema   = input_schema?;
                        let mut out: Vec<(String, ColumnType)> = schema.iter()
                            .filter(|(n, _)| keep.contains(n)).cloned().collect();
                        out.push((attr_col.to_string(), ColumnType::Text));
                        out.push((val_col.to_string(), ColumnType::Text));
                        Some(out)
                    }

                    // ── PrefixColumns ────────────────────────────────────────
                    "Table.PrefixColumns" => {
                        let prefix = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                        input_schema.map(|s| s.into_iter()
                            .map(|(n, t)| (format!("{}.{}", prefix, n), t)).collect())
                    }

                    // ── SelectColumns ────────────────────────────────────────
                    "Table.SelectColumns" => {
                        let columns = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let schema  = input_schema?;
                        Some(columns.iter()
                            .filter_map(|c| schema.iter().find(|(n, _)| n == c).cloned())
                            .collect())
                    }

                    // ── ReorderColumns ───────────────────────────────────────
                    "Table.ReorderColumns" => {
                        let columns = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let schema  = input_schema?;
                        let mut out: Vec<(String, ColumnType)> = columns.iter()
                            .filter_map(|c| schema.iter().find(|(n, _)| n == c).cloned())
                            .collect();
                        for item in &schema { if !columns.contains(&item.0) { out.push(item.clone()); } }
                        Some(out)
                    }

                    // ── CombineColumns: merge listed cols into one ───────────
                    "Table.CombineColumns" => {
                        let columns = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let new_col = args.get(3).and_then(|a| a.as_str()).unwrap_or("Combined");
                        input_schema.map(|s| {
                            let mut out: Vec<(String, ColumnType)> = s.into_iter()
                                .filter(|(n, _)| !columns.contains(n)).collect();
                            out.push((new_col.to_string(), ColumnType::Text));
                            out
                        })
                    }

                    // ── SplitColumn: one col → two cols ─────────────────────
                    "Table.SplitColumn" => {
                        let col_name = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                        input_schema.map(|s| {
                            let mut out: Vec<(String, ColumnType)> = s.into_iter()
                                .filter(|(n, _)| n != col_name).collect();
                            out.push((format!("{}.1", col_name), ColumnType::Text));
                            out.push((format!("{}.2", col_name), ColumnType::Text));
                            out
                        })
                    }

                    // ── ExpandTableColumn / ExpandRecordColumn ────────────────
                    "Table.ExpandTableColumn" | "Table.ExpandRecordColumn" => {
                        let col_name = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                        let columns  = args.get(2).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        input_schema.map(|s| {
                            let mut out: Vec<(String, ColumnType)> = s.into_iter()
                                .filter(|(n, _)| n != col_name).collect();
                            out.extend(columns.iter().map(|c| (c.clone(), ColumnType::Text)));
                            out
                        })
                    }

                    // ── Join: merge left + right (non-overlapping cols) ──────
                    "Table.Join" | "Table.FuzzyJoin" => {
                        let right_name = args.get(2).and_then(|a| a.as_step_ref()).unwrap_or("");
                        let left  = input_schema.unwrap_or_default();
                        let right = self.step_schemas.get(right_name).cloned().unwrap_or_default();
                        let mut out = left.clone();
                        for item in right { if !left.iter().any(|(n, _)| n == &item.0) { out.push(item); } }
                        Some(out)
                    }

                    // ── NestedJoin: add nested table column ──────────────────
                    "Table.NestedJoin" | "Table.FuzzyNestedJoin" => {
                        let new_col = args.get(4).and_then(|a| a.as_str()).unwrap_or("NewColumn");
                        input_schema.map(|mut s| { s.push((new_col.to_string(), ColumnType::Text)); s })
                    }

                    // ── AddRankColumn: add integer rank column ───────────────
                    "Table.AddRankColumn" => {
                        let col_name = args.get(1).and_then(|a| a.as_str()).unwrap_or("Rank");
                        input_schema.map(|mut s| { s.push((col_name.to_string(), ColumnType::Integer)); s })
                    }

                    // ── Combine: schema from first input ─────────────────────
                    "Table.Combine" => {
                        if let Some(inputs) = args.first().and_then(|a| a.as_step_ref_list()) {
                            inputs.first().and_then(|s| self.step_schemas.get(s.as_str()).cloned())
                        } else { None }
                    }

                    // ── Schema table ─────────────────────────────────────────
                    "Table.Schema" => Some(vec![
                        ("Name".to_string(),       ColumnType::Text),
                        ("Kind".to_string(),       ColumnType::Text),
                        ("IsNullable".to_string(), ColumnType::Boolean),
                    ]),

                    // ── No meaningful schema ─────────────────────────────────
                    "Table.RowCount" | "Table.ColumnCount" | "Table.IsEmpty" | "Table.IsDistinct"
                    | "Table.HasColumns" | "Table.MatchesAllRows" | "Table.MatchesAnyRows"
                    | "Table.ColumnNames" | "Table.ColumnsOfType"
                    | "List.Generate" | "List.Select" | "List.Transform"
                    => Some(vec![("Value".to_string(), ColumnType::Text)]),

                    // ── Structural transforms without simple schema ──────────
                    "Table.Transpose" | "Table.Pivot" | "Table.TransformColumnNames"
                    | "Table.FromColumns" | "Table.FromList" | "Table.FromRecords"
                    | "Table.FromRows" | "Table.FromValue"
                    | "Table.ToColumns" | "Table.ToList" | "Table.ToRecords" | "Table.ToRows"
                    | "Table.PartitionValues" | "Table.Profile" | "Table.Column" => None,

                    // ── Default: passthrough ─────────────────────────────────
                    _ => input_schema,
                }
            }
        }
    }
