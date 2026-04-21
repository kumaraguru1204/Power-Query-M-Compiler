    // ── step → (CTE body SQL, output schema) ─────────────────────────────────

    fn emit(
        &self,
        kind:     &StepKind,
        schema:   &[(String, ColumnType)],
        order_by: &mut Vec<(String, SortOrder)>,
    ) -> (String, Vec<(String, ColumnType)>) {
        match kind {

            // Source ──────────────────────────────────────────────────────────
            StepKind::Source { path, .. } => {
                let table_name = std::path::Path::new(path.as_str())
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(path.as_str());
                let body = format!("    SELECT * FROM {}", qi(table_name));
                (body, schema.to_vec())
            }

            // NavigateSheet ───────────────────────────────────────────────────
            StepKind::NavigateSheet { input, .. } => {
                let body = format!("    SELECT * FROM {}", input);
                (body, schema.to_vec())
            }

            // ValueBinding ────────────────────────────────────────────────────
            StepKind::ValueBinding { expr } => {
                let body = match &expr.expr {
                    Expr::List(items) => {
                        let rows = items.iter()
                            .map(|item| format!("SELECT {} AS \"Value\"", emit_expr(item)))
                            .collect::<Vec<_>>()
                            .join(" UNION ALL ");
                        format!("    {}", rows)
                    }
                    _ => format!("    SELECT {} AS \"Value\"", emit_expr(expr)),
                };
                (body, vec![("Value".to_string(), ColumnType::Text)])
            }

            // FunctionCall ────────────────────────────────────────────────────
            StepKind::FunctionCall { name, args } => {
                // Primary (left) input step name
                let input = args.first().and_then(|a| a.as_step_ref()).unwrap_or("");

                match name.as_str() {
                    // ── PromoteHeaders ────────────────────────────────────────
                    "Table.PromoteHeaders" => {
                        (format!("    SELECT * FROM {}", input), schema.to_vec())
                    }

                    // ── TransformColumnTypes (ChangeTypes) ────────────────────
                    "Table.TransformColumnTypes" => {
                        let columns = args.get(1).and_then(|a| a.as_type_list()).unwrap_or(&[]);
                        let cast_exprs: Vec<String> = schema.iter().map(|(name, _)| {
                            if let Some((_, new_ty)) = columns.iter().find(|(n, _)| n == name) {
                                format!("        CAST({} AS {}) AS {}", qi(name), sql_type(new_ty), qi(name))
                            } else {
                                format!("        {}", qi(name))
                            }
                        }).collect();
                        let body = format!("    SELECT\n{}\n    FROM {}", cast_exprs.join(",\n"), input);
                        let new_schema: Vec<(String, ColumnType)> = schema.iter().map(|(name, t)| {
                            let ty = columns.iter().find(|(n, _)| n == name).map(|(_, t)| t.clone()).unwrap_or_else(|| t.clone());
                            (name.clone(), ty)
                        }).collect();
                        (body, new_schema)
                    }

                    // ── SelectRows (Filter) ───────────────────────────────────
                    "Table.SelectRows" => {
                        if let Some(cond) = args.get(1).and_then(|a| a.as_expr()) {
                            let body = format!("    SELECT *\n    FROM {}\n    WHERE {}", input, emit_expr(cond));
                            (body, schema.to_vec())
                        } else {
                            (format!("    SELECT * FROM {}", input), schema.to_vec())
                        }
                    }

                    // ── AddColumn ─────────────────────────────────────────────
                    "Table.AddColumn" => {
                        let col_name  = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                        let expr_sql  = args.get(2).and_then(|a| a.as_expr()).map(emit_expr).unwrap_or_else(|| "NULL".to_string());
                        let body = format!("    SELECT\n        *,\n        {} AS {}\n    FROM {}", expr_sql, qi(col_name), input);
                        let inferred  = args.get(2).and_then(|a| a.as_expr()).map(|e| infer_type_sql(e, schema)).unwrap_or(ColumnType::Text);
                        let mut new_schema = schema.to_vec();
                        new_schema.push((col_name.to_string(), inferred));
                        (body, new_schema)
                    }

                    // ── RemoveColumns ─────────────────────────────────────────
                    "Table.RemoveColumns" => {
                        let columns = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let kept: Vec<&(String, ColumnType)> = schema.iter().filter(|(name, _)| !columns.contains(name)).collect();
                        let select = kept.iter().map(|(name, _)| format!("        {}", qi(name))).collect::<Vec<_>>().join(",\n");
                        let body = format!("    SELECT\n{}\n    FROM {}", select, input);
                        let new_schema = kept.iter().map(|(n, t)| (n.clone(), t.clone())).collect();
                        (body, new_schema)
                    }

                    // ── RenameColumns ─────────────────────────────────────────
                    "Table.RenameColumns" => {
                        let renames = args.get(1).and_then(|a| a.as_rename_list()).unwrap_or(&[]);
                        let select = schema.iter().map(|(name, _)| {
                            if let Some((_, new)) = renames.iter().find(|(old, _)| old == name) {
                                format!("        {} AS {}", qi(name), qi(new))
                            } else {
                                format!("        {}", qi(name))
                            }
                        }).collect::<Vec<_>>().join(",\n");
                        let body = format!("    SELECT\n{}\n    FROM {}", select, input);
                        let new_schema = schema.iter().map(|(name, t)| {
                            let new_name = renames.iter().find(|(old, _)| old == name).map(|(_, new)| new.clone()).unwrap_or_else(|| name.clone());
                            (new_name, t.clone())
                        }).collect();
                        (body, new_schema)
                    }

                    // ── Sort ──────────────────────────────────────────────────
                    "Table.Sort" => {
                        let sort_list = args.get(1).and_then(|a| a.as_sort_list()).unwrap_or(&[]);
                        *order_by = sort_list.to_vec();
                        (format!("    SELECT * FROM {}", input), schema.to_vec())
                    }

                    // ── TransformColumns ──────────────────────────────────────
                    "Table.TransformColumns" => {
                        let transforms   = args.get(1).and_then(|a| a.as_transform_list()).unwrap_or(&[]);
                        let default_expr = args.get(2).and_then(|a| a.as_expr());
                        let transform_cols: std::collections::HashSet<&str> = transforms.iter().map(|(n, _, _)| n.as_str()).collect();
                        let select = schema.iter().map(|(name, _)| {
                            if let Some((_, expr, _)) = transforms.iter().find(|(n, _, _)| n == name) {
                                format!("        {} AS {}", emit_expr(expr), qi(name))
                            } else if let Some(def_expr) = default_expr {
                                if !transform_cols.contains(name.as_str()) {
                                    format!("        {} AS {}", emit_expr(def_expr), qi(name))
                                } else { format!("        {}", qi(name)) }
                            } else { format!("        {}", qi(name)) }
                        }).collect::<Vec<_>>().join(",\n");
                        let body = format!("    SELECT\n{}\n    FROM {}", select, input);
                        (body, schema.to_vec())
                    }

                    // ── Group ─────────────────────────────────────────────────
                    "Table.Group" | "Table.FuzzyGroup" => {
                        let by  = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let agg = args.get(2).and_then(|a| a.as_agg_list()).unwrap_or(&[]);
                        let group_cols = by.iter().map(|c| qi(c)).collect::<Vec<_>>().join(", ");
                        let mut select_parts: Vec<String> = by.iter().map(|c| format!("        {}", qi(c))).collect();
                        for a in agg {
                            let inner_sql = match &a.expression.expr {
                                Expr::Lambda { body, .. } => emit_expr(body),
                                _ => emit_expr(&a.expression),
                            };
                            select_parts.push(format!("        {} AS {}", inner_sql, qi(&a.name)));
                        }
                        let body = format!("    SELECT\n{}\n    FROM {}\n    GROUP BY {}", select_parts.join(",\n"), input, group_cols);
                        let new_schema: Vec<(String, ColumnType)> = by.iter()
                            .map(|c| {
                                let t = schema.iter().find(|(n, _)| n == c).map(|(_, t)| t.clone()).unwrap_or(ColumnType::Text);
                                (c.clone(), t)
                            })
                            .chain(agg.iter().map(|a| (a.name.clone(), a.col_type.clone())))
                            .collect();
                        (body, new_schema)
                    }

                    // ── FirstN ────────────────────────────────────────────────
                    "Table.FirstN" => {
                        let n = args.get(1).and_then(|a| a.as_int()).unwrap_or(0);
                        (format!("    SELECT * FROM {} LIMIT {}", input, n), schema.to_vec())
                    }

                    // ── LastN ─────────────────────────────────────────────────
                    "Table.LastN" => {
                        let n = args.get(1).and_then(|a| a.as_int()).unwrap_or(0);
                        let body = format!("    SELECT * FROM (SELECT *, ROW_NUMBER() OVER() AS _rn FROM {0}) AS _t WHERE _rn > (SELECT COUNT(*) FROM {0}) - {1}", input, n);
                        (body, schema.to_vec())
                    }

                    // ── Skip / RemoveFirstN ───────────────────────────────────
                    "Table.Skip" | "Table.RemoveFirstN" => {
                        let n = args.get(1).and_then(|a| a.as_int()).unwrap_or(0);
                        (format!("    SELECT * FROM {} OFFSET {}", input, n), schema.to_vec())
                    }

                    // ── Range ─────────────────────────────────────────────────
                    "Table.Range" => {
                        let off = args.get(1).and_then(|a| a.as_int()).unwrap_or(0);
                        let cnt = args.get(2).and_then(|a| a.as_int()).unwrap_or(0);
                        (format!("    SELECT * FROM {} LIMIT {} OFFSET {}", input, cnt, off), schema.to_vec())
                    }

                    // ── RemoveLastN ───────────────────────────────────────────
                    "Table.RemoveLastN" => {
                        let n = args.get(1).and_then(|a| a.as_int()).unwrap_or(0);
                        let body = format!("    SELECT * FROM (SELECT *, ROW_NUMBER() OVER() AS _rn FROM {0}) AS _t WHERE _rn <= (SELECT COUNT(*) FROM {0}) - {1}", input, n);
                        (body, schema.to_vec())
                    }

                    // ── RemoveRows ────────────────────────────────────────────
                    "Table.RemoveRows" => {
                        let off = args.get(1).and_then(|a| a.as_int()).unwrap_or(0);
                        let cnt = args.get(2).and_then(|a| a.as_int()).unwrap_or(0);
                        let body = format!("    SELECT * FROM (SELECT *, ROW_NUMBER() OVER() - 1 AS _rn FROM {}) AS _t WHERE _rn < {} OR _rn >= {} + {}", input, off, off, cnt);
                        (body, schema.to_vec())
                    }

                    // ── ReverseRows ───────────────────────────────────────────
                    "Table.ReverseRows" => {
                        let body = format!("    SELECT * FROM (SELECT *, ROW_NUMBER() OVER() AS _rn FROM {}) AS _t ORDER BY _rn DESC", input);
                        (body, schema.to_vec())
                    }

                    // ── Distinct ──────────────────────────────────────────────
                    "Table.Distinct" => {
                        let columns = args.get(1).and_then(|a| a.as_col_list());
                        let body = if let Some(cols) = columns {
                            if cols.is_empty() {
                                format!("    SELECT DISTINCT * FROM {}", input)
                            } else {
                                let col_list = cols.iter().map(|c| qi(c)).collect::<Vec<_>>().join(", ");
                                format!("    SELECT DISTINCT ON ({}) * FROM {}", col_list, input)
                            }
                        } else {
                            format!("    SELECT DISTINCT * FROM {}", input)
                        };
                        (body, schema.to_vec())
                    }

                    // ── Repeat ────────────────────────────────────────────────
                    "Table.Repeat" => {
                        let n = args.get(1).and_then(|a| a.as_int()).unwrap_or(1);
                        let body = format!("    SELECT * FROM {} CROSS JOIN generate_series(1, {}) AS _g", input, n);
                        (body, schema.to_vec())
                    }

                    // ── AlternateRows ─────────────────────────────────────────
                    "Table.AlternateRows" => {
                        let off = args.get(1).and_then(|a| a.as_int()).unwrap_or(0);
                        let sk  = args.get(2).and_then(|a| a.as_int()).unwrap_or(1);
                        let tk  = args.get(3).and_then(|a| a.as_int()).unwrap_or(1);
                        let body = format!("    SELECT * FROM (SELECT *, ROW_NUMBER() OVER() - 1 AS _rn FROM {}) AS _t WHERE _rn >= {} AND (_rn - {}) % ({} + {}) < {}", input, off, off, sk, tk, tk);
                        (body, schema.to_vec())
                    }

                    // ── FindText ──────────────────────────────────────────────
                    "Table.FindText" => {
                        let text = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                        let conditions: Vec<String> = schema.iter()
                            .map(|(name, _)| format!("LOWER(CAST({} AS TEXT)) LIKE '%{}%'", qi(name), text.to_lowercase().replace('\'', "''")))
                            .collect();
                        let where_clause = if conditions.is_empty() { "TRUE".to_string() } else { conditions.join(" OR ") };
                        (format!("    SELECT * FROM {} WHERE {}", input, where_clause), schema.to_vec())
                    }

                    // ── FillDown / FillUp ─────────────────────────────────────
                    "Table.FillDown" | "Table.FillUp" => {
                        let columns = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let is_down = name == "Table.FillDown";
                        let lag_fn = if is_down { "LAG" } else { "LEAD" };
                        let select = schema.iter().map(|(col, _)| {
                            if columns.contains(col) {
                                format!("        COALESCE({0}, {1}({0} IGNORE NULLS) OVER(ORDER BY _rn)) AS {0}", qi(col), lag_fn)
                            } else { format!("        {}", qi(col)) }
                        }).collect::<Vec<_>>().join(",\n");
                        let body = format!("    SELECT\n{}\n    FROM (SELECT *, ROW_NUMBER() OVER() AS _rn FROM {}) AS _t", select, input);
                        (body, schema.to_vec())
                    }

                    // ── AddIndexColumn ────────────────────────────────────────
                    "Table.AddIndexColumn" => {
                        let col_name = args.get(1).and_then(|a| a.as_str()).unwrap_or("Index");
                        let start    = args.get(2).and_then(|a| a.as_int()).unwrap_or(0);
                        let step     = args.get(3).and_then(|a| a.as_int()).unwrap_or(1);
                        let body = format!("    SELECT *, (ROW_NUMBER() OVER() - 1) * {} + {} AS {}\n    FROM {}", step, start, qi(col_name), input);
                        let mut new_schema = schema.to_vec();
                        new_schema.push((col_name.to_string(), ColumnType::Integer));
                        (body, new_schema)
                    }

                    // ── DuplicateColumn ───────────────────────────────────────
                    "Table.DuplicateColumn" => {
                        let src_col = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                        let new_col = args.get(2).and_then(|a| a.as_str()).unwrap_or("");
                        let body = format!("    SELECT *, {} AS {}\n    FROM {}", qi(src_col), qi(new_col), input);
                        let src_type = schema.iter().find(|(n, _)| n == src_col).map(|(_, t)| t.clone()).unwrap_or(ColumnType::Text);
                        let mut new_schema = schema.to_vec();
                        new_schema.push((new_col.to_string(), src_type));
                        (body, new_schema)
                    }

                    // ── Unpivot ───────────────────────────────────────────────
                    "Table.Unpivot" => {
                        let columns  = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let attr_col = args.get(2).and_then(|a| a.as_str()).unwrap_or("Attribute");
                        let val_col  = args.get(3).and_then(|a| a.as_str()).unwrap_or("Value");
                        let keep_cols: Vec<&str> = schema.iter().filter(|(n, _)| !columns.contains(n)).map(|(n, _)| n.as_str()).collect();
                        let unions: Vec<String> = columns.iter().map(|col| {
                            let kc = keep_cols.iter().map(|c| qi(c)).collect::<Vec<_>>().join(", ");
                            format!("SELECT {}, '{}' AS {}, {} AS {} FROM {}", kc, col, qi(attr_col), qi(col), qi(val_col), input)
                        }).collect();
                        let body = format!("    {}", unions.join("\n    UNION ALL\n    "));
                        let mut new_schema: Vec<(String, ColumnType)> = schema.iter().filter(|(n, _)| !columns.contains(n)).cloned().collect();
                        new_schema.push((attr_col.to_string(), ColumnType::Text));
                        new_schema.push((val_col.to_string(), ColumnType::Text));
                        (body, new_schema)
                    }

                    // ── UnpivotOtherColumns ───────────────────────────────────
                    "Table.UnpivotOtherColumns" => {
                        let keep_cols = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let attr_col  = args.get(2).and_then(|a| a.as_str()).unwrap_or("Attribute");
                        let val_col   = args.get(3).and_then(|a| a.as_str()).unwrap_or("Value");
                        let unpivot_cols: Vec<&str> = schema.iter().filter(|(n, _)| !keep_cols.contains(n)).map(|(n, _)| n.as_str()).collect();
                        let unions: Vec<String> = unpivot_cols.iter().map(|col| {
                            let kc = keep_cols.iter().map(|c| qi(c)).collect::<Vec<_>>().join(", ");
                            format!("SELECT {}, '{}' AS {}, {} AS {} FROM {}", kc, col, qi(attr_col), qi(col), qi(val_col), input)
                        }).collect();
                        let body = format!("    {}", unions.join("\n    UNION ALL\n    "));
                        let mut new_schema: Vec<(String, ColumnType)> = schema.iter().filter(|(n, _)| keep_cols.contains(n)).cloned().collect();
                        new_schema.push((attr_col.to_string(), ColumnType::Text));
                        new_schema.push((val_col.to_string(), ColumnType::Text));
                        (body, new_schema)
                    }

                    // ── Transpose ─────────────────────────────────────────────
                    "Table.Transpose" => {
                        (format!("    SELECT * FROM {} /* TRANSPOSE */", input), vec![])
                    }

                    // ── Combine (CombineTables) ───────────────────────────────
                    "Table.Combine" => {
                        let inputs = args.first().and_then(|a| a.as_step_ref_list()).unwrap_or(&[]);
                        if inputs.is_empty() {
                            ("    SELECT NULL".to_string(), vec![])
                        } else {
                            let unions: Vec<String> = inputs.iter().map(|inp| format!("SELECT * FROM {}", inp)).collect();
                            let body = format!("    {}", unions.join("\n    UNION ALL\n    "));
                            (body, schema.to_vec())
                        }
                    }

                    // ── RemoveRowsWithErrors ──────────────────────────────────
                    "Table.RemoveRowsWithErrors" => {
                        let columns = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let conds: Vec<String> = columns.iter().map(|c| format!("{} IS NOT NULL", qi(c))).collect();
                        let where_c = if conds.is_empty() { "TRUE".to_string() } else { conds.join(" AND ") };
                        (format!("    SELECT * FROM {} WHERE {}", input, where_c), schema.to_vec())
                    }

                    // ── SelectRowsWithErrors ──────────────────────────────────
                    "Table.SelectRowsWithErrors" => {
                        let columns = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let conds: Vec<String> = columns.iter().map(|c| format!("{} IS NULL", qi(c))).collect();
                        let where_c = if conds.is_empty() { "FALSE".to_string() } else { conds.join(" OR ") };
                        (format!("    SELECT * FROM {} WHERE {}", input, where_c), schema.to_vec())
                    }

                    // ── TransformRows ─────────────────────────────────────────
                    "Table.TransformRows" => {
                        if let Some(expr) = args.get(1).and_then(|a| a.as_expr()) {
                            (format!("    SELECT {} FROM {}", emit_expr(expr), input), schema.to_vec())
                        } else {
                            (format!("    SELECT * FROM {}", input), schema.to_vec())
                        }
                    }

                    // ── MatchesAllRows ────────────────────────────────────────
                    "Table.MatchesAllRows" => {
                        if let Some(cond) = args.get(1).and_then(|a| a.as_expr()) {
                            let body = format!("    SELECT NOT EXISTS (SELECT 1 FROM {} WHERE NOT ({})) AS \"Value\"", input, emit_expr(cond));
                            (body, vec![("Value".to_string(), ColumnType::Boolean)])
                        } else {
                            ("    SELECT TRUE AS \"Value\"".to_string(), vec![("Value".to_string(), ColumnType::Boolean)])
                        }
                    }

                    // ── MatchesAnyRows ────────────────────────────────────────
                    "Table.MatchesAnyRows" => {
                        if let Some(cond) = args.get(1).and_then(|a| a.as_expr()) {
                            let body = format!("    SELECT EXISTS (SELECT 1 FROM {} WHERE {}) AS \"Value\"", input, emit_expr(cond));
                            (body, vec![("Value".to_string(), ColumnType::Boolean)])
                        } else {
                            ("    SELECT FALSE AS \"Value\"".to_string(), vec![("Value".to_string(), ColumnType::Boolean)])
                        }
                    }

                    // ── PrefixColumns ─────────────────────────────────────────
                    "Table.PrefixColumns" => {
                        let prefix = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                        let select = schema.iter()
                            .map(|(name, _)| format!("        {} AS {}", qi(name), qi(&format!("{}.{}", prefix, name))))
                            .collect::<Vec<_>>().join(",\n");
                        let body = format!("    SELECT\n{}\n    FROM {}", select, input);
                        let new_schema: Vec<(String, ColumnType)> = schema.iter().map(|(n, t)| (format!("{}.{}", prefix, n), t.clone())).collect();
                        (body, new_schema)
                    }

                    // ── DemoteHeaders ─────────────────────────────────────────
                    "Table.DemoteHeaders" => {
                        let header_select = schema.iter().enumerate()
                            .map(|(i, (name, _))| format!("'{}' AS {}", name, qi(&format!("Column{}", i + 1))))
                            .collect::<Vec<_>>().join(", ");
                        let data_select = schema.iter().enumerate()
                            .map(|(i, (name, _))| format!("CAST({} AS TEXT) AS {}", qi(name), qi(&format!("Column{}", i + 1))))
                            .collect::<Vec<_>>().join(", ");
                        let body = format!("    SELECT {} UNION ALL SELECT {} FROM {}", header_select, data_select, input);
                        let new_schema: Vec<(String, ColumnType)> = schema.iter().enumerate().map(|(i, _)| (format!("Column{}", i + 1), ColumnType::Text)).collect();
                        (body, new_schema)
                    }

                    // ── SelectColumns ─────────────────────────────────────────
                    "Table.SelectColumns" => {
                        use pq_ast::MissingFieldKind;
                        let columns = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let mf = args.get(2).and_then(|a| a.as_opt_missing_field()).flatten();
                        let mut select_parts: Vec<String> = Vec::new();
                        let mut new_schema: Vec<(String, ColumnType)> = Vec::new();
                        for col in columns {
                            if let Some(s) = schema.iter().find(|(n, _)| n == col) {
                                select_parts.push(format!("        {}", qi(col)));
                                new_schema.push(s.clone());
                            } else {
                                match mf {
                                    Some(MissingFieldKind::UseNull) => {
                                        select_parts.push(format!("        NULL AS {}", qi(col)));
                                        new_schema.push((col.clone(), ColumnType::Text));
                                    }
                                    Some(MissingFieldKind::Ignore) => {}
                                    _ => {}
                                }
                            }
                        }
                        let body = format!("    SELECT\n{}\n    FROM {}", select_parts.join(",\n"), input);
                        (body, new_schema)
                    }

                    // ── ReorderColumns ────────────────────────────────────────
                    "Table.ReorderColumns" => {
                        let columns = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let mut ordered: Vec<&str> = columns.iter().map(|s| s.as_str()).collect();
                        for (n, _) in schema { if !ordered.contains(&n.as_str()) { ordered.push(n.as_str()); } }
                        let select = ordered.iter().map(|c| format!("        {}", qi(c))).collect::<Vec<_>>().join(",\n");
                        let body = format!("    SELECT\n{}\n    FROM {}", select, input);
                        let new_schema: Vec<(String, ColumnType)> = ordered.iter().filter_map(|c| schema.iter().find(|(n, _)| n == *c).cloned()).collect();
                        (body, new_schema)
                    }

                    // ── TransformColumnNames ──────────────────────────────────
                    "Table.TransformColumnNames" => {
                        if let Some(transform) = args.get(1).and_then(|a| a.as_expr()) {
                            let select = schema.iter()
                                .map(|(name, _)| format!("        {} AS {}", qi(name), emit_expr(transform)))
                                .collect::<Vec<_>>().join(",\n");
                            let body = format!("    SELECT\n{}\n    FROM {}", select, input);
                            (body, vec![])
                        } else {
                            (format!("    SELECT * FROM {}", input), schema.to_vec())
                        }
                    }

                    // ── CombineColumns ────────────────────────────────────────
                    "Table.CombineColumns" => {
                        let columns = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let new_col = args.get(3).and_then(|a| a.as_str()).unwrap_or("Combined");
                        let concat_expr = columns.iter().map(|c| qi(c)).collect::<Vec<_>>().join(" || ");
                        let kept = schema.iter().filter(|(n, _)| !columns.contains(n)).map(|(n, _)| format!("        {}", qi(n))).collect::<Vec<_>>();
                        let mut select_parts = kept;
                        select_parts.push(format!("        {} AS {}", concat_expr, qi(new_col)));
                        let body = format!("    SELECT\n{}\n    FROM {}", select_parts.join(",\n"), input);
                        let mut new_schema: Vec<(String, ColumnType)> = schema.iter().filter(|(n, _)| !columns.contains(n)).cloned().collect();
                        new_schema.push((new_col.to_string(), ColumnType::Text));
                        (body, new_schema)
                    }

                    // ── SplitColumn ───────────────────────────────────────────
                    "Table.SplitColumn" => {
                        let col_name = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                        let body = format!("    SELECT * FROM {} /* SPLIT {} */", input, qi(col_name));
                        let new_schema: Vec<(String, ColumnType)> = schema.iter().filter(|(n, _)| n != col_name).cloned().collect();
                        (body, new_schema)
                    }

                    // ── ExpandTableColumn / ExpandRecordColumn ────────────────
                    "Table.ExpandTableColumn" | "Table.ExpandRecordColumn" => {
                        let col_name = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                        let columns  = args.get(2).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let kept: Vec<String> = schema.iter().filter(|(n, _)| n != col_name).map(|(n, _)| format!("        {}", qi(n))).collect();
                        let expanded: Vec<String> = columns.iter().map(|c| format!("        {}.{} AS {}", qi(col_name), qi(c), qi(c))).collect();
                        let mut all = kept; all.extend(expanded);
                        let body = format!("    SELECT\n{}\n    FROM {}", all.join(",\n"), input);
                        let mut new_schema: Vec<(String, ColumnType)> = schema.iter().filter(|(n, _)| n != col_name).cloned().collect();
                        for c in columns { new_schema.push((c.clone(), ColumnType::Text)); }
                        (body, new_schema)
                    }

                    // ── Pivot ─────────────────────────────────────────────────
                    "Table.Pivot" => {
                        (format!("    SELECT * FROM {} /* PIVOT */", input), vec![])
                    }

                    // ── RowCount ──────────────────────────────────────────────
                    "Table.RowCount" => {
                        let body = format!("    SELECT COUNT(*) AS \"Value\" FROM {}", input);
                        (body, vec![("Value".to_string(), ColumnType::Integer)])
                    }

                    // ── ColumnCount ───────────────────────────────────────────
                    "Table.ColumnCount" => {
                        let n = schema.len();
                        let body = format!("    SELECT {} AS \"Value\" FROM {}", n, input);
                        (body, vec![("Value".to_string(), ColumnType::Integer)])
                    }

                    // ── ColumnNames ───────────────────────────────────────────
                    "Table.ColumnNames" => {
                        let unions: Vec<String> = schema.iter().map(|(n, _)| format!("SELECT '{}' AS \"Value\"", n)).collect();
                        let body = if unions.is_empty() {
                            format!("    SELECT NULL AS \"Value\" FROM {}", input)
                        } else { format!("    {}", unions.join(" UNION ALL ")) };
                        (body, vec![("Value".to_string(), ColumnType::Text)])
                    }

                    // ── ColumnsOfType ─────────────────────────────────────────
                    "Table.ColumnsOfType" => {
                        let type_filter = args.get(1).and_then(|a| a.as_bare_type_list()).unwrap_or(&[]);
                        let unions: Vec<String> = schema.iter()
                            .filter(|(_, t)| type_filter.iter().any(|ft| sql_type_matches(t, ft)))
                            .map(|(n, _)| format!("SELECT '{}' AS \"Value\"", n))
                            .collect();
                        let body = if unions.is_empty() {
                            format!("    SELECT NULL AS \"Value\" FROM {}", input)
                        } else { format!("    {}", unions.join(" UNION ALL ")) };
                        (body, vec![("Value".to_string(), ColumnType::Text)])
                    }

                    // ── IsEmpty ───────────────────────────────────────────────
                    "Table.IsEmpty" => {
                        let body = format!("    SELECT (COUNT(*) = 0) AS \"Value\" FROM {}", input);
                        (body, vec![("Value".to_string(), ColumnType::Boolean)])
                    }

                    // ── Schema ────────────────────────────────────────────────
                    "Table.Schema" => {
                        let unions: Vec<String> = schema.iter()
                            .map(|(n, t)| format!("SELECT '{}' AS \"Name\", '{}' AS \"Kind\", TRUE AS \"IsNullable\"", n, sql_type(t)))
                            .collect();
                        let body = if unions.is_empty() {
                            format!("    SELECT NULL AS \"Name\", NULL AS \"Kind\", NULL AS \"IsNullable\" FROM {}", input)
                        } else { format!("    {}", unions.join(" UNION ALL ")) };
                        (body, vec![
                            ("Name".to_string(), ColumnType::Text),
                            ("Kind".to_string(), ColumnType::Text),
                            ("IsNullable".to_string(), ColumnType::Boolean),
                        ])
                    }

                    // ── HasColumns ────────────────────────────────────────────
                    "Table.HasColumns" => {
                        let columns = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let has_all = columns.iter().all(|c| schema.iter().any(|(n, _)| n == c));
                        let body = format!("    SELECT {} AS \"Value\" FROM {}", has_all, input);
                        (body, vec![("Value".to_string(), ColumnType::Boolean)])
                    }

                    // ── IsDistinct ────────────────────────────────────────────
                    "Table.IsDistinct" => {
                        let body = format!("    SELECT (COUNT(*) = COUNT(DISTINCT *)) AS \"Value\" FROM {}", input);
                        (body, vec![("Value".to_string(), ColumnType::Boolean)])
                    }

                    // ── Join ──────────────────────────────────────────────────
                    "Table.Join" | "Table.FuzzyJoin" => {
                        let left_keys  = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let right      = args.get(2).and_then(|a| a.as_step_ref()).unwrap_or("");
                        let right_keys = args.get(3).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let join_kind  = args.get(4).and_then(|a| a.as_join_kind());
                        let join_type  = match join_kind {
                            Some(&pq_ast::step::JoinKind::Inner)     => "INNER JOIN",
                            Some(&pq_ast::step::JoinKind::Left)      => "LEFT JOIN",
                            Some(&pq_ast::step::JoinKind::Right)     => "RIGHT JOIN",
                            Some(&pq_ast::step::JoinKind::Full)      => "FULL OUTER JOIN",
                            Some(&pq_ast::step::JoinKind::LeftAnti)  => "LEFT JOIN",
                            Some(&pq_ast::step::JoinKind::RightAnti) => "RIGHT JOIN",
                            None => "INNER JOIN",
                        };
                        let on_clause: Vec<String> = left_keys.iter().zip(right_keys.iter())
                            .map(|(lk, rk)| format!("{}.{} = {}.{}", input, qi(lk), right, qi(rk)))
                            .collect();
                        let mut body = format!("    SELECT * FROM {} {} {} ON {}", input, join_type, right, on_clause.join(" AND "));
                        if matches!(join_kind, Some(&pq_ast::step::JoinKind::LeftAnti)) {
                            if let Some(rk) = right_keys.first() {
                                body.push_str(&format!(" WHERE {}.{} IS NULL", right, qi(rk)));
                            }
                        } else if matches!(join_kind, Some(&pq_ast::step::JoinKind::RightAnti)) {
                            if let Some(lk) = left_keys.first() {
                                body.push_str(&format!(" WHERE {}.{} IS NULL", input, qi(lk)));
                            }
                        }
                        let right_schema = self.schema_of(right);
                        let mut new_schema = schema.to_vec();
                        for (n, t) in &right_schema { if !new_schema.iter().any(|(sn, _)| sn == n) { new_schema.push((n.clone(), t.clone())); } }
                        (body, new_schema)
                    }

                    // ── NestedJoin ────────────────────────────────────────────
                    "Table.NestedJoin" | "Table.FuzzyNestedJoin" => {
                        let left_keys  = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let right      = args.get(2).and_then(|a| a.as_step_ref()).unwrap_or("");
                        let right_keys = args.get(3).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let new_col    = args.get(4).and_then(|a| a.as_str()).unwrap_or("NewColumn");
                        let join_kind  = args.get(5).and_then(|a| a.as_join_kind());
                        let join_type  = match join_kind {
                            Some(&pq_ast::step::JoinKind::Inner)  => "INNER JOIN",
                            Some(&pq_ast::step::JoinKind::Left)   => "LEFT JOIN",
                            Some(&pq_ast::step::JoinKind::Right)  => "RIGHT JOIN",
                            Some(&pq_ast::step::JoinKind::Full)   => "FULL OUTER JOIN",
                            _ => "LEFT JOIN",
                        };
                        let on_clause: Vec<String> = left_keys.iter().zip(right_keys.iter())
                            .map(|(lk, rk)| format!("{}.{} = {}.{}", input, qi(lk), right, qi(rk)))
                            .collect();
                        let left_cols = schema.iter().map(|(n, _)| format!("{}.{}", input, qi(n))).collect::<Vec<_>>().join(", ");
                        let body = format!("    SELECT {}, {} AS {}\n    FROM {} {} {} ON {}", left_cols, right, qi(new_col), input, join_type, right, on_clause.join(" AND "));
                        let mut new_schema = schema.to_vec();
                        new_schema.push((new_col.to_string(), ColumnType::Text));
                        (body, new_schema)
                    }

                    // ── AddRankColumn ─────────────────────────────────────────
                    "Table.AddRankColumn" => {
                        let col_name = args.get(1).and_then(|a| a.as_str()).unwrap_or("Rank");
                        let by       = args.get(2).and_then(|a| a.as_sort_list()).unwrap_or(&[]);
                        let order_clause = if by.is_empty() { "1".to_string() } else {
                            by.iter().map(|(c, o)| format!("{} {}", qi(c), sort_dir(o))).collect::<Vec<_>>().join(", ")
                        };
                        let body = format!("    SELECT *, RANK() OVER(ORDER BY {}) AS {}\n    FROM {}", order_clause, qi(col_name), input);
                        let mut new_schema = schema.to_vec();
                        new_schema.push((col_name.to_string(), ColumnType::Integer));
                        (body, new_schema)
                    }

                    // ── TableMax / TableMin ───────────────────────────────────
                    "Table.Max" => {
                        let col = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                        (format!("    SELECT * FROM {} ORDER BY {} DESC LIMIT 1", input, qi(col)), schema.to_vec())
                    }
                    "Table.Min" => {
                        let col = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                        (format!("    SELECT * FROM {} ORDER BY {} ASC LIMIT 1", input, qi(col)), schema.to_vec())
                    }

                    // ── TableMaxN / TableMinN ─────────────────────────────────
                    "Table.MaxN" => {
                        let n   = args.get(1).and_then(|a| a.as_int()).unwrap_or(1);
                        let col = args.get(2).and_then(|a| a.as_str()).unwrap_or("");
                        (format!("    SELECT * FROM {} ORDER BY {} DESC LIMIT {}", input, qi(col), n), schema.to_vec())
                    }
                    "Table.MinN" => {
                        let n   = args.get(1).and_then(|a| a.as_int()).unwrap_or(1);
                        let col = args.get(2).and_then(|a| a.as_str()).unwrap_or("");
                        (format!("    SELECT * FROM {} ORDER BY {} ASC LIMIT {}", input, qi(col), n), schema.to_vec())
                    }

                    // ── ReplaceValue ──────────────────────────────────────────
                    "Table.ReplaceValue" => {
                        let old_val = args.get(1).and_then(|a| a.as_expr()).map(emit_expr).unwrap_or_else(|| "NULL".to_string());
                        let new_val = args.get(2).and_then(|a| a.as_expr()).map(emit_expr).unwrap_or_else(|| "NULL".to_string());
                        let select = schema.iter().map(|(name, _)| format!(
                            "        CASE WHEN {} = {} THEN {} ELSE {} END AS {}", qi(name), old_val, new_val, qi(name), qi(name)
                        )).collect::<Vec<_>>().join(",\n");
                        (format!("    SELECT\n{}\n    FROM {}", select, input), schema.to_vec())
                    }

                    // ── ReplaceErrorValues ────────────────────────────────────
                    "Table.ReplaceErrorValues" => {
                        let replacements = args.get(1).and_then(|a| a.as_transform_list()).unwrap_or(&[]);
                        let select = schema.iter().map(|(name, _)| {
                            if let Some((_, repl, _)) = replacements.iter().find(|(n, _, _)| n == name) {
                                format!("        COALESCE({}, {}) AS {}", qi(name), emit_expr(repl), qi(name))
                            } else { format!("        {}", qi(name)) }
                        }).collect::<Vec<_>>().join(",\n");
                        (format!("    SELECT\n{}\n    FROM {}", select, input), schema.to_vec())
                    }

                    // ── InsertRows ────────────────────────────────────────────
                    "Table.InsertRows" => {
                        (format!("    SELECT * FROM {} /* INSERT ROWS */", input), schema.to_vec())
                    }

                    // ── ListGenerate ──────────────────────────────────────────
                    "List.Generate" => {
                        ("    SELECT NULL AS \"Value\"".to_string(), vec![("Value".to_string(), ColumnType::Text)])
                    }

                    // ── ListSelect ────────────────────────────────────────────
                    "List.Select" => {
                        let list_expr = args.first().and_then(|a| a.as_expr());
                        let predicate = args.get(1).and_then(|a| a.as_expr());
                        if let (Some(list_expr), Some(predicate)) = (list_expr, predicate) {
                            let pred_sql = emit_expr_with_underscore(predicate, "Value");
                            let body = match &list_expr.expr {
                                Expr::List(items) => {
                                    let rows = items.iter().map(|item| format!("SELECT {} AS \"Value\"", emit_expr(item))).collect::<Vec<_>>().join(" UNION ALL ");
                                    format!("    SELECT \"Value\" FROM ({}) t WHERE {}", rows, pred_sql)
                                }
                                _ => format!("    SELECT \"Value\" FROM {} WHERE {}", emit_expr(list_expr), pred_sql),
                            };
                            (body, vec![("Value".to_string(), ColumnType::Text)])
                        } else {
                            ("    SELECT NULL AS \"Value\"".to_string(), vec![("Value".to_string(), ColumnType::Text)])
                        }
                    }

                    // ── ListTransform ─────────────────────────────────────────
                    "List.Transform" => {
                        let list_expr = args.first().and_then(|a| a.as_expr());
                        let transform = args.get(1).and_then(|a| a.as_expr());
                        if let (Some(list_expr), Some(transform)) = (list_expr, transform) {
                            let transform_sql = emit_expr_with_underscore(transform, "Value");
                            let body = match &list_expr.expr {
                                Expr::List(items) => {
                                    let rows = items.iter()
                                        .map(|item| format!("        ({}) AS \"Value\"", emit_expr(item)))
                                        .collect::<Vec<_>>()
                                        .join(",\n        UNION ALL SELECT\n");
                                    format!("    SELECT {}", rows)
                                }
                                _ => format!("    SELECT {} AS \"Value\" FROM {}", transform_sql, emit_expr(list_expr)),
                            };
                            (body, vec![("Value".to_string(), ColumnType::Text)])
                        } else {
                            ("    SELECT NULL AS \"Value\"".to_string(), vec![("Value".to_string(), ColumnType::Text)])
                        }
                    }

                    // ── Construction stubs ────────────────────────────────────
                    "Table.FromColumns" => {
                        let columns = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let schema: Vec<(String, ColumnType)> = columns.iter().map(|c| (c.clone(), ColumnType::Text)).collect();
                        ("    SELECT NULL /* Table.FromColumns */".to_string(), schema)
                    }
                    "Table.FromList" => {
                        ("    SELECT NULL AS \"Column1\" /* Table.FromList */".to_string(), vec![("Column1".to_string(), ColumnType::Text)])
                    }
                    "Table.FromRecords" => {
                        ("    SELECT NULL /* Table.FromRecords */".to_string(), vec![])
                    }
                    "Table.FromRows" => {
                        let columns = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let schema: Vec<(String, ColumnType)> = columns.iter().map(|c| (c.clone(), ColumnType::Text)).collect();
                        ("    SELECT NULL /* Table.FromRows */".to_string(), schema)
                    }
                    "Table.FromValue" => {
                        let col_name = args.get(1).and_then(|a| a.as_str()).unwrap_or("Value");
                        (format!("    SELECT NULL AS \"{}\" /* Table.FromValue */", col_name), vec![(col_name.to_string(), ColumnType::Text)])
                    }

                    // ── Conversion passthrough ────────────────────────────────
                    "Table.ToColumns" | "Table.ToList" | "Table.ToRecords" | "Table.ToRows"
                    | "Table.PartitionValues" | "Table.Profile" => {
                        (format!("    SELECT * FROM {}", input), schema.to_vec())
                    }

                    // ── TableColumn ───────────────────────────────────────────
                    "Table.Column" => {
                        let col_name = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                        let body = format!("    SELECT \"{}\" FROM {}", col_name, input);
                        (body, vec![(col_name.to_string(), ColumnType::Text)])
                    }

                    // ── Default passthrough ───────────────────────────────────
                    _ => {
                        if input.is_empty() {
                            ("    SELECT NULL AS \"Value\"".to_string(), vec![])
                        } else {
                            (format!("    SELECT * FROM {}", input), schema.to_vec())
                        }
                    }
                }
            }
        }
    }
}
