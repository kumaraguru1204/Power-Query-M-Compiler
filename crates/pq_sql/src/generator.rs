use std::collections::HashMap;

use pq_ast::{
    Program,
    expr::{Expr, ExprNode},
    step::{StepKind, SortOrder},
};
use pq_grammar::operators::{Operator, UnaryOp};
use pq_pipeline::Table;
use pq_types::ColumnType;

// ── public entry point ────────────────────────────────────────────────────

/// Convert a typed `Program` into a SQL query string.
///
/// Every M step becomes a CTE.  The final output is:
///
/// ```sql
/// WITH
///   Source          AS (SELECT * FROM "sheet"),
///   PromotedHeaders AS (SELECT * FROM Source),
///   ...
/// SELECT * FROM <output_step>
/// [ORDER BY ...]
/// ```
pub fn generate_sql(program: &Program, table: &Table) -> String {
    Generator::new(table).run(program)
}

// ── internal ──────────────────────────────────────────────────────────────

struct Generator {
    /// schema after each step: step_name -> [(col_name, col_type)]
    schemas: HashMap<String, Vec<(String, ColumnType)>>,
    /// initial column schema from the source Table
    initial: Vec<(String, ColumnType)>,
}

impl Generator {
    fn new(table: &Table) -> Self {
        let initial = table.columns.iter()
            .map(|c| (c.name.clone(), c.col_type.clone()))
            .collect();
        Generator { schemas: HashMap::new(), initial }
    }

    fn run(&mut self, program: &Program) -> String {
        let mut ctes:     Vec<String>              = Vec::new();
        let mut order_by: Vec<(String, SortOrder)> = Vec::new();

        for binding in &program.steps {
            let in_name = step_input(&binding.step.kind);
            let schema  = self.schema_of(in_name);

            let (body, out_schema) =
                self.emit(&binding.step.kind, &schema, &mut order_by);

            ctes.push(format!("  {} AS (\n{}\n  )", binding.name, body));
            self.schemas.insert(binding.name.clone(), out_schema);
        }

        let mut sql = format!("SELECT *\nFROM {}", program.output);
        if !order_by.is_empty() {
            let ob = order_by.iter()
                .map(|(col, ord)| format!("{} {}", qi(col), sort_dir(ord)))
                .collect::<Vec<_>>()
                .join(", ");
            sql.push_str(&format!("\nORDER BY {}", ob));
        }

        if ctes.is_empty() {
            return sql;
        }
        format!("WITH\n{}\n{}", ctes.join(",\n"), sql)
    }

    fn schema_of(&self, step_name: &str) -> Vec<(String, ColumnType)> {
        if step_name.is_empty() {
            return self.initial.clone();
        }
        self.schemas
            .get(step_name)
            .cloned()
            .unwrap_or_else(|| self.initial.clone())
    }

    // ── step → (CTE body SQL, output schema) ─────────────────────────────

    fn emit(
        &self,
        kind:     &StepKind,
        schema:   &[(String, ColumnType)],
        order_by: &mut Vec<(String, SortOrder)>,
    ) -> (String, Vec<(String, ColumnType)>) {
        match kind {

            // Source ──────────────────────────────────────────────────────
            StepKind::Source { path, .. } => {
                // Derive table name from the file path (strip directory + extension)
                let table_name = std::path::Path::new(path.as_str())
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(path.as_str());
                let body = format!("    SELECT * FROM {}", qi(table_name));
                (body, schema.to_vec())
            }

            // PromoteHeaders ──────────────────────────────────────────────
            StepKind::PromoteHeaders { input } => {
                let body = format!("    SELECT * FROM {}", input);
                (body, schema.to_vec())
            }

            // ChangeTypes — CAST each listed column ───────────────────────
            StepKind::ChangeTypes { input, columns } => {
                let select = schema.iter()
                    .map(|(name, _)| {
                        if let Some((_, new_type)) = columns.iter().find(|(n, _)| n == name) {
                            format!("        CAST({} AS {}) AS {}", qi(name), sql_type(new_type), qi(name))
                        } else {
                            format!("        {}", qi(name))
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(",\n");

                let body = format!("    SELECT\n{}\n    FROM {}", select, input);
                let new_schema = schema.iter()
                    .map(|(name, t)| {
                        let nt = columns.iter()
                            .find(|(n, _)| n == name)
                            .map(|(_, nt)| nt.clone())
                            .unwrap_or_else(|| t.clone());
                        (name.clone(), nt)
                    })
                    .collect();
                (body, new_schema)
            }

            // Filter — WHERE clause ───────────────────────────────────────
            // condition is Each(inner) — emit_expr unwraps the Each wrapper
            StepKind::Filter { input, condition } => {
                let body = format!(
                    "    SELECT *\n    FROM {}\n    WHERE {}",
                    input,
                    emit_expr(condition)
                );
                (body, schema.to_vec())
            }

            // AddColumn — SELECT *, expr AS col ───────────────────────────
            // expression is Each(inner) — emit_expr unwraps the Each wrapper
            StepKind::AddColumn { input, col_name, expression } => {
                let expr_sql = emit_expr(expression);
                let body = format!(
                    "    SELECT\n        *,\n        {} AS {}\n    FROM {}",
                    expr_sql, qi(col_name), input
                );
                let inferred   = infer_type_sql(expression, schema);
                let mut new_schema = schema.to_vec();
                new_schema.push((col_name.clone(), inferred));
                (body, new_schema)
            }

            // RemoveColumns — enumerate surviving columns ──────────────────
            StepKind::RemoveColumns { input, columns } => {
                let kept: Vec<&(String, ColumnType)> = schema.iter()
                    .filter(|(name, _)| !columns.contains(name))
                    .collect();
                let select = kept.iter()
                    .map(|(name, _)| format!("        {}", qi(name)))
                    .collect::<Vec<_>>()
                    .join(",\n");
                let body       = format!("    SELECT\n{}\n    FROM {}", select, input);
                let new_schema = kept.iter().map(|(n, t)| (n.clone(), t.clone())).collect();
                (body, new_schema)
            }

            // RenameColumns — col AS "new_name" ───────────────────────────
            StepKind::RenameColumns { input, renames } => {
                let select = schema.iter()
                    .map(|(name, _)| {
                        if let Some((_, new)) = renames.iter().find(|(old, _)| old == name) {
                            format!("        {} AS {}", qi(name), qi(new))
                        } else {
                            format!("        {}", qi(name))
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(",\n");
                let body       = format!("    SELECT\n{}\n    FROM {}", select, input);
                let new_schema = schema.iter()
                    .map(|(name, t)| {
                        let new_name = renames.iter()
                            .find(|(old, _)| old == name)
                            .map(|(_, new)| new.clone())
                            .unwrap_or_else(|| name.clone());
                        (new_name, t.clone())
                    })
                    .collect();
                (body, new_schema)
            }

            // Sort — ORDER BY collected; CTE is a passthrough ─────────────
            StepKind::Sort { input, by } => {
                *order_by = by.clone();
                let body   = format!("    SELECT * FROM {}", input);
                (body, schema.to_vec())
            }

            // TransformColumns — SELECT with expression for each listed col ─
            // Each transform is Each(inner) — emit_expr handles it.
            StepKind::TransformColumns { input, transforms } => {
                let select = schema.iter()
                    .map(|(name, _)| {
                        if let Some((_, expr, _)) = transforms.iter().find(|(n, _, _)| n == name) {
                            format!("        {} AS {}", emit_expr(expr), qi(name))
                        } else {
                            format!("        {}", qi(name))
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(",\n");
                let body = format!("    SELECT\n{}\n    FROM {}", select, input);
                (body, schema.to_vec())
            }

            // Group — GROUP BY with aggregate expressions ──────────────────
            StepKind::Group { input, by, aggregates } => {
                let group_cols = by.iter().map(|c| qi(c)).collect::<Vec<_>>().join(", ");

                let mut select_parts: Vec<String> = by.iter()
                    .map(|c| format!("        {}", qi(c)))
                    .collect();

                for agg in aggregates {
                    // Unwrap the Lambda wrapper (param == "_") for the aggregate body.
                    let inner_sql = match &agg.expression.expr {
                        Expr::Lambda { body, .. } => emit_expr(body),
                        _ => emit_expr(&agg.expression),
                    };
                    select_parts.push(format!("        {} AS {}", inner_sql, qi(&agg.name)));
                }

                let body = format!(
                    "    SELECT\n{}\n    FROM {}\n    GROUP BY {}",
                    select_parts.join(",\n"),
                    input,
                    group_cols
                );

                let new_schema: Vec<(String, ColumnType)> = by.iter()
                    .map(|c| {
                        let t = schema.iter()
                            .find(|(n, _)| n == c)
                            .map(|(_, t)| t.clone())
                            .unwrap_or(ColumnType::Text);
                        (c.clone(), t)
                    })
                    .chain(aggregates.iter().map(|a| (a.name.clone(), a.col_type.clone())))
                    .collect();

                (body, new_schema)
            }

            // Passthrough ─────────────────────────────────────────────────
            StepKind::Passthrough { input, .. } => {
                if input.is_empty() {
                    ("    SELECT NULL AS \"Value\"".to_string(), vec![])
                } else {
                    let body = format!("    SELECT * FROM {}", input);
                    (body, schema.to_vec())
                }
            }

            // ── FirstN ───────────────────────────────────────────────────
            StepKind::FirstN { input, count } => {
                let n = emit_expr(count);
                let body = format!("    SELECT * FROM {} LIMIT {}", input, n);
                (body, schema.to_vec())
            }

            // ── LastN ────────────────────────────────────────────────────
            StepKind::LastN { input, count } => {
                let n = emit_expr(count);
                let body = format!(
                    "    SELECT * FROM (SELECT *, ROW_NUMBER() OVER() AS _rn FROM {0}) AS _t \
                     WHERE _rn > (SELECT COUNT(*) FROM {0}) - {1}",
                    input, n
                );
                (body, schema.to_vec())
            }

            // ── Skip ─────────────────────────────────────────────────────
            StepKind::Skip { input, count }
            | StepKind::RemoveFirstN { input, count } => {
                let n = emit_expr(count);
                let body = format!("    SELECT * FROM {} OFFSET {}", input, n);
                (body, schema.to_vec())
            }

            // ── Range ────────────────────────────────────────────────────
            StepKind::Range { input, offset, count } => {
                let off = emit_expr(offset);
                let cnt = emit_expr(count);
                let body = format!("    SELECT * FROM {} LIMIT {} OFFSET {}", input, cnt, off);
                (body, schema.to_vec())
            }

            // ── RemoveLastN ──────────────────────────────────────────────
            StepKind::RemoveLastN { input, count } => {
                let n = emit_expr(count);
                let body = format!(
                    "    SELECT * FROM (SELECT *, ROW_NUMBER() OVER() AS _rn FROM {0}) AS _t \
                     WHERE _rn <= (SELECT COUNT(*) FROM {0}) - {1}",
                    input, n
                );
                (body, schema.to_vec())
            }

            // ── RemoveRows ───────────────────────────────────────────────
            StepKind::RemoveRows { input, offset, count } => {
                let off = emit_expr(offset);
                let cnt = emit_expr(count);
                let body = format!(
                    "    SELECT * FROM (SELECT *, ROW_NUMBER() OVER() - 1 AS _rn FROM {}) AS _t \
                     WHERE _rn < {} OR _rn >= {} + {}",
                    input, off, off, cnt
                );
                (body, schema.to_vec())
            }

            // ── ReverseRows ──────────────────────────────────────────────
            StepKind::ReverseRows { input } => {
                let body = format!(
                    "    SELECT * FROM (SELECT *, ROW_NUMBER() OVER() AS _rn FROM {}) AS _t ORDER BY _rn DESC",
                    input
                );
                (body, schema.to_vec())
            }

            // ── Distinct ─────────────────────────────────────────────────
            StepKind::Distinct { input, columns } => {
                let body = if columns.is_empty() {
                    format!("    SELECT DISTINCT * FROM {}", input)
                } else {
                    let cols = columns.iter().map(|c| qi(c)).collect::<Vec<_>>().join(", ");
                    format!("    SELECT DISTINCT ON ({}) * FROM {}", cols, input)
                };
                (body, schema.to_vec())
            }

            // ── Repeat ───────────────────────────────────────────────────
            StepKind::Repeat { input, count } => {
                let n = emit_expr(count);
                let body = format!(
                    "    SELECT * FROM {} CROSS JOIN generate_series(1, {}) AS _g",
                    input, n
                );
                (body, schema.to_vec())
            }

            // ── AlternateRows ────────────────────────────────────────────
            StepKind::AlternateRows { input, offset, skip, take } => {
                let off = emit_expr(offset);
                let sk  = emit_expr(skip);
                let tk  = emit_expr(take);
                let body = format!(
                    "    SELECT * FROM (SELECT *, ROW_NUMBER() OVER() - 1 AS _rn FROM {}) AS _t \
                     WHERE _rn >= {} AND (_rn - {}) % ({} + {}) < {}",
                    input, off, off, sk, tk, tk
                );
                (body, schema.to_vec())
            }

            // ── FindText ─────────────────────────────────────────────────
            StepKind::FindText { input, text } => {
                let conditions: Vec<String> = schema.iter()
                    .map(|(name, _)| format!("LOWER(CAST({} AS TEXT)) LIKE '%{}%'", qi(name), text.to_lowercase().replace('\'', "''")))
                    .collect();
                let where_clause = if conditions.is_empty() { "TRUE".to_string() } else { conditions.join(" OR ") };
                let body = format!("    SELECT * FROM {} WHERE {}", input, where_clause);
                (body, schema.to_vec())
            }

            // ── FillDown / FillUp ────────────────────────────────────────
            StepKind::FillDown { input, columns } | StepKind::FillUp { input, columns } => {
                let is_down = matches!(kind, StepKind::FillDown { .. });
                let lag_fn = if is_down { "LAG" } else { "LEAD" };
                let select = schema.iter()
                    .map(|(name, _)| {
                        if columns.contains(name) {
                            format!("        COALESCE({0}, {1}({0} IGNORE NULLS) OVER(ORDER BY _rn)) AS {0}",
                                qi(name), lag_fn)
                        } else {
                            format!("        {}", qi(name))
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(",\n");
                let body = format!(
                    "    SELECT\n{}\n    FROM (SELECT *, ROW_NUMBER() OVER() AS _rn FROM {}) AS _t",
                    select, input
                );
                (body, schema.to_vec())
            }

            // ── AddIndexColumn ───────────────────────────────────────────
            StepKind::AddIndexColumn { input, col_name, start, step } => {
                let body = format!(
                    "    SELECT *, (ROW_NUMBER() OVER() - 1) * {} + {} AS {}\n    FROM {}",
                    step, start, qi(col_name), input
                );
                let mut new_schema = schema.to_vec();
                new_schema.push((col_name.clone(), ColumnType::Integer));
                (body, new_schema)
            }

            // ── DuplicateColumn ──────────────────────────────────────────
            StepKind::DuplicateColumn { input, src_col, new_col } => {
                let body = format!(
                    "    SELECT *, {} AS {}\n    FROM {}",
                    qi(src_col), qi(new_col), input
                );
                let src_type = schema.iter()
                    .find(|(n, _)| n == src_col)
                    .map(|(_, t)| t.clone())
                    .unwrap_or(ColumnType::Text);
                let mut new_schema = schema.to_vec();
                new_schema.push((new_col.clone(), src_type));
                (body, new_schema)
            }

            // ── Unpivot ──────────────────────────────────────────────────
            StepKind::Unpivot { input, columns, attr_col, val_col } => {
                let keep_cols: Vec<&str> = schema.iter()
                    .filter(|(n, _)| !columns.contains(n))
                    .map(|(n, _)| n.as_str())
                    .collect();
                let unions: Vec<String> = columns.iter().map(|col| {
                    let kc = keep_cols.iter().map(|c| qi(c)).collect::<Vec<_>>().join(", ");
                    format!("SELECT {}, '{}' AS {}, {} AS {} FROM {}",
                        kc, col, qi(attr_col), qi(col), qi(val_col), input)
                }).collect();
                let body = format!("    {}", unions.join("\n    UNION ALL\n    "));
                let mut new_schema: Vec<(String, ColumnType)> = schema.iter()
                    .filter(|(n, _)| !columns.contains(n))
                    .cloned()
                    .collect();
                new_schema.push((attr_col.clone(), ColumnType::Text));
                new_schema.push((val_col.clone(), ColumnType::Text));
                (body, new_schema)
            }

            // ── UnpivotOtherColumns ──────────────────────────────────────
            StepKind::UnpivotOtherColumns { input, keep_cols, attr_col, val_col } => {
                let unpivot_cols: Vec<&str> = schema.iter()
                    .filter(|(n, _)| !keep_cols.contains(n))
                    .map(|(n, _)| n.as_str())
                    .collect();
                let unions: Vec<String> = unpivot_cols.iter().map(|col| {
                    let kc = keep_cols.iter().map(|c| qi(c)).collect::<Vec<_>>().join(", ");
                    format!("SELECT {}, '{}' AS {}, {} AS {} FROM {}",
                        kc, col, qi(attr_col), qi(col), qi(val_col), input)
                }).collect();
                let body = format!("    {}", unions.join("\n    UNION ALL\n    "));
                let mut new_schema: Vec<(String, ColumnType)> = schema.iter()
                    .filter(|(n, _)| keep_cols.contains(n))
                    .cloned()
                    .collect();
                new_schema.push((attr_col.clone(), ColumnType::Text));
                new_schema.push((val_col.clone(), ColumnType::Text));
                (body, new_schema)
            }

            // ── Transpose ────────────────────────────────────────────────
            StepKind::Transpose { input } => {
                let body = format!("    SELECT * FROM {} /* TRANSPOSE */", input);
                (body, vec![])
            }

            // ── CombineTables ────────────────────────────────────────────
            StepKind::CombineTables { inputs } => {
                if inputs.is_empty() {
                    ("    SELECT NULL".to_string(), vec![])
                } else {
                    let unions: Vec<String> = inputs.iter()
                        .map(|inp| format!("SELECT * FROM {}", inp))
                        .collect();
                    let body = format!("    {}", unions.join("\n    UNION ALL\n    "));
                    (body, schema.to_vec())
                }
            }

            // ── RemoveRowsWithErrors / SelectRowsWithErrors ──────────────
            StepKind::RemoveRowsWithErrors { input, columns } => {
                let conds: Vec<String> = columns.iter()
                    .map(|c| format!("{} IS NOT NULL", qi(c)))
                    .collect();
                let where_c = if conds.is_empty() { "TRUE".to_string() } else { conds.join(" AND ") };
                let body = format!("    SELECT * FROM {} WHERE {}", input, where_c);
                (body, schema.to_vec())
            }

            StepKind::SelectRowsWithErrors { input, columns } => {
                let conds: Vec<String> = columns.iter()
                    .map(|c| format!("{} IS NULL", qi(c)))
                    .collect();
                let where_c = if conds.is_empty() { "FALSE".to_string() } else { conds.join(" OR ") };
                let body = format!("    SELECT * FROM {} WHERE {}", input, where_c);
                (body, schema.to_vec())
            }

            // ── TransformRows ────────────────────────────────────────────
            StepKind::TransformRows { input, transform } => {
                let body = format!("    SELECT {} FROM {}", emit_expr(transform), input);
                (body, schema.to_vec())
            }

            // ── MatchesAllRows / MatchesAnyRows ──────────────────────────
            StepKind::MatchesAllRows { input, condition } => {
                let body = format!(
                    "    SELECT NOT EXISTS (SELECT 1 FROM {} WHERE NOT ({})) AS \"Value\"",
                    input, emit_expr(condition)
                );
                (body, vec![("Value".to_string(), ColumnType::Boolean)])
            }

            StepKind::MatchesAnyRows { input, condition } => {
                let body = format!(
                    "    SELECT EXISTS (SELECT 1 FROM {} WHERE {}) AS \"Value\"",
                    input, emit_expr(condition)
                );
                (body, vec![("Value".to_string(), ColumnType::Boolean)])
            }

            // ── PrefixColumns ────────────────────────────────────────────
            StepKind::PrefixColumns { input, prefix } => {
                let select = schema.iter()
                    .map(|(name, _)| format!("        {} AS {}", qi(name), qi(&format!("{}.{}", prefix, name))))
                    .collect::<Vec<_>>()
                    .join(",\n");
                let body = format!("    SELECT\n{}\n    FROM {}", select, input);
                let new_schema: Vec<(String, ColumnType)> = schema.iter()
                    .map(|(n, t)| (format!("{}.{}", prefix, n), t.clone()))
                    .collect();
                (body, new_schema)
            }

            // ── DemoteHeaders ────────────────────────────────────────────
            StepKind::DemoteHeaders { input } => {
                // Insert column names as first row, rename to generic names
                let header_select = schema.iter().enumerate()
                    .map(|(i, (name, _))| format!("'{}' AS {}", name, qi(&format!("Column{}", i + 1))))
                    .collect::<Vec<_>>()
                    .join(", ");
                let data_select = schema.iter().enumerate()
                    .map(|(i, (name, _))| format!("CAST({} AS TEXT) AS {}", qi(name), qi(&format!("Column{}", i + 1))))
                    .collect::<Vec<_>>()
                    .join(", ");
                let body = format!(
                    "    SELECT {} UNION ALL SELECT {} FROM {}",
                    header_select, data_select, input
                );
                let new_schema: Vec<(String, ColumnType)> = schema.iter().enumerate()
                    .map(|(i, _)| (format!("Column{}", i + 1), ColumnType::Text))
                    .collect();
                (body, new_schema)
            }

            // ── SelectColumns ────────────────────────────────────────────
            StepKind::SelectColumns { input, columns } => {
                let select = columns.iter()
                    .map(|c| format!("        {}", qi(c)))
                    .collect::<Vec<_>>()
                    .join(",\n");
                let body = format!("    SELECT\n{}\n    FROM {}", select, input);
                let new_schema: Vec<(String, ColumnType)> = columns.iter()
                    .filter_map(|c| schema.iter().find(|(n, _)| n == c).cloned())
                    .collect();
                (body, new_schema)
            }

            // ── ReorderColumns ───────────────────────────────────────────
            StepKind::ReorderColumns { input, columns } => {
                let mut ordered: Vec<&str> = columns.iter().map(|s| s.as_str()).collect();
                for (n, _) in schema {
                    if !ordered.contains(&n.as_str()) {
                        ordered.push(n.as_str());
                    }
                }
                let select = ordered.iter()
                    .map(|c| format!("        {}", qi(c)))
                    .collect::<Vec<_>>()
                    .join(",\n");
                let body = format!("    SELECT\n{}\n    FROM {}", select, input);
                let new_schema: Vec<(String, ColumnType)> = ordered.iter()
                    .filter_map(|c| schema.iter().find(|(n, _)| n == *c).cloned())
                    .collect();
                (body, new_schema)
            }

            // ── TransformColumnNames ─────────────────────────────────────
            StepKind::TransformColumnNames { input, transform } => {
                let select = schema.iter()
                    .map(|(name, _)| format!("        {} AS {}", qi(name), emit_expr(transform)))
                    .collect::<Vec<_>>()
                    .join(",\n");
                let body = format!("    SELECT\n{}\n    FROM {}", select, input);
                (body, vec![])
            }

            // ── CombineColumns ───────────────────────────────────────────
            StepKind::CombineColumns { input, columns, combiner, new_col } => {
                let concat_expr = columns.iter()
                    .map(|c| qi(c))
                    .collect::<Vec<_>>()
                    .join(" || ");
                let _ = combiner;
                let kept = schema.iter()
                    .filter(|(n, _)| !columns.contains(n))
                    .map(|(n, _)| format!("        {}", qi(n)))
                    .collect::<Vec<_>>();
                let mut select_parts = kept;
                select_parts.push(format!("        {} AS {}", concat_expr, qi(new_col)));
                let body = format!("    SELECT\n{}\n    FROM {}", select_parts.join(",\n"), input);
                let mut new_schema: Vec<(String, ColumnType)> = schema.iter()
                    .filter(|(n, _)| !columns.contains(n))
                    .cloned()
                    .collect();
                new_schema.push((new_col.clone(), ColumnType::Text));
                (body, new_schema)
            }

            // ── SplitColumn ──────────────────────────────────────────────
            StepKind::SplitColumn { input, col_name, .. } => {
                let body = format!("    SELECT * FROM {} /* SPLIT {} */", input, qi(col_name));
                let new_schema: Vec<(String, ColumnType)> = schema.iter()
                    .filter(|(n, _)| n != col_name)
                    .cloned()
                    .collect();
                (body, new_schema)
            }

            // ── ExpandTableColumn / ExpandRecordColumn ───────────────────
            StepKind::ExpandTableColumn { input, col_name, columns }
            | StepKind::ExpandRecordColumn { input, col_name, fields: columns } => {
                let kept: Vec<String> = schema.iter()
                    .filter(|(n, _)| n != col_name)
                    .map(|(n, _)| format!("        {}", qi(n)))
                    .collect();
                let expanded: Vec<String> = columns.iter()
                    .map(|c| format!("        {}.{} AS {}", qi(col_name), qi(c), qi(c)))
                    .collect();
                let mut all = kept;
                all.extend(expanded);
                let body = format!("    SELECT\n{}\n    FROM {}", all.join(",\n"), input);
                let mut new_schema: Vec<(String, ColumnType)> = schema.iter()
                    .filter(|(n, _)| n != col_name)
                    .cloned()
                    .collect();
                for c in columns {
                    new_schema.push((c.clone(), ColumnType::Text));
                }
                (body, new_schema)
            }

            // ── Pivot ────────────────────────────────────────────────────
            StepKind::Pivot { input, .. } => {
                let body = format!("    SELECT * FROM {} /* PIVOT */", input);
                (body, vec![])
            }

            // ── RowCount ─────────────────────────────────────────────────
            StepKind::RowCount { input } => {
                let body = format!("    SELECT COUNT(*) AS \"Value\" FROM {}", input);
                (body, vec![("Value".to_string(), ColumnType::Integer)])
            }

            // ── ColumnCount ──────────────────────────────────────────────
            StepKind::ColumnCount { input } => {
                let n = schema.len();
                let body = format!("    SELECT {} AS \"Value\" FROM {}", n, input);
                (body, vec![("Value".to_string(), ColumnType::Integer)])
            }

            // ── ColumnNames ──────────────────────────────────────────────
            StepKind::TableColumnNames { input } => {
                let unions: Vec<String> = schema.iter()
                    .map(|(n, _)| format!("SELECT '{}' AS \"Value\"", n))
                    .collect();
                let body = if unions.is_empty() {
                    format!("    SELECT NULL AS \"Value\" FROM {}", input)
                } else {
                    format!("    {}", unions.join(" UNION ALL "))
                };
                (body, vec![("Value".to_string(), ColumnType::Text)])
            }

            // ── TableIsEmpty ─────────────────────────────────────────────
            StepKind::TableIsEmpty { input } => {
                let body = format!("    SELECT (COUNT(*) = 0) AS \"Value\" FROM {}", input);
                (body, vec![("Value".to_string(), ColumnType::Boolean)])
            }

            // ── TableSchema ──────────────────────────────────────────────
            StepKind::TableSchema { input } => {
                let unions: Vec<String> = schema.iter()
                    .map(|(n, t)| format!("SELECT '{}' AS \"Name\", '{}' AS \"Kind\", TRUE AS \"IsNullable\"", n, sql_type(t)))
                    .collect();
                let body = if unions.is_empty() {
                    format!("    SELECT NULL AS \"Name\", NULL AS \"Kind\", NULL AS \"IsNullable\" FROM {}", input)
                } else {
                    format!("    {}", unions.join(" UNION ALL "))
                };
                (body, vec![
                    ("Name".to_string(), ColumnType::Text),
                    ("Kind".to_string(), ColumnType::Text),
                    ("IsNullable".to_string(), ColumnType::Boolean),
                ])
            }

            // ── HasColumns ───────────────────────────────────────────────
            StepKind::HasColumns { input, columns } => {
                let has_all = columns.iter().all(|c| schema.iter().any(|(n, _)| n == c));
                let body = format!("    SELECT {} AS \"Value\" FROM {}", has_all, input);
                (body, vec![("Value".to_string(), ColumnType::Boolean)])
            }

            // ── TableIsDistinct ──────────────────────────────────────────
            StepKind::TableIsDistinct { input } => {
                let body = format!(
                    "    SELECT (COUNT(*) = COUNT(DISTINCT *)) AS \"Value\" FROM {}",
                    input
                );
                (body, vec![("Value".to_string(), ColumnType::Boolean)])
            }

            // ── Join ─────────────────────────────────────────────────────
            StepKind::Join { left, left_keys, right, right_keys, join_kind } => {
                let join_type = match join_kind {
                    pq_ast::step::JoinKind::Inner     => "INNER JOIN",
                    pq_ast::step::JoinKind::Left      => "LEFT JOIN",
                    pq_ast::step::JoinKind::Right     => "RIGHT JOIN",
                    pq_ast::step::JoinKind::Full      => "FULL OUTER JOIN",
                    pq_ast::step::JoinKind::LeftAnti  => "LEFT JOIN",
                    pq_ast::step::JoinKind::RightAnti => "RIGHT JOIN",
                };
                let on_clause: Vec<String> = left_keys.iter().zip(right_keys.iter())
                    .map(|(lk, rk)| format!("{}.{} = {}.{}", left, qi(lk), right, qi(rk)))
                    .collect();
                let mut body = format!(
                    "    SELECT * FROM {} {} {} ON {}",
                    left, join_type, right, on_clause.join(" AND ")
                );
                if matches!(join_kind, pq_ast::step::JoinKind::LeftAnti) {
                    body.push_str(&format!(" WHERE {}.{} IS NULL", right, qi(&right_keys[0])));
                } else if matches!(join_kind, pq_ast::step::JoinKind::RightAnti) {
                    body.push_str(&format!(" WHERE {}.{} IS NULL", left, qi(&left_keys[0])));
                }
                // Merge schemas
                let right_schema = self.schema_of(right);
                let mut new_schema = schema.to_vec();
                for (n, t) in &right_schema {
                    if !new_schema.iter().any(|(sn, _)| sn == n) {
                        new_schema.push((n.clone(), t.clone()));
                    }
                }
                (body, new_schema)
            }

            // ── NestedJoin ───────────────────────────────────────────────
            StepKind::NestedJoin { left, left_keys, right, right_keys, new_col, join_kind } => {
                let join_type = match join_kind {
                    pq_ast::step::JoinKind::Inner     => "INNER JOIN",
                    pq_ast::step::JoinKind::Left      => "LEFT JOIN",
                    pq_ast::step::JoinKind::Right     => "RIGHT JOIN",
                    pq_ast::step::JoinKind::Full      => "FULL OUTER JOIN",
                    _ => "LEFT JOIN",
                };
                let on_clause: Vec<String> = left_keys.iter().zip(right_keys.iter())
                    .map(|(lk, rk)| format!("{}.{} = {}.{}", left, qi(lk), right, qi(rk)))
                    .collect();
                let left_cols = schema.iter()
                    .map(|(n, _)| format!("{}.{}", left, qi(n)))
                    .collect::<Vec<_>>()
                    .join(", ");
                let body = format!(
                    "    SELECT {}, {} AS {}\n    FROM {} {} {} ON {}",
                    left_cols, right, qi(new_col), left, join_type, right, on_clause.join(" AND ")
                );
                let mut new_schema = schema.to_vec();
                new_schema.push((new_col.clone(), ColumnType::Text));
                (body, new_schema)
            }

            // ── AddRankColumn ────────────────────────────────────────────
            StepKind::AddRankColumn { input, col_name, by } => {
                let order_clause = if by.is_empty() {
                    "1".to_string()
                } else {
                    by.iter()
                        .map(|(c, o)| format!("{} {}", qi(c), sort_dir(o)))
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                let body = format!(
                    "    SELECT *, RANK() OVER(ORDER BY {}) AS {}\n    FROM {}",
                    order_clause, qi(col_name), input
                );
                let mut new_schema = schema.to_vec();
                new_schema.push((col_name.clone(), ColumnType::Integer));
                (body, new_schema)
            }

            // ── TableMax ─────────────────────────────────────────────────
            StepKind::TableMax { input, col_name } => {
                let body = format!(
                    "    SELECT * FROM {} ORDER BY {} DESC LIMIT 1",
                    input, qi(col_name)
                );
                (body, schema.to_vec())
            }

            // ── TableMin ─────────────────────────────────────────────────
            StepKind::TableMin { input, col_name } => {
                let body = format!(
                    "    SELECT * FROM {} ORDER BY {} ASC LIMIT 1",
                    input, qi(col_name)
                );
                (body, schema.to_vec())
            }

            // ── TableMaxN ────────────────────────────────────────────────
            StepKind::TableMaxN { input, count, col_name } => {
                let n = emit_expr(count);
                let body = format!(
                    "    SELECT * FROM {} ORDER BY {} DESC LIMIT {}",
                    input, qi(col_name), n
                );
                (body, schema.to_vec())
            }

            // ── TableMinN ────────────────────────────────────────────────
            StepKind::TableMinN { input, count, col_name } => {
                let n = emit_expr(count);
                let body = format!(
                    "    SELECT * FROM {} ORDER BY {} ASC LIMIT {}",
                    input, qi(col_name), n
                );
                (body, schema.to_vec())
            }

            // ── ReplaceValue ─────────────────────────────────────────────
            StepKind::ReplaceValue { input, old_value, new_value, .. } => {
                let old_sql = emit_expr(old_value);
                let new_sql = emit_expr(new_value);
                let select = schema.iter()
                    .map(|(name, _)| format!(
                        "        CASE WHEN {} = {} THEN {} ELSE {} END AS {}",
                        qi(name), old_sql, new_sql, qi(name), qi(name)
                    ))
                    .collect::<Vec<_>>()
                    .join(",\n");
                let body = format!("    SELECT\n{}\n    FROM {}", select, input);
                (body, schema.to_vec())
            }

            // ── ReplaceErrorValues ───────────────────────────────────────
            StepKind::ReplaceErrorValues { input, replacements } => {
                let select = schema.iter()
                    .map(|(name, _)| {
                        if let Some((_, repl, _)) = replacements.iter().find(|(n, _, _)| n == name) {
                            format!("        COALESCE({}, {}) AS {}", qi(name), emit_expr(repl), qi(name))
                        } else {
                            format!("        {}", qi(name))
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(",\n");
                let body = format!("    SELECT\n{}\n    FROM {}", select, input);
                (body, schema.to_vec())
            }

            // ── InsertRows ───────────────────────────────────────────────
            StepKind::InsertRows { input, .. } => {
                let body = format!("    SELECT * FROM {} /* INSERT ROWS */", input);
                (body, schema.to_vec())
            }

            // ListGenerate —— SQL does not model iteration; emit a placeholder.
            StepKind::ListGenerate { .. } => {
                let body = "    SELECT NULL AS \"Value\"".to_string();
                (body, vec![("Value".to_string(), ColumnType::Text)])
            }

            // ListTransform ───────────────────────────────────────────────
            // SQL does not have a native list-transform; emit a VALUES CTE
            // with the transform applied inline where possible.
            StepKind::ListTransform { list_expr, transform } => {
                let transform_sql = emit_expr(transform);
                let body = match &list_expr.expr {
                    Expr::List(items) => {
                        let rows = items.iter()
                            .map(|item| format!("        ({}) AS \"Value\"", emit_expr(item)))
                            .collect::<Vec<_>>()
                            .join(",\n        UNION ALL SELECT\n");
                        format!("    SELECT {}", rows)
                    }
                    _ => {
                        // Fallback: reference input step.
                        format!(
                            "    SELECT {} AS \"Value\" FROM {}",
                            transform_sql,
                            emit_expr(list_expr)
                        )
                    }
                };
                (body, vec![("Value".to_string(), ColumnType::Text)])
            }
        }
    }
}

// ── expression → SQL ─────────────────────────────────────────────────────

fn emit_expr(node: &ExprNode) -> String {
    match &node.expr {
        // ── literals ──────────────────────────────────────────────────────
        Expr::IntLit(n)       => n.to_string(),
        Expr::FloatLit(n)     => n.to_string(),
        Expr::BoolLit(true)   => "TRUE".into(),
        Expr::BoolLit(false)  => "FALSE".into(),
        Expr::StringLit(s)    => format!("'{}'", s.replace('\'', "''")),
        Expr::NullLit         => "NULL".into(),

        // ── column references ─────────────────────────────────────────────
        Expr::Identifier(name)   => qi(name),
        Expr::ColumnAccess(name) => qi(name),
        // Field access `row[col]` — emit just the column name
        Expr::FieldAccess { field, .. } => qi(field),

        // ── lambda — emit body (param ignored in SQL context) ─────────────
        // Covers both `each` (param == "_") and explicit lambdas.
        Expr::Lambda { body, .. } => emit_expr(body),

        // ── binary ops ────────────────────────────────────────────────────
        Expr::BinaryOp { left, op, right } => format!(
            "({} {} {})",
            emit_expr(left),
            op_sql(op),
            emit_expr(right)
        ),

        // ── unary ops ─────────────────────────────────────────────────────
        Expr::UnaryOp { op, operand } => match op {
            UnaryOp::Not => format!("(NOT {})", emit_expr(operand)),
            UnaryOp::Neg => format!("(-{})",    emit_expr(operand)),
        },

        // ── function call — best-effort SQL mapping ────────────────────────
        Expr::FunctionCall { name, args } => {
            let args_sql = args.iter().map(emit_expr).collect::<Vec<_>>().join(", ");
            match name.as_str() {
                // ── Text functions ────────────────────────────────────────
                "Text.Length"     => format!("LENGTH({})", args_sql),
                "Text.Upper"     => format!("UPPER({})",  args_sql),
                "Text.Lower"     => format!("LOWER({})",  args_sql),
                "Text.Trim"      => format!("TRIM({})",   args_sql),
                "Text.TrimStart" => format!("LTRIM({})",  args_sql),
                "Text.TrimEnd"   => format!("RTRIM({})",  args_sql),
                "Text.From"      => format!("CAST({} AS TEXT)", args_sql),
                "Text.Contains"  => {
                    let parts: Vec<String> = args.iter().map(emit_expr).collect();
                    if parts.len() >= 2 {
                        format!("(POSITION({1} IN {0}) > 0)", parts[0], parts[1])
                    } else {
                        format!("Text_Contains({})", args_sql)
                    }
                },
                "Text.StartsWith" => {
                    let parts: Vec<String> = args.iter().map(emit_expr).collect();
                    if parts.len() >= 2 {
                        format!("({} LIKE {} || '%')", parts[0], parts[1])
                    } else {
                        format!("Text_StartsWith({})", args_sql)
                    }
                },
                "Text.EndsWith" => {
                    let parts: Vec<String> = args.iter().map(emit_expr).collect();
                    if parts.len() >= 2 {
                        format!("({} LIKE '%' || {})", parts[0], parts[1])
                    } else {
                        format!("Text_EndsWith({})", args_sql)
                    }
                },
                "Text.Replace" => {
                    let parts: Vec<String> = args.iter().map(emit_expr).collect();
                    if parts.len() >= 3 {
                        format!("REPLACE({}, {}, {})", parts[0], parts[1], parts[2])
                    } else {
                        format!("REPLACE({})", args_sql)
                    }
                },
                "Text.Range" => {
                    let parts: Vec<String> = args.iter().map(emit_expr).collect();
                    if parts.len() >= 3 {
                        format!("SUBSTRING({} FROM ({} + 1) FOR {})", parts[0], parts[1], parts[2])
                    } else {
                        format!("SUBSTRING({})", args_sql)
                    }
                },
                "Text.Split"   => format!("STRING_TO_ARRAY({})", args_sql),
                "Text.Combine"  => {
                    let parts: Vec<String> = args.iter().map(emit_expr).collect();
                    if parts.len() >= 2 {
                        format!("ARRAY_TO_STRING({}, {})", parts[0], parts[1])
                    } else {
                        format!("ARRAY_TO_STRING({}, '')", parts.first().map(|s| s.as_str()).unwrap_or("NULL"))
                    }
                },
                "Text.PadStart" => {
                    let parts: Vec<String> = args.iter().map(emit_expr).collect();
                    if parts.len() >= 3 {
                        format!("LPAD({}, {}, {})", parts[0], parts[1], parts[2])
                    } else {
                        format!("LPAD({})", args_sql)
                    }
                },
                "Text.PadEnd" => {
                    let parts: Vec<String> = args.iter().map(emit_expr).collect();
                    if parts.len() >= 3 {
                        format!("RPAD({}, {}, {})", parts[0], parts[1], parts[2])
                    } else {
                        format!("RPAD({})", args_sql)
                    }
                },

                // ── Number functions ──────────────────────────────────────
                "Number.From"      => format!("CAST({} AS FLOAT)", args_sql),
                "Number.Round"     => format!("ROUND({})", args_sql),
                "Number.RoundUp"   => format!("CEIL({})",  args_sql),
                "Number.RoundDown" => format!("FLOOR({})", args_sql),
                "Number.Abs"       => format!("ABS({})",   args_sql),
                "Number.Sqrt"      => format!("SQRT({})",  args_sql),
                "Number.Power"     => format!("POWER({})", args_sql),
                "Number.Log"       => format!("LN({})",    args_sql),
                "Number.Mod"       => format!("MOD({})",   args_sql),
                "Number.Sign"      => format!("SIGN({})",  args_sql),

                // ── Logical functions ─────────────────────────────────────
                "Logical.From" => format!("CAST({} AS BOOLEAN)", args_sql),
                "Logical.Not"  => format!("(NOT {})", args_sql),
                "Logical.And"  => {
                    let parts: Vec<String> = args.iter().map(emit_expr).collect();
                    if parts.len() >= 2 { format!("({} AND {})", parts[0], parts[1]) }
                    else { format!("({})", args_sql) }
                },
                "Logical.Or"   => {
                    let parts: Vec<String> = args.iter().map(emit_expr).collect();
                    if parts.len() >= 2 { format!("({} OR {})", parts[0], parts[1]) }
                    else { format!("({})", args_sql) }
                },
                "Logical.Xor"  => {
                    let parts: Vec<String> = args.iter().map(emit_expr).collect();
                    if parts.len() >= 2 {
                        format!("(({0} AND NOT {1}) OR (NOT {0} AND {1}))", parts[0], parts[1])
                    } else { format!("({})", args_sql) }
                },

                // ── List aggregate functions ──────────────────────────────
                "List.Sum"               => format!("SUM({})",    args_sql),
                "List.Count"             => format!("COUNT({})",  args_sql),
                "List.Average"           => format!("AVG({})",    args_sql),
                "List.Min"               => format!("MIN({})",    args_sql),
                "List.Max"               => format!("MAX({})",    args_sql),
                "List.Median"            => format!("PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY {})", args_sql),
                "List.StandardDeviation" => format!("STDDEV_SAMP({})", args_sql),
                "List.Product"           => format!("EXP(SUM(LN({})))", args_sql),
                "List.Covariance"        => format!("COVAR_SAMP({})", args_sql),
                "List.Mode"              => format!("MODE() WITHIN GROUP (ORDER BY {})", args_sql),
                "List.Modes"             => format!("MODE() WITHIN GROUP (ORDER BY {})", args_sql),
                "List.NonNullCount"      => format!("COUNT({})",  args_sql),
                "List.IsEmpty"           => format!("(COUNT({}) = 0)", args_sql),
                "List.AllTrue"           => format!("BOOL_AND({})", args_sql),
                "List.AnyTrue"           => format!("BOOL_OR({})",  args_sql),
                "List.Contains"          => {
                    let parts: Vec<String> = args.iter().map(emit_expr).collect();
                    if parts.len() >= 2 { format!("({1} = ANY({0}))", parts[0], parts[1]) }
                    else { format!("List_Contains({})", args_sql) }
                },
                "List.First"     => format!("({})[1]", args_sql),
                "List.Last"      => format!("({})[ARRAY_LENGTH({0}, 1)]", args_sql),
                "List.Reverse"   => format!("ARRAY_REVERSE({})", args_sql),
                "List.Sort"      => format!("ARRAY_SORT({})",  args_sql),
                "List.Distinct"  => format!("ARRAY_DISTINCT({})", args_sql),
                "List.Positions" => format!("ARRAY_POSITIONS({})", args_sql),
                "List.Numbers"   => {
                    let parts: Vec<String> = args.iter().map(emit_expr).collect();
                    if parts.len() >= 2 {
                        let inc = parts.get(2).map(|s| s.as_str()).unwrap_or("1");
                        format!("GENERATE_SERIES({}, {} + ({} - 1) * {}, {})", parts[0], parts[0], parts[1], inc, inc)
                    } else { format!("GENERATE_SERIES({})", args_sql) }
                },
                "List.Random" => format!("(SELECT ARRAY_AGG(random()) FROM generate_series(1, {}))", args_sql),

                // Generic fallback: emit as-is for unknown functions.
                _ => format!("{}({})", name.replace('.', "_"), args_sql),
            }
        }

        // ── collections — not directly expressible in scalar SQL ──────────
        Expr::List(items) => {
            let inner = items.iter().map(emit_expr).collect::<Vec<_>>().join(", ");
            format!("({})", inner)
        }
        Expr::Record(fields) => {
            // Emit as a JSON-like object — only meaningful in some SQL dialects.
            let inner = fields.iter()
                .map(|(k, v)| format!("'{}', {}", k, emit_expr(v)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("JSON_OBJECT({})", inner)
        }
    }
}

fn op_sql(op: &Operator) -> &'static str {
    match op {
        Operator::Eq    => "=",
        Operator::NotEq => "<>",
        Operator::Gt    => ">",
        Operator::Lt    => "<",
        Operator::GtEq  => ">=",
        Operator::LtEq  => "<=",
        Operator::Add   => "+",
        Operator::Sub   => "-",
        Operator::Mul   => "*",
        Operator::Div   => "/",
        Operator::And   => "AND",
        Operator::Or    => "OR",
        Operator::Concat => "||",
    }
}

// ── type inference for AddColumn schema tracking ─────────────────────────

fn infer_type_sql(node: &ExprNode, schema: &[(String, ColumnType)]) -> ColumnType {
    match &node.expr {
        // Unwrap Lambda before inferring (covers both `each` and explicit lambdas)
        Expr::Lambda { body, .. } => infer_type_sql(body, schema),

        Expr::IntLit(_)         => ColumnType::Integer,
        Expr::FloatLit(_)       => ColumnType::Float,
        Expr::BoolLit(_)        => ColumnType::Boolean,
        Expr::StringLit(_)      => ColumnType::Text,
        Expr::NullLit           => ColumnType::Null,

        Expr::Identifier(name) | Expr::ColumnAccess(name) => schema.iter()
            .find(|(n, _)| n == name)
            .map(|(_, t)| t.clone())
            .unwrap_or(ColumnType::Text),

        // Field access `row[col]` — look up the field name in the schema
        Expr::FieldAccess { field, .. } => schema.iter()
            .find(|(n, _)| n == field)
            .map(|(_, t)| t.clone())
            .unwrap_or(ColumnType::Text),

        Expr::BinaryOp { left, op, right } => {
            if op.is_comparison() || op.is_logical() {
                return ColumnType::Boolean;
            }
            match (infer_type_sql(left, schema), infer_type_sql(right, schema)) {
                (ColumnType::Float, _) | (_, ColumnType::Float) => ColumnType::Float,
                (ColumnType::Integer, ColumnType::Integer)       => ColumnType::Integer,
                _ => ColumnType::Text,
            }
        }
        Expr::UnaryOp { op, operand } => match op {
            UnaryOp::Not => ColumnType::Boolean,
            UnaryOp::Neg => infer_type_sql(operand, schema),
        },
        Expr::FunctionCall { name, .. } => match name.as_str() {
            "Text.Length" | "List.Count" | "List.NonNullCount"
            | "Number.Sign"              => ColumnType::Integer,
            "Number.From" | "List.Sum" | "List.Average" | "List.Median"
            | "List.StandardDeviation" | "List.Covariance" | "List.Product"
            | "Number.Round" | "Number.RoundUp" | "Number.RoundDown"
            | "Number.Abs" | "Number.Sqrt" | "Number.Power" | "Number.Log"
            | "Number.Mod"               => ColumnType::Float,
            "Text.From" | "Text.Upper" | "Text.Lower" | "Text.Trim"
            | "Text.TrimStart" | "Text.TrimEnd" | "Text.PadStart" | "Text.PadEnd"
            | "Text.Range" | "Text.Replace" | "Text.Combine"
                                         => ColumnType::Text,
            "Logical.From" | "Logical.Not" | "Logical.And" | "Logical.Or" | "Logical.Xor"
            | "Text.Contains" | "Text.StartsWith" | "Text.EndsWith"
            | "List.IsEmpty" | "List.AllTrue" | "List.AnyTrue" | "List.Contains"
                                         => ColumnType::Boolean,
            _                            => ColumnType::Text,
        },
        Expr::List(_) | Expr::Record(_) => ColumnType::Text,
    }
}

// ── small helpers ─────────────────────────────────────────────────────────

fn qi(name: &str) -> String {
    format!("\"{}\"", name)
}

fn sql_type(t: &ColumnType) -> &'static str {
    match t {
        ColumnType::Integer      => "BIGINT",
        ColumnType::Float        => "FLOAT",
        ColumnType::Boolean      => "BOOLEAN",
        ColumnType::Text         => "TEXT",
        ColumnType::Date         => "DATE",
        ColumnType::DateTime     => "TIMESTAMP",
        ColumnType::Currency     => "DECIMAL",
        ColumnType::DateTimeZone => "TIMESTAMPTZ",
        ColumnType::Duration     => "INTERVAL",
        ColumnType::Time         => "TIME",
        ColumnType::Binary       => "BYTEA",
        ColumnType::Null         => "NULL",
        ColumnType::Function(_)  => "TEXT",
        ColumnType::List(_)      => "TEXT",
    }
}

fn sort_dir(order: &SortOrder) -> &'static str {
    match order {
        SortOrder::Ascending  => "ASC",
        SortOrder::Descending => "DESC",
    }
}

/// Return the name of the input step for any StepKind.
fn step_input(kind: &StepKind) -> &str {
    match kind {
        StepKind::Source { .. }                  => "",
        StepKind::PromoteHeaders  { input }
        | StepKind::ChangeTypes   { input, .. }
        | StepKind::Filter        { input, .. }
        | StepKind::AddColumn     { input, .. }
        | StepKind::RemoveColumns { input, .. }
        | StepKind::RenameColumns { input, .. }
        | StepKind::Sort          { input, .. }
        | StepKind::TransformColumns { input, .. }
        | StepKind::Group         { input, .. }
        | StepKind::Passthrough   { input, .. }
        // New operations
        | StepKind::FirstN        { input, .. }
        | StepKind::LastN         { input, .. }
        | StepKind::Skip          { input, .. }
        | StepKind::Range         { input, .. }
        | StepKind::RemoveFirstN  { input, .. }
        | StepKind::RemoveLastN   { input, .. }
        | StepKind::RemoveRows    { input, .. }
        | StepKind::ReverseRows   { input }
        | StepKind::Distinct      { input, .. }
        | StepKind::Repeat        { input, .. }
        | StepKind::AlternateRows { input, .. }
        | StepKind::FindText      { input, .. }
        | StepKind::FillDown      { input, .. }
        | StepKind::FillUp        { input, .. }
        | StepKind::AddIndexColumn { input, .. }
        | StepKind::DuplicateColumn { input, .. }
        | StepKind::Unpivot       { input, .. }
        | StepKind::UnpivotOtherColumns { input, .. }
        | StepKind::Transpose     { input }
        | StepKind::RemoveRowsWithErrors  { input, .. }
        | StepKind::SelectRowsWithErrors  { input, .. }
        | StepKind::TransformRows { input, .. }
        | StepKind::MatchesAllRows { input, .. }
        | StepKind::MatchesAnyRows { input, .. }
        | StepKind::PrefixColumns  { input, .. }
        | StepKind::DemoteHeaders  { input }
        // New operations
        | StepKind::SelectColumns       { input, .. }
        | StepKind::ReorderColumns      { input, .. }
        | StepKind::TransformColumnNames { input, .. }
        | StepKind::CombineColumns      { input, .. }
        | StepKind::SplitColumn          { input, .. }
        | StepKind::ExpandTableColumn   { input, .. }
        | StepKind::ExpandRecordColumn  { input, .. }
        | StepKind::Pivot               { input, .. }
        | StepKind::RowCount            { input }
        | StepKind::ColumnCount         { input }
        | StepKind::TableColumnNames    { input }
        | StepKind::TableIsEmpty        { input }
        | StepKind::TableSchema         { input }
        | StepKind::HasColumns          { input, .. }
        | StepKind::TableIsDistinct     { input }
        | StepKind::AddRankColumn       { input, .. }
        | StepKind::TableMax            { input, .. }
        | StepKind::TableMin            { input, .. }
        | StepKind::TableMaxN           { input, .. }
        | StepKind::TableMinN           { input, .. }
        | StepKind::ReplaceValue        { input, .. }
        | StepKind::ReplaceErrorValues  { input, .. }
        | StepKind::InsertRows          { input, .. } => input.as_str(),
        StepKind::Join { left, .. }
        | StepKind::NestedJoin { left, .. } => left.as_str(),
        StepKind::CombineTables { inputs } => {
            inputs.first().map(|s| s.as_str()).unwrap_or("")
        }
        // ListGenerate / ListTransform have no table input step.
        StepKind::ListGenerate { .. }
        | StepKind::ListTransform { .. }         => "",
    }
}

// ── tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use pq_lexer::Lexer;
    use pq_parser::Parser;
    use pq_pipeline::{build_table, RawWorkbook};

    fn table() -> Table {
        build_table(RawWorkbook {
            source: "workbook.xlsx".into(),
            sheet:  "Sales".into(),
            rows:   vec![
                vec!["Name".into(), "Age".into(), "Salary".into(), "Active".into()],
                vec!["Alice".into(), "30".into(), "50000.50".into(), "true".into()],
                vec!["Bob".into(),   "25".into(), "40000.00".into(), "false".into()],
            ],
        })
    }

    fn parse_and_gen(formula: &str) -> String {
        let t      = table();
        let tokens = Lexer::new(formula).tokenize().unwrap();
        let prog   = Parser::new(tokens).parse().unwrap();
        generate_sql(&prog, &t)
    }

    #[test]
    fn test_source_only() {
        let sql = parse_and_gen(
            r#"let Source = Excel.Workbook(File.Contents("workbook.xlsx"), null, true) in Source"#
        );
        assert!(sql.contains("SELECT * FROM \"workbook\""));
    }

    #[test]
    fn test_filter() {
        let sql = parse_and_gen(r#"
            let
                Source   = Excel.Workbook(File.Contents("workbook.xlsx"), null, true),
                Filtered = Table.SelectRows(Source, each Age > 25)
            in Filtered
        "#);
        assert!(sql.contains("WHERE (\"Age\" > 25)"));
    }

    #[test]
    fn test_filter_with_and() {
        let sql = parse_and_gen(r#"
            let
                Source   = Excel.Workbook(File.Contents("workbook.xlsx"), null, true),
                Filtered = Table.SelectRows(Source, each Age > 25 and Active = true)
            in Filtered
        "#);
        assert!(sql.contains("AND"));
        assert!(sql.contains("WHERE"));
    }

    #[test]
    fn test_add_column() {
        let sql = parse_and_gen(r#"
            let
                Source    = Excel.Workbook(File.Contents("workbook.xlsx"), null, true),
                WithBonus = Table.AddColumn(Source, "Bonus", each Salary + 1000.0)
            in WithBonus
        "#);
        assert!(sql.contains("AS \"Bonus\""));
        assert!(sql.contains("\"Salary\""));
    }

    #[test]
    fn test_remove_columns() {
        let sql = parse_and_gen(r#"
            let
                Source  = Excel.Workbook(File.Contents("workbook.xlsx"), null, true),
                Removed = Table.RemoveColumns(Source, {"Active"})
            in Removed
        "#);
        assert!(!sql.contains("\"Active\""));
        assert!(sql.contains("\"Name\""));
    }

    #[test]
    fn test_rename_columns() {
        let sql = parse_and_gen(r#"
            let
                Source  = Excel.Workbook(File.Contents("workbook.xlsx"), null, true),
                Renamed = Table.RenameColumns(Source, {{"Name", "FullName"}})
            in Renamed
        "#);
        assert!(sql.contains("\"Name\" AS \"FullName\""));
    }

    #[test]
    fn test_sort() {
        let sql = parse_and_gen(r#"
            let
                Source = Excel.Workbook(File.Contents("workbook.xlsx"), null, true),
                Sorted = Table.Sort(Source, {{"Age", Order.Ascending}})
            in Sorted
        "#);
        assert!(sql.contains("ORDER BY \"Age\" ASC"));
    }

    #[test]
    fn test_bracket_column_access() {
        let sql = parse_and_gen(r#"
            let
                Source   = Excel.Workbook(File.Contents("workbook.xlsx"), null, true),
                Filtered = Table.SelectRows(Source, each [Age] > 25)
            in Filtered
        "#);
        assert!(sql.contains("WHERE (\"Age\" > 25)"));
    }

    #[test]
    fn test_full_pipeline() {
        let sql = parse_and_gen(r#"
            let
                Source          = Excel.Workbook(File.Contents("workbook.xlsx"), null, true),
                PromotedHeaders = Table.PromoteHeaders(Source),
                ChangedTypes    = Table.TransformColumnTypes(PromotedHeaders, {{"Age", Int64.Type}, {"Salary", Number.Type}}),
                Filtered        = Table.SelectRows(ChangedTypes, each Age > 25),
                WithBonus       = Table.AddColumn(Filtered, "Bonus", each Salary + 1000.0),
                Removed         = Table.RemoveColumns(WithBonus, {"Active"}),
                Renamed         = Table.RenameColumns(Removed, {{"Name", "FullName"}}),
                Sorted          = Table.Sort(Renamed, {{"Age", Order.Ascending}})
            in Sorted
        "#);
        assert!(sql.contains("WITH"));
        assert!(sql.contains("CAST(\"Age\" AS BIGINT)"));
        assert!(sql.contains("WHERE (\"Age\" > 25)"));
        assert!(sql.contains("AS \"Bonus\""));
        assert!(sql.contains("ORDER BY \"Age\" ASC"));
        assert!(sql.contains("\"Name\" AS \"FullName\""));
        let removed_cte_start = sql.find("  Removed AS").expect("Removed CTE not found");
        let removed_cte       = &sql[removed_cte_start..];
        let removed_cte_end   = removed_cte.find("\n  )").unwrap_or(removed_cte.len());
        assert!(!removed_cte[..removed_cte_end].contains("\"Active\""));
    }
}

