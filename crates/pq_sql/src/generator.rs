use std::collections::HashMap;

use pq_ast::{
    Program,
    call_arg::CallArg,
    expr::{Expr, ExprNode},
    step::{StepKind, SortOrder},
    MissingFieldKind,
};
use pq_grammar::operators::{Operator, UnaryOp};
use pq_pipeline::Table;
use pq_types::ColumnType;

// â”€â”€ public entry point â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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

// â”€â”€ internal â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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
            let in_name = pq_ast::step_input(&binding.step.kind);
            let schema  = self.schema_of(in_name);

            let (body, out_schema) =
                self.emit(&binding.step.kind, &schema, &mut order_by);

            ctes.push(format!("  {} AS (\n{}\n  )", binding.name, body));
            self.schemas.insert(binding.name.clone(), out_schema);
        }

        // Choose the FROM target.  When the `in` clause is a non-identifier
        // expression (function call, etc.) we don't have a single CTE to
        // SELECT from, so fall back to the last named binding.
        let from_target = if program.output_expr.is_some() {
            program.steps.last()
                .map(|b| b.name.clone())
                .unwrap_or_else(|| "Source".to_string())
        } else {
            program.output.clone()
        };
        let mut sql = format!("SELECT *\nFROM {}", from_target);
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

    // â”€â”€ step â†’ (CTE body SQL, output schema) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

    fn emit(
        &self,
        kind:     &StepKind,
        schema:   &[(String, ColumnType)],
        order_by: &mut Vec<(String, SortOrder)>,
    ) -> (String, Vec<(String, ColumnType)>) {
        match kind {

            // Source â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
            StepKind::Source { path, .. } => {
                let table_name = std::path::Path::new(path.as_str())
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(path.as_str());
                let body = format!("    SELECT * FROM {}", qi(table_name));
                (body, schema.to_vec())
            }

            // NavigateSheet â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
            StepKind::NavigateSheet { input, .. } => {
                let body = format!("    SELECT * FROM {}", input);
                (body, schema.to_vec())
            }

            // ValueBinding â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
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

            // FunctionCall â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
            StepKind::FunctionCall { name, args } => {
                // Primary (left) input step name
                let input = args.first().and_then(|a| a.as_step_ref()).unwrap_or("");

                match name.as_str() {
                    // â”€â”€ PromoteHeaders â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.PromoteHeaders" => {
                        (format!("    SELECT * FROM {}", input), schema.to_vec())
                    }

                    // â”€â”€ TransformColumnTypes (ChangeTypes) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
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

                    // â”€â”€ SelectRows (Filter) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.SelectRows" => {
                        if let Some(cond) = args.get(1).and_then(|a| a.as_expr()) {
                            let body = format!("    SELECT *\n    FROM {}\n    WHERE {}", input, emit_expr(cond));
                            (body, schema.to_vec())
                        } else {
                            (format!("    SELECT * FROM {}", input), schema.to_vec())
                        }
                    }

                    // â”€â”€ AddColumn â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.AddColumn" => {
                        let col_name  = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                        let expr_sql  = args.get(2).and_then(|a| a.as_expr()).map(emit_expr).unwrap_or_else(|| "NULL".to_string());
                        let body = format!("    SELECT\n        *,\n        {} AS {}\n    FROM {}", expr_sql, qi(col_name), input);
                        let inferred  = args.get(2).and_then(|a| a.as_expr()).map(|e| infer_type_sql(e, schema)).unwrap_or(ColumnType::Text);
                        let mut new_schema = schema.to_vec();
                        new_schema.push((col_name.to_string(), inferred));
                        (body, new_schema)
                    }

                    // â”€â”€ RemoveColumns â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.RemoveColumns" => {
                        let columns = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let kept: Vec<&(String, ColumnType)> = schema.iter().filter(|(name, _)| !columns.contains(name)).collect();
                        let select = kept.iter().map(|(name, _)| format!("        {}", qi(name))).collect::<Vec<_>>().join(",\n");
                        let body = format!("    SELECT\n{}\n    FROM {}", select, input);
                        let new_schema = kept.iter().map(|(n, t)| (n.clone(), t.clone())).collect();
                        (body, new_schema)
                    }

                    // â”€â”€ RenameColumns â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
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

                    // â”€â”€ Sort â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.Sort" => {
                        let sort_list = args.get(1).and_then(|a| a.as_sort_list()).unwrap_or(&[]);
                        *order_by = sort_list.to_vec();
                        (format!("    SELECT * FROM {}", input), schema.to_vec())
                    }

                    // â”€â”€ TransformColumns â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
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

                    // â”€â”€ Group â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
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

                    // â”€â”€ FirstN â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.FirstN" => {
                        if let Some(n) = args.get(1).and_then(|a| a.as_int()) {
                            // Integer count → simple LIMIT.
                            (format!("    SELECT * FROM {} LIMIT {}", input, n), schema.to_vec())
                        } else if let Some(pred) = args.get(1).and_then(|a| a.as_expr()) {
                            // Take-while predicate → window-based cutoff.
                            let pred_sql = emit_expr(pred);
                            let body = format!(
                                "    WITH _t AS (SELECT *, ROW_NUMBER() OVER () AS _rn FROM {input}),\n         _stop AS (SELECT MIN(_rn) AS _first_fail FROM _t WHERE NOT ({pred_sql}))\n    SELECT * FROM _t WHERE _rn < COALESCE((SELECT _first_fail FROM _stop), _rn + 1)",
                                input = input, pred_sql = pred_sql);
                            (body, schema.to_vec())
                        } else {
                            (format!("    SELECT * FROM {} LIMIT 0", input), schema.to_vec())
                        }
                    }

                    // â”€â”€ LastN â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.LastN" => {
                        if let Some(n) = args.get(1).and_then(|a| a.as_int()) {
                            let body = format!("    SELECT * FROM (SELECT *, ROW_NUMBER() OVER() AS _rn FROM {0}) AS _t WHERE _rn > (SELECT COUNT(*) FROM {0}) - {1}", input, n);
                            (body, schema.to_vec())
                        } else if let Some(pred) = args.get(1).and_then(|a| a.as_expr()) {
                            // Take-while-from-bottom: reverse-number rows, find first
                            // failure in reverse order, keep only rows before that cutoff.
                            let pred_sql = emit_expr(pred);
                            let body = format!(
                                "    WITH _t AS (SELECT *, ROW_NUMBER() OVER () AS _rn, ROW_NUMBER() OVER (ORDER BY (SELECT NULL) DESC) AS _rrn FROM {input}),\n         _stop AS (SELECT MIN(_rrn) AS _first_fail FROM _t WHERE NOT ({pred_sql}))\n    SELECT * FROM _t WHERE _rrn < COALESCE((SELECT _first_fail FROM _stop), _rrn + 1) ORDER BY _rn",
                                input = input, pred_sql = pred_sql);
                            (body, schema.to_vec())
                        } else {
                            (format!("    SELECT * FROM {} LIMIT 0", input), schema.to_vec())
                        }
                    }

                    // â”€â”€ Skip / RemoveFirstN â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.Skip" | "Table.RemoveFirstN" => {
                        let n = args.get(1).and_then(|a| a.as_int()).unwrap_or(0);
                        (format!("    SELECT * FROM {} OFFSET {}", input, n), schema.to_vec())
                    }

                    // â”€â”€ Range â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.Range" => {
                        let off = args.get(1).and_then(|a| a.as_int()).unwrap_or(0);
                        // count absent or null → no LIMIT (return all rows after offset)
                        let cnt_opt: Option<i64> = args.get(2).and_then(|a| {
                            a.as_int().or_else(|| {
                                // If it's an expression that is literally NullLit, treat as absent.
                                a.as_expr().and_then(|e| {
                                    use pq_ast::expr::Expr;
                                    if matches!(e.expr, Expr::NullLit) { None }
                                    else { None } // other exprs: fall back to no LIMIT
                                })
                            })
                        });
                        let body = match cnt_opt {
                            Some(n) => format!("    SELECT * FROM {} LIMIT {} OFFSET {}", input, n, off),
                            None    => format!("    SELECT * FROM {} OFFSET {}",          input, off),
                        };
                        (body, schema.to_vec())
                    }

                    // â”€â”€ RemoveLastN â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.RemoveLastN" => {
                        let n = args.get(1).and_then(|a| a.as_int()).unwrap_or(0);
                        let body = format!("    SELECT * FROM (SELECT *, ROW_NUMBER() OVER() AS _rn FROM {0}) AS _t WHERE _rn <= (SELECT COUNT(*) FROM {0}) - {1}", input, n);
                        (body, schema.to_vec())
                    }

                    // â”€â”€ RemoveRows â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.RemoveRows" => {
                        let off = args.get(1).and_then(|a| a.as_int()).unwrap_or(0);
                        let cnt = args.get(2).and_then(|a| a.as_int()).unwrap_or(0);
                        let body = format!("    SELECT * FROM (SELECT *, ROW_NUMBER() OVER() - 1 AS _rn FROM {}) AS _t WHERE _rn < {} OR _rn >= {} + {}", input, off, off, cnt);
                        (body, schema.to_vec())
                    }

                    // â”€â”€ ReverseRows â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.ReverseRows" => {
                        let body = format!("    SELECT * FROM (SELECT *, ROW_NUMBER() OVER() AS _rn FROM {}) AS _t ORDER BY _rn DESC", input);
                        (body, schema.to_vec())
                    }

                    // â”€â”€ Distinct â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
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

                    // â”€â”€ Repeat â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.Repeat" => {
                        let n = args.get(1).and_then(|a| a.as_int()).unwrap_or(1);
                        let body = format!("    SELECT * FROM {} CROSS JOIN generate_series(1, {}) AS _g", input, n);
                        (body, schema.to_vec())
                    }

                    // â”€â”€ AlternateRows â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.AlternateRows" => {
                        let off = args.get(1).and_then(|a| a.as_int()).unwrap_or(0);
                        let sk  = args.get(2).and_then(|a| a.as_int()).unwrap_or(1);
                        let tk  = args.get(3).and_then(|a| a.as_int()).unwrap_or(1);
                        let body = format!("    SELECT * FROM (SELECT *, ROW_NUMBER() OVER() - 1 AS _rn FROM {}) AS _t WHERE _rn >= {} AND (_rn - {}) % ({} + {}) < {}", input, off, off, sk, tk, tk);
                        (body, schema.to_vec())
                    }

                    // â”€â”€ FindText â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.FindText" => {
                        let text = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                        let conditions: Vec<String> = schema.iter()
                            .map(|(name, _)| format!("LOWER(CAST({} AS TEXT)) LIKE '%{}%'", qi(name), text.to_lowercase().replace('\'', "''")))
                            .collect();
                        let where_clause = if conditions.is_empty() { "TRUE".to_string() } else { conditions.join(" OR ") };
                        (format!("    SELECT * FROM {} WHERE {}", input, where_clause), schema.to_vec())
                    }

                    // â”€â”€ FillDown / FillUp â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
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

                    // â”€â”€ AddIndexColumn â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.AddIndexColumn" => {
                        let col_name = args.get(1).and_then(|a| a.as_str()).unwrap_or("Index");
                        let start    = args.get(2).and_then(|a| a.as_int()).unwrap_or(0);
                        let step     = args.get(3).and_then(|a| a.as_int()).unwrap_or(1);
                        let body = format!("    SELECT *, (ROW_NUMBER() OVER() - 1) * {} + {} AS {}\n    FROM {}", step, start, qi(col_name), input);
                        let mut new_schema = schema.to_vec();
                        new_schema.push((col_name.to_string(), ColumnType::Integer));
                        (body, new_schema)
                    }

                    // â”€â”€ DuplicateColumn â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.DuplicateColumn" => {
                        let src_col = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                        let new_col = args.get(2).and_then(|a| a.as_str()).unwrap_or("");
                        let body = format!("    SELECT *, {} AS {}\n    FROM {}", qi(src_col), qi(new_col), input);
                        let src_type = schema.iter().find(|(n, _)| n == src_col).map(|(_, t)| t.clone()).unwrap_or(ColumnType::Text);
                        let mut new_schema = schema.to_vec();
                        new_schema.push((new_col.to_string(), src_type));
                        (body, new_schema)
                    }

                    // â”€â”€ Unpivot â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
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

                    // â”€â”€ UnpivotOtherColumns â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
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

                    // â”€â”€ Transpose â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.Transpose" => {
                        (format!("    SELECT * FROM {} /* TRANSPOSE */", input), vec![])
                    }

                    // â”€â”€ Combine (CombineTables) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
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

                    // â”€â”€ RemoveRowsWithErrors â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.RemoveRowsWithErrors" => {
                        let columns = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let conds: Vec<String> = columns.iter().map(|c| format!("{} IS NOT NULL", qi(c))).collect();
                        let where_c = if conds.is_empty() { "TRUE".to_string() } else { conds.join(" AND ") };
                        (format!("    SELECT * FROM {} WHERE {}", input, where_c), schema.to_vec())
                    }

                    // â”€â”€ SelectRowsWithErrors â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.SelectRowsWithErrors" => {
                        let columns = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let conds: Vec<String> = columns.iter().map(|c| format!("{} IS NULL", qi(c))).collect();
                        let where_c = if conds.is_empty() { "FALSE".to_string() } else { conds.join(" OR ") };
                        (format!("    SELECT * FROM {} WHERE {}", input, where_c), schema.to_vec())
                    }

                    // â”€â”€ TransformRows â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.TransformRows" => {
                        if let Some(expr) = args.get(1).and_then(|a| a.as_expr()) {
                            (format!("    SELECT {} FROM {}", emit_expr(expr), input), schema.to_vec())
                        } else {
                            (format!("    SELECT * FROM {}", input), schema.to_vec())
                        }
                    }

                    // â”€â”€ MatchesAllRows â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.MatchesAllRows" => {
                        if let Some(cond) = args.get(1).and_then(|a| a.as_expr()) {
                            let body = format!("    SELECT NOT EXISTS (SELECT 1 FROM {} WHERE NOT ({})) AS \"Value\"", input, emit_expr(cond));
                            (body, vec![("Value".to_string(), ColumnType::Boolean)])
                        } else {
                            ("    SELECT TRUE AS \"Value\"".to_string(), vec![("Value".to_string(), ColumnType::Boolean)])
                        }
                    }

                    // â”€â”€ MatchesAnyRows â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.MatchesAnyRows" => {
                        if let Some(cond) = args.get(1).and_then(|a| a.as_expr()) {
                            let body = format!("    SELECT EXISTS (SELECT 1 FROM {} WHERE {}) AS \"Value\"", input, emit_expr(cond));
                            (body, vec![("Value".to_string(), ColumnType::Boolean)])
                        } else {
                            ("    SELECT FALSE AS \"Value\"".to_string(), vec![("Value".to_string(), ColumnType::Boolean)])
                        }
                    }

                    // â”€â”€ PrefixColumns â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.PrefixColumns" => {
                        let prefix = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                        let select = schema.iter()
                            .map(|(name, _)| format!("        {} AS {}", qi(name), qi(&format!("{}.{}", prefix, name))))
                            .collect::<Vec<_>>().join(",\n");
                        let body = format!("    SELECT\n{}\n    FROM {}", select, input);
                        let new_schema: Vec<(String, ColumnType)> = schema.iter().map(|(n, t)| (format!("{}.{}", prefix, n), t.clone())).collect();
                        (body, new_schema)
                    }

                    // â”€â”€ DemoteHeaders â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
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

                    // â”€â”€ SelectColumns â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
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

                    // â”€â”€ ReorderColumns â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.ReorderColumns" => {
                        let columns = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let mut ordered: Vec<&str> = columns.iter().map(|s| s.as_str()).collect();
                        for (n, _) in schema { if !ordered.contains(&n.as_str()) { ordered.push(n.as_str()); } }
                        let select = ordered.iter().map(|c| format!("        {}", qi(c))).collect::<Vec<_>>().join(",\n");
                        let body = format!("    SELECT\n{}\n    FROM {}", select, input);
                        let new_schema: Vec<(String, ColumnType)> = ordered.iter().filter_map(|c| schema.iter().find(|(n, _)| n == *c).cloned()).collect();
                        (body, new_schema)
                    }

                    // â”€â”€ TransformColumnNames â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
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

                    // â”€â”€ CombineColumns â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
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

                    // â”€â”€ SplitColumn â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.SplitColumn" => {
                        let col_name = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                        let body = format!("    SELECT * FROM {} /* SPLIT {} */", input, qi(col_name));
                        let new_schema: Vec<(String, ColumnType)> = schema.iter().filter(|(n, _)| n != col_name).cloned().collect();
                        (body, new_schema)
                    }

                    // â”€â”€ ExpandTableColumn / ExpandRecordColumn â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.ExpandTableColumn" | "Table.ExpandRecordColumn" => {
                        let col_name: String = match args.get(1) {
                            Some(CallArg::Str(s)) => s.clone(),
                            Some(CallArg::Expr(e)) => match &e.expr {
                                Expr::StringLit(s) => s.clone(),
                                _ => String::new(),
                            },
                            _ => String::new(),
                        };
                        let inner_cols = args.get(2).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let new_names_opt: Option<Vec<String>> = args.get(3).and_then(|a| a.as_expr()).and_then(|e| {
                            if let Expr::List(items) = &e.expr {
                                let names: Vec<String> = items.iter().filter_map(|it| {
                                    if let Expr::StringLit(s) = &it.expr { Some(s.clone()) } else { None }
                                }).collect();
                                if names.len() == items.len() { Some(names) } else { None }
                            } else { None }
                        });
                        let output_names: Vec<String> = new_names_opt.unwrap_or_else(|| inner_cols.iter().cloned().collect());
                        let kept: Vec<String> = schema.iter()
                            .filter(|(n, _)| n != &col_name)
                            .map(|(n, _)| format!("        {}", qi(n))).collect();
                        let expanded: Vec<String> = inner_cols.iter().zip(output_names.iter())
                            .map(|(inner, out)| format!("        {}.{} AS {}", qi(&col_name), qi(inner), qi(out))).collect();
                        let mut all = kept; all.extend(expanded);
                        let body = format!("    SELECT\n{}\n    FROM {}", all.join(",\n"), input);
                        let mut new_schema: Vec<(String, ColumnType)> = schema.iter()
                            .filter(|(n, _)| n != &col_name).cloned().collect();
                        for out_name in &output_names { new_schema.push((out_name.clone(), ColumnType::Text)); }
                        (body, new_schema)
                    }

                    // â”€â”€ Pivot â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.Pivot" => {
                        (format!("    SELECT * FROM {} /* PIVOT */", input), vec![])
                    }

                    // â”€â”€ RowCount â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.RowCount" => {
                        let body = format!("    SELECT COUNT(*) AS \"Value\" FROM {}", input);
                        (body, vec![("Value".to_string(), ColumnType::Integer)])
                    }

                    // â”€â”€ ColumnCount â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.ColumnCount" => {
                        let n = schema.len();
                        let body = format!("    SELECT {} AS \"Value\" FROM {}", n, input);
                        (body, vec![("Value".to_string(), ColumnType::Integer)])
                    }

                    // â”€â”€ ColumnNames â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.ColumnNames" => {
                        let unions: Vec<String> = schema.iter().map(|(n, _)| format!("SELECT '{}' AS \"Value\"", n)).collect();
                        let body = if unions.is_empty() {
                            format!("    SELECT NULL AS \"Value\" FROM {}", input)
                        } else { format!("    {}", unions.join(" UNION ALL ")) };
                        (body, vec![("Value".to_string(), ColumnType::Text)])
                    }

                    // â”€â”€ ColumnsOfType â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
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

                    // â”€â”€ IsEmpty â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.IsEmpty" => {
                        let body = format!("    SELECT (COUNT(*) = 0) AS \"Value\" FROM {}", input);
                        (body, vec![("Value".to_string(), ColumnType::Boolean)])
                    }

                    // â”€â”€ Schema â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
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

                    // â”€â”€ HasColumns â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.HasColumns" => {
                        let columns = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
                        let has_all = columns.iter().all(|c| schema.iter().any(|(n, _)| n == c));
                        let body = format!("    SELECT {} AS \"Value\" FROM {}", has_all, input);
                        (body, vec![("Value".to_string(), ColumnType::Boolean)])
                    }

                    // â”€â”€ IsDistinct â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.IsDistinct" => {
                        let body = format!("    SELECT (COUNT(*) = COUNT(DISTINCT *)) AS \"Value\" FROM {}", input);
                        (body, vec![("Value".to_string(), ColumnType::Boolean)])
                    }

                    // â”€â”€ Join â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
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

                    // â”€â”€ NestedJoin â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
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

                    // â”€â”€ AddRankColumn â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
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

                    // â”€â”€ TableMax / TableMin â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.Max" => {
                        let col = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                        (format!("    SELECT * FROM {} ORDER BY {} DESC LIMIT 1", input, qi(col)), schema.to_vec())
                    }
                    "Table.Min" => {
                        let col = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                        (format!("    SELECT * FROM {} ORDER BY {} ASC LIMIT 1", input, qi(col)), schema.to_vec())
                    }

                    // â”€â”€ TableMaxN / TableMinN â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
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

                    // â”€â”€ ReplaceValue â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.ReplaceValue" => {
                        let old_val = args.get(1).and_then(|a| a.as_expr()).map(emit_expr).unwrap_or_else(|| "NULL".to_string());
                        let new_val = args.get(2).and_then(|a| a.as_expr()).map(emit_expr).unwrap_or_else(|| "NULL".to_string());
                        let select = schema.iter().map(|(name, _)| format!(
                            "        CASE WHEN {} = {} THEN {} ELSE {} END AS {}", qi(name), old_val, new_val, qi(name), qi(name)
                        )).collect::<Vec<_>>().join(",\n");
                        (format!("    SELECT\n{}\n    FROM {}", select, input), schema.to_vec())
                    }

                    // â”€â”€ ReplaceErrorValues â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.ReplaceErrorValues" => {
                        let replacements = args.get(1).and_then(|a| a.as_transform_list()).unwrap_or(&[]);
                        let select = schema.iter().map(|(name, _)| {
                            if let Some((_, repl, _)) = replacements.iter().find(|(n, _, _)| n == name) {
                                format!("        COALESCE({}, {}) AS {}", qi(name), emit_expr(repl), qi(name))
                            } else { format!("        {}", qi(name)) }
                        }).collect::<Vec<_>>().join(",\n");
                        (format!("    SELECT\n{}\n    FROM {}", select, input), schema.to_vec())
                    }

                    // â”€â”€ InsertRows â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.InsertRows" => {
                        (format!("    SELECT * FROM {} /* INSERT ROWS */", input), schema.to_vec())
                    }

                    // â”€â”€ ListGenerate â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "List.Generate" => {
                        ("    SELECT NULL AS \"Value\"".to_string(), vec![("Value".to_string(), ColumnType::Text)])
                    }

                    // â”€â”€ ListSelect â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    // -- List.Intersect -------------------------------------------
                    // N-ary intersection of a list-of-lists.
                    // For a literal outer list of literal inner lists, emit SQL
                    // INTERSECT. For other shapes, fall through to the runtime executor.
                    // NOTE: SQL INTERSECT uses set semantics; the runtime executor
                    // correctly honours multiset (MIN multiplicity) semantics.
                    "List.Intersect" => {
                        let body = match args.first() {
                            Some(CallArg::Expr(e)) => match &e.expr {
                                Expr::List(outer_items) => {
                                    let parts: Vec<String> = outer_items.iter().filter_map(|inner| {
                                        if let Expr::List(items) = &inner.expr {
                                            let rows = items.iter()
                                                .map(|i| format!("SELECT {} AS \"Value\"", emit_expr(i)))
                                                .collect::<Vec<_>>()
                                                .join(" UNION ALL ");
                                            Some(format!("({})", rows))
                                        } else {
                                            None
                                        }
                                    }).collect();
                                    if parts.is_empty() {
                                        "    SELECT NULL AS \"Value\" WHERE 1=0".to_string()
                                    } else {
                                        format!("    {}", parts.join(" INTERSECT "))
                                    }
                                }
                                _ => format!("    SELECT \"Value\" FROM {} WHERE 1=1", emit_expr(e)),
                            },
                            Some(CallArg::StepRef(s)) => {
                                format!("    SELECT \"Value\" FROM {} WHERE 1=1", s)
                            }
                            _ => "    SELECT NULL AS \"Value\" WHERE 1=0".to_string(),
                        };
                        (body, vec![("Value".to_string(), ColumnType::Text)])
                    }

                    // -- List.Difference ------------------------------------------
                    // NOTE: SQL fallback uses set semantics (NOT IN); true multiset
                    // diff requires ROW_NUMBER() partitioning. The runtime executor
                    // correctly honours multiset semantics.
                    "List.Difference" => {
                        let list1_sql = match args.first() {
                            Some(CallArg::StepRef(s)) => s.clone(),
                            Some(CallArg::Expr(e)) => match &e.expr {
                                Expr::List(items) => {
                                    let rows = items.iter()
                                        .map(|item| format!("SELECT {} AS \"Value\"", emit_expr(item)))
                                        .collect::<Vec<_>>()
                                        .join(" UNION ALL ");
                                    format!("({})", rows)
                                }
                                _ => emit_expr(e),
                            },
                            _ => String::new(),
                        };
                        let list2_sql = match args.get(1) {
                            Some(CallArg::StepRef(s)) => s.clone(),
                            Some(CallArg::Expr(e)) => match &e.expr {
                                Expr::List(items) => {
                                    let rows = items.iter()
                                        .map(|item| format!("SELECT {} AS \"Value\"", emit_expr(item)))
                                        .collect::<Vec<_>>()
                                        .join(" UNION ALL ");
                                    format!("({})", rows)
                                }
                                _ => emit_expr(e),
                            },
                            _ => String::new(),
                        };
                        if !list1_sql.is_empty() && !list2_sql.is_empty() {
                            let body = format!(
                                "    SELECT \"Value\" FROM {} t1 WHERE \"Value\" NOT IN (SELECT \"Value\" FROM {} t2)",
                                list1_sql, list2_sql
                            );
                            (body, vec![("Value".to_string(), ColumnType::Text)])
                        } else {
                            ("    SELECT NULL AS \"Value\"".to_string(), vec![("Value".to_string(), ColumnType::Text)])
                        }
                    }

                    // -- List.RemoveItems -----------------------------------------
                    "List.RemoveItems" => {
                        let list1_sql = match args.first() {
                            Some(CallArg::StepRef(s)) => s.clone(),
                            Some(CallArg::Expr(e)) => match &e.expr {
                                Expr::List(items) => {
                                    let rows = items.iter()
                                        .map(|item| format!("SELECT {} AS \"Value\"", emit_expr(item)))
                                        .collect::<Vec<_>>()
                                        .join(" UNION ALL ");
                                    format!("({})", rows)
                                }
                                _ => emit_expr(e),
                            },
                            _ => String::new(),
                        };
                        let list2_sql = match args.get(1) {
                            Some(CallArg::StepRef(s)) => s.clone(),
                            Some(CallArg::Expr(e)) => match &e.expr {
                                Expr::List(items) => {
                                    let rows = items.iter()
                                        .map(|item| format!("SELECT {} AS \"Value\"", emit_expr(item)))
                                        .collect::<Vec<_>>()
                                        .join(" UNION ALL ");
                                    format!("({})", rows)
                                }
                                _ => emit_expr(e),
                            },
                            _ => String::new(),
                        };
                        if !list1_sql.is_empty() && !list2_sql.is_empty() {
                            let body = format!(
                                "    SELECT \"Value\" FROM {} t1 WHERE \"Value\" NOT IN (SELECT \"Value\" FROM {} t2)",
                                list1_sql, list2_sql
                            );
                            (body, vec![("Value".to_string(), ColumnType::Text)])
                        } else {
                            ("    SELECT NULL AS \"Value\"".to_string(), vec![("Value".to_string(), ColumnType::Text)])
                        }
                    }

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

                    // â”€â”€ ListTransform â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
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

                    // â”€â”€ Construction stubs â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
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

                    // â”€â”€ Conversion passthrough â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.ToColumns" | "Table.ToList" | "Table.ToRecords" | "Table.ToRows"
                    | "Table.PartitionValues" | "Table.Profile" => {
                        (format!("    SELECT * FROM {}", input), schema.to_vec())
                    }

                    // â”€â”€ TableColumn â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    "Table.Column" => {
                        let col_name = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
                        let body = format!("    SELECT \"{}\" FROM {}", col_name, input);
                        (body, vec![(col_name.to_string(), ColumnType::Text)])
                    }

                    // â”€â”€ Default passthrough â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
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

// â”€â”€ expression â†’ SQL â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

fn emit_expr(node: &ExprNode) -> String {
    match &node.expr {
        // â”€â”€ literals â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
        Expr::IntLit(n)       => n.to_string(),
        Expr::FloatLit(n)     => n.to_string(),
        Expr::BoolLit(true)   => "TRUE".into(),
        Expr::BoolLit(false)  => "FALSE".into(),
        Expr::StringLit(s)    => format!("'{}'", s.replace('\'', "''")),
        Expr::NullLit         => "NULL".into(),

        // â”€â”€ column references â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
        Expr::Identifier(name)   => qi(name),
        Expr::ColumnAccess(name) => qi(name),
        // Field access `row[col]` â€” emit just the column name
        Expr::FieldAccess { field, .. } => qi(field),

        // â”€â”€ lambda â€” emit body (param ignored in SQL context) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
        // Covers both `each` (param == "_") and explicit lambdas.
        Expr::Lambda { body, .. } => emit_expr(body),

        // â”€â”€ binary ops â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
        Expr::BinaryOp { left, op, right } => {
            // M's `x = null` / `x <> null` are boolean tests, but standard
            // SQL's `x = NULL` is always NULL. Lower to `IS NULL` / `IS NOT NULL`.
            let l_is_null = matches!(left.expr,  Expr::NullLit);
            let r_is_null = matches!(right.expr, Expr::NullLit);
            match op {
                Operator::Eq if r_is_null => format!("({} IS NULL)",     emit_expr(left)),
                Operator::Eq if l_is_null => format!("({} IS NULL)",     emit_expr(right)),
                Operator::NotEq if r_is_null => format!("({} IS NOT NULL)", emit_expr(left)),
                Operator::NotEq if l_is_null => format!("({} IS NOT NULL)", emit_expr(right)),
                _ => format!(
                    "({} {} {})",
                    emit_expr(left),
                    op_sql(op),
                    emit_expr(right),
                ),
            }
        }

        // â”€â”€ unary ops â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
        Expr::UnaryOp { op, operand } => match op {
            UnaryOp::Not => format!("(NOT {})", emit_expr(operand)),
            UnaryOp::Neg => format!("(-{})",    emit_expr(operand)),
        },

        // â”€â”€ function call â€” best-effort SQL mapping â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
        Expr::FunctionCall { name, args } => {
            let args_sql = args.iter().map(emit_expr).collect::<Vec<_>>().join(", ");
            match name.as_str() {
                // â”€â”€ Text functions â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                "Text.Length"     => format!("LENGTH({})", args_sql),
                "Text.Upper"     => format!("UPPER({})",  args_sql),
                "Text.Lower"     => format!("LOWER({})",  args_sql),
                "Text.Trim"      => format!("TRIM({})",   args_sql),
                "Text.TrimStart" => format!("LTRIM({})",  args_sql),
                "Text.TrimEnd"   => format!("RTRIM({})",  args_sql),
                "Text.From"      => format!("CAST({} AS TEXT)", args_sql),
                "Text.Contains"  => {
                    let parts: Vec<String> = args.iter().map(emit_expr).collect();
                    if parts.len() >= 3 {
                        let case_insensitive = matches!(
                            &args[2],
                            pq_ast::expr::ExprNode { expr: pq_ast::expr::Expr::Identifier(n), .. }
                                if n == "Comparer.OrdinalIgnoreCase"
                        );
                        if case_insensitive {
                            format!("(POSITION(LOWER({1}) IN LOWER({0})) > 0)", parts[0], parts[1])
                        } else {
                            format!("(POSITION({1} IN {0}) > 0)", parts[0], parts[1])
                        }
                    } else if parts.len() == 2 {
                        format!("(POSITION({1} IN {0}) > 0)", parts[0], parts[1])
                    } else {
                        format!("Text_Contains({})", args_sql)
                    }
                },
                "Text.StartsWith" => {
                    let parts: Vec<String> = args.iter().map(emit_expr).collect();
                    if parts.len() >= 3 {
                        // Check if the 3rd arg names Comparer.OrdinalIgnoreCase.
                        let case_insensitive = matches!(
                            &args[2],
                            pq_ast::expr::ExprNode { expr: pq_ast::expr::Expr::Identifier(n), .. }
                                if n == "Comparer.OrdinalIgnoreCase"
                        );
                        if case_insensitive {
                            format!("(LOWER({}) LIKE LOWER({}) || '%')", parts[0], parts[1])
                        } else {
                            format!("({} LIKE {} || '%')", parts[0], parts[1])
                        }
                    } else if parts.len() == 2 {
                        format!("({} LIKE {} || '%')", parts[0], parts[1])
                    } else {
                        format!("Text_StartsWith({})", args_sql)
                    }
                },
                "Text.EndsWith" => {
                    let parts: Vec<String> = args.iter().map(emit_expr).collect();
                    if parts.len() >= 3 {
                        let case_insensitive = matches!(
                            &args[2],
                            pq_ast::expr::ExprNode { expr: pq_ast::expr::Expr::Identifier(n), .. }
                                if n == "Comparer.OrdinalIgnoreCase"
                        );
                        if case_insensitive {
                            format!("(LOWER({}) LIKE '%' || LOWER({}))", parts[0], parts[1])
                        } else {
                            format!("({} LIKE '%' || {})", parts[0], parts[1])
                        }
                    } else if parts.len() == 2 {
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

                // â”€â”€ Number functions â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
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

                // â”€â”€ Logical functions â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
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

                // â”€â”€ List aggregate functions â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
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

        // â”€â”€ collections â€” not directly expressible in scalar SQL â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
        Expr::List(items) => {
            let inner = items.iter().map(emit_expr).collect::<Vec<_>>().join(", ");
            format!("({})", inner)
        }
        Expr::Record(fields) => {
            // Emit as a JSON-like object â€” only meaningful in some SQL dialects.
            let inner = fields.iter()
                .map(|(k, v)| format!("'{}', {}", k, emit_expr(v)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("JSON_OBJECT({})", inner)
        }
    }
}

/// Emit an expression rewriting any reference to the implicit `_` parameter
/// to a named column. Used to lower list-context lambdas to SQL where the
/// per-element value lives in a synthetic column (`"Value"`).
fn emit_expr_with_underscore(node: &ExprNode, col: &str) -> String {
    match &node.expr {
        Expr::Identifier(name) | Expr::ColumnAccess(name) if name == "_" => {
            qi(col)
        }
        Expr::Lambda { body, .. } => emit_expr_with_underscore(body, col),
        Expr::BinaryOp { left, op, right } => {
            // Reuse null-aware lowering from emit_expr for `x = null` etc.
            let l_is_null = matches!(left.expr,  Expr::NullLit);
            let r_is_null = matches!(right.expr, Expr::NullLit);
            match op {
                Operator::Eq if r_is_null => format!("({} IS NULL)",     emit_expr_with_underscore(left, col)),
                Operator::Eq if l_is_null => format!("({} IS NULL)",     emit_expr_with_underscore(right, col)),
                Operator::NotEq if r_is_null => format!("({} IS NOT NULL)", emit_expr_with_underscore(left, col)),
                Operator::NotEq if l_is_null => format!("({} IS NOT NULL)", emit_expr_with_underscore(right, col)),
                _ => format!(
                    "({} {} {})",
                    emit_expr_with_underscore(left, col),
                    op_sql(op),
                    emit_expr_with_underscore(right, col),
                ),
            }
        }
        Expr::UnaryOp { op, operand } => match op {
            UnaryOp::Not => format!("(NOT {})", emit_expr_with_underscore(operand, col)),
            UnaryOp::Neg => format!("(-{})",    emit_expr_with_underscore(operand, col)),
        },
        Expr::FunctionCall { name, args } => {
            // Rebuild a temporary node with rewritten args and reuse emit_expr's
            // function-call lowering.
            let new_args: Vec<ExprNode> = args.iter()
                .map(|a| ExprNode {
                    expr: parse_underscore_replace(&a.expr, col),
                    span: a.span.clone(),
                    inferred_type: a.inferred_type.clone(),
                })
                .collect();
            let rebuilt = ExprNode {
                expr: Expr::FunctionCall { name: name.clone(), args: new_args },
                span: node.span.clone(),
                inferred_type: node.inferred_type.clone(),
            };
            emit_expr(&rebuilt)
        }
        _ => emit_expr(node),
    }
}

/// Recursively rewrite `_` references to `col` inside an Expr tree (for use
/// when constructing a new ExprNode whose lowering should treat `_` as `col`).
fn parse_underscore_replace(expr: &Expr, col: &str) -> Expr {
    match expr {
        Expr::Identifier(name) | Expr::ColumnAccess(name) if name == "_" => {
            Expr::ColumnAccess(col.to_string())
        }
        Expr::BinaryOp { left, op, right } => Expr::BinaryOp {
            left:  Box::new(ExprNode {
                expr: parse_underscore_replace(&left.expr, col),
                span: left.span.clone(),
                inferred_type: left.inferred_type.clone(),
            }),
            op:    op.clone(),
            right: Box::new(ExprNode {
                expr: parse_underscore_replace(&right.expr, col),
                span: right.span.clone(),
                inferred_type: right.inferred_type.clone(),
            }),
        },
        Expr::UnaryOp { op, operand } => Expr::UnaryOp {
            op: op.clone(),
            operand: Box::new(ExprNode {
                expr: parse_underscore_replace(&operand.expr, col),
                span: operand.span.clone(),
                inferred_type: operand.inferred_type.clone(),
            }),
        },
        Expr::FunctionCall { name, args } => Expr::FunctionCall {
            name: name.clone(),
            args: args.iter()
                .map(|a| ExprNode {
                    expr: parse_underscore_replace(&a.expr, col),
                    span: a.span.clone(),
                    inferred_type: a.inferred_type.clone(),
                })
                .collect(),
        },
        Expr::Lambda { params, body } => Expr::Lambda {
            params: params.clone(),
            body: Box::new(ExprNode {
                expr: parse_underscore_replace(&body.expr, col),
                span: body.span.clone(),
                inferred_type: body.inferred_type.clone(),
            }),
        },
        other => other.clone(),
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

// â”€â”€ type inference for AddColumn schema tracking â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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

        // Field access `row[col]` â€” look up the field name in the schema
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

// â”€â”€ small helpers â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

fn qi(name: &str) -> String {
    format!("\"{}\"", name)
}

/// Returns `true` when a column with type `col_ty` should be included for a
/// `ColumnsOfType` filter element `filter`.
///
/// Mirrors Power Query semantics: `type number` matches Integer, Float, Currency.
fn sql_type_matches(col_ty: &ColumnType, filter: &ColumnType) -> bool {
    match filter {
        ColumnType::Float => matches!(col_ty, ColumnType::Float | ColumnType::Integer | ColumnType::Currency),
        _ => col_ty == filter,
    }
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


// â”€â”€ tests â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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

