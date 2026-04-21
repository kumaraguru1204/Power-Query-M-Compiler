    fn register_output_schema(&mut self, step_name: &str, kind: &StepKind) {
        let output: Option<Vec<String>> = match kind {
            StepKind::Source { .. } => {
                Some(self.table.column_names().iter().map(|s| s.to_string()).collect())
            }
            StepKind::NavigateSheet { input, .. } => self.schema_of(input),
            StepKind::ValueBinding { .. } => None,
            StepKind::FunctionCall { name, args } => {
                let input_name = args.first().and_then(|a| a.as_step_ref()).unwrap_or("");
                match name.as_str() {
                    "Table.AddColumn" => {
                        let col_name = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                        self.schema_of(input_name).map(|mut cols| { cols.push(col_name.to_string()); cols })
                    }
                    "Table.RemoveColumns" => {
                        let cols = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        self.schema_of(input_name).map(|s| s.into_iter().filter(|n| !cols.contains(n)).collect())
                    }
                    "Table.RenameColumns" => {
                        let renames = args.get(1).and_then(|a| a.as_rename_list()).unwrap_or(&[]);
                        self.schema_of(input_name).map(|cols| {
                            cols.into_iter().map(|n| {
                                renames.iter().find(|(old, _)| old == &n)
                                    .map(|(_, new)| new.clone()).unwrap_or(n)
                            }).collect()
                        })
                    }
                    "Table.Group" | "Table.FuzzyGroup" => {
                        let by  = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let agg = args.get(2).and_then(|a| a.as_agg_list()).unwrap_or(&[]);
                        let mut cols: Vec<String> = by.iter().cloned().collect();
                        for a in agg { cols.push(a.name.clone()); }
                        Some(cols)
                    }
                    "Table.AddIndexColumn" => {
                        let col_name = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                        self.schema_of(input_name).map(|mut cols| { cols.push(col_name.to_string()); cols })
                    }
                    "Table.DuplicateColumn" => {
                        let new_col = args.get(2).and_then(|a| a.as_str()).unwrap_or("");
                        self.schema_of(input_name).map(|mut cols| { cols.push(new_col.to_string()); cols })
                    }
                    "Table.Unpivot" => {
                        let columns  = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let attr_col = args.get(2).and_then(|a| a.as_str()).unwrap_or("Attribute");
                        let val_col  = args.get(3).and_then(|a| a.as_str()).unwrap_or("Value");
                        self.schema_of(input_name).map(|cols| {
                            let mut out: Vec<String> = cols.into_iter().filter(|n| !columns.contains(n)).collect();
                            out.push(attr_col.to_string()); out.push(val_col.to_string()); out
                        })
                    }
                    "Table.UnpivotOtherColumns" => {
                        let keep = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let attr = args.get(2).and_then(|a| a.as_str()).unwrap_or("Attribute");
                        let val  = args.get(3).and_then(|a| a.as_str()).unwrap_or("Value");
                        let mut out: Vec<String> = keep.iter().cloned().collect();
                        out.push(attr.to_string()); out.push(val.to_string()); Some(out)
                    }
                    "Table.PrefixColumns" => {
                        let prefix = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                        self.schema_of(input_name).map(|cols| cols.into_iter().map(|n| format!("{}.{}", prefix, n)).collect())
                    }
                    "Table.SelectColumns" => {
                        let columns = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        Some(columns.iter().cloned().collect())
                    }
                    "Table.ReorderColumns" => {
                        let columns = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        self.schema_of(input_name).map(|all_cols| {
                            let mut out: Vec<String> = columns.iter().filter(|c| all_cols.contains(*c)).cloned().collect();
                            for c in all_cols { if !columns.contains(&c) { out.push(c); } }
                            out
                        })
                    }
                    "Table.CombineColumns" => {
                        let columns = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let new_col = args.get(3).and_then(|a| a.as_str()).unwrap_or("Combined");
                        self.schema_of(input_name).map(|cols| {
                            let mut out: Vec<String> = cols.into_iter().filter(|n| !columns.contains(n)).collect();
                            out.push(new_col.to_string()); out
                        })
                    }
                    "Table.SplitColumn" => {
                        let col_name = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                        self.schema_of(input_name).map(|cols| {
                            let mut out: Vec<String> = cols.into_iter().filter(|n| n != col_name).collect();
                            out.push(format!("{}.1", col_name)); out.push(format!("{}.2", col_name)); out
                        })
                    }
                    "Table.ExpandTableColumn" | "Table.ExpandRecordColumn" => {
                        let col_name = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                        let columns  = args.get(2).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        self.schema_of(input_name).map(|cols| {
                            let mut out: Vec<String> = cols.into_iter().filter(|n| n != col_name).collect();
                            out.extend(columns.iter().cloned()); out
                        })
                    }
                    "Table.Join" | "Table.FuzzyJoin" => {
                        let right_name = args.get(2).and_then(|a| a.as_step_ref()).unwrap_or("");
                        let left  = self.schema_of(input_name).unwrap_or_default();
                        let right = self.schema_of(right_name).unwrap_or_default();
                        let mut out = left.clone();
                        for c in right { if !left.contains(&c) { out.push(c); } }
                        Some(out)
                    }
                    "Table.NestedJoin" | "Table.FuzzyNestedJoin" => {
                        let new_col = args.get(4).and_then(|a| a.as_str()).unwrap_or("NewColumn");
                        self.schema_of(input_name).map(|mut cols| { cols.push(new_col.to_string()); cols })
                    }
                    "Table.AddRankColumn" => {
                        let col_name = args.get(1).and_then(|a| a.as_str()).unwrap_or("Rank");
                        self.schema_of(input_name).map(|mut cols| { cols.push(col_name.to_string()); cols })
                    }
                    "Table.RowCount" | "Table.ColumnCount" | "Table.IsEmpty" | "Table.IsDistinct"
                    | "Table.HasColumns" | "Table.ColumnNames" | "Table.ColumnsOfType"
                    | "Table.MatchesAllRows" | "Table.MatchesAnyRows"
                    | "List.Generate" | "List.Select" | "List.Transform" => {
                        Some(vec!["Value".to_string()])
                    }
                    "Table.Schema" => {
                        Some(vec!["Name".to_string(), "Kind".to_string(), "IsNullable".to_string()])
                    }
                    "Table.Combine" => {
                        let inputs = args.first().and_then(|a| a.as_step_ref_list()).unwrap_or(&[]);
                        inputs.first().and_then(|s| self.schema_of(s))
                    }
                    "Table.Transpose" | "Table.Pivot" | "Table.TransformColumnNames"
                    | "Table.FromColumns" | "Table.FromList" | "Table.FromRecords"
                    | "Table.FromRows" | "Table.FromValue"
                    | "Table.ToColumns" | "Table.ToList" | "Table.ToRecords" | "Table.ToRows"
                    | "Table.PartitionValues" | "Table.Profile" | "Table.Column" => None,
                    _ => self.schema_of(input_name),
                }
            }
        };
        if let Some(cols) = output {
            self.step_schemas.insert(step_name.to_string(), cols);
        }
    }

    // ── step validation helper ────────────────────────────────────────────────

    /// Encapsulates the three-part pattern repeated in every step arm:
    /// 1. Verify `input` is a known step in `scope`.
    /// 2. Verify every name in `col_names` exists in `schema_ref`.
    /// 3. Recursively resolve every expression in `exprs`.
    ///
    /// `input` may be empty (e.g. `Passthrough` with no predecessor) -- the
    /// scope check is skipped in that case.
    fn validate_step(
        &mut self,
        input:      &str,
        col_names:  &[&str],
        exprs:      &[&ExprNode],
        step_name:  &str,
        step_span:  &Span,
        scope:      &Scope,
        schema_ref: Option<&[String]>,
    ) {
        if !input.is_empty() && !scope.contains(input) {
            self.unknown_step(input, step_span.clone(), scope, step_name);
        }
        if let Some(cols) = schema_ref {
            for col in col_names {
                if !cols.iter().any(|c| c == col) {
                    self.unknown_column(col, step_span.clone());
                }
            }
        }
        for expr in exprs {
            self.resolve_expr(expr, schema_ref);
        }
    }

    // ── step resolver ─────────────────────────────────────────────────────────

    fn resolve_step(
        &mut self,
        step_name: &str,
        step_span: &Span,
        kind:      &StepKind,
        scope:     &Scope,
    ) {
        match kind {
            StepKind::Source { .. } => {
                self.register_output_schema(step_name, kind);
            }
            StepKind::NavigateSheet { input, .. } => {
                if !scope.contains(input.as_str()) {
                    self.unknown_step(input, step_span.clone(), scope, step_name);
                }
                self.register_output_schema(step_name, kind);
            }
            StepKind::ValueBinding { expr } => {
                self.resolve_expr(expr, None);
                self.register_output_schema(step_name, kind);
            }
            StepKind::FunctionCall { name, args } => {
                // Extract primary input step reference (args[0])
                let input_name = args.first().and_then(|a| a.as_step_ref()).unwrap_or("");

                // Validate primary input step exists
                if !input_name.is_empty() && !scope.contains(input_name) {
                    self.unknown_step(input_name, step_span.clone(), scope, step_name);
                }

                // For joins, also validate the secondary table reference (args[2])
                match name.as_str() {
                    "Table.Join" | "Table.NestedJoin" | "Table.FuzzyJoin" | "Table.FuzzyNestedJoin" => {
                        if let Some(right) = args.get(2).and_then(|a| a.as_step_ref()) {
                            if !scope.contains(right) {
                                self.unknown_step(right, step_span.clone(), scope, step_name);
                            }
                        }
                    }
                    "Table.Combine" => {
                        if let Some(inputs) = args.first().and_then(|a| a.as_step_ref_list()) {
                            for inp in inputs {
                                if !scope.contains(inp.as_str()) {
                                    self.unknown_step(inp, step_span.clone(), scope, step_name);
                                }
                            }
                        }
                    }
                    _ => {}
                }

                // Get input schema for column validation
                let input_schema = if !input_name.is_empty() {
                    self.schema_of(input_name)
                } else {
                    None
                };
                let schema_ref = input_schema.as_deref();

                // Validate column list args against input schema
                for arg in args.iter() {
                    if let CallArg::ColList(cols) = arg {
                        if let Some(sch) = schema_ref {
                            for col in cols {
                                if !sch.iter().any(|c| c == col) {
                                    self.unknown_column(col, step_span.clone());
                                }
                            }
                        }
                    }
                }

                // Resolve expression args against input schema
                for arg in args.iter() {
                    match arg {
                        CallArg::Expr(expr) => {
                            self.resolve_expr(expr, schema_ref);
                        }
                        CallArg::AggList(aggs) => {
                            for agg in aggs {
                                self.resolve_expr(&agg.expression, schema_ref);
                            }
                        }
                        CallArg::TransformList(transforms) => {
                            for (_, expr, _) in transforms {
                                self.resolve_expr(expr, schema_ref);
                            }
                        }
                        _ => {}
                    }
                }

                // Register this step's output schema for downstream steps
                self.register_output_schema(step_name, kind);
            }
        }
    }
