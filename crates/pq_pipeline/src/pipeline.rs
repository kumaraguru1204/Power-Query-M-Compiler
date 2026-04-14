use pq_types::{ColumnType, infer_type};
use crate::table::{Column, RawWorkbook, Table};

/// Convert a RawWorkbook into a typed Table.
///
/// Three things happen here:
///   1. first row is pulled out as column headers
///   2. remaining rows fill each column's values
///   3. type inference runs on each column
///
/// This is the M equivalent of:
///   Source          = Excel.Workbook(File.Contents("file.xlsx"), null, true)
///   PromotedHeaders = Table.PromoteHeaders(Source)
///   ChangedTypes    = Table.TransformColumnTypes(PromotedHeaders, ...)
pub fn build_table(wb: RawWorkbook) -> Table {
    let mut rows = wb.rows.into_iter();

    // ── step 1: pull headers ──────────────────────────────────────────────
    let headers = match rows.next() {
        Some(h) => h,
        None    => return Table {
            source:  wb.source,
            sheet:   wb.sheet,
            columns: vec![],
        },
    };

    // ── step 2: create one empty column per header ────────────────────────
    let mut columns: Vec<Column> = headers
        .into_iter()
        .map(|name| Column {
            name,
            col_type: ColumnType::Text, // placeholder, overwritten below
            values:   vec![],
        })
        .collect();

    // ── step 3: fill column values row by row ─────────────────────────────
    for row in rows {
        for (i, val) in row.into_iter().enumerate() {
            if let Some(col) = columns.get_mut(i) {
                col.values.push(val);
            }
            // if a row has more values than headers we silently drop them
            // this matches Excel's behavior
        }
    }

    // ── step 4: infer type per column ─────────────────────────────────────
    for col in columns.iter_mut() {
        col.col_type = infer_type(&col.values);
    }

    Table {
        source:  wb.source,
        sheet:   wb.sheet,
        columns,
    }
}

/// Parse a JSON string directly into a Table.
/// Convenience wrapper used by the engine.
pub fn build_table_from_json(json: &str) -> Result<Table, serde_json::Error> {
    let wb: RawWorkbook = serde_json::from_str(json)?;
    Ok(build_table(wb))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pq_types::ColumnType;

    fn make_workbook() -> RawWorkbook {
        RawWorkbook {
            source: "test.xlsx".into(),
            sheet:  "Sheet1".into(),
            rows:   vec![
                vec!["Name".into(),    "Age".into(),  "Salary".into(),    "Active".into()],
                vec!["Alice".into(),   "30".into(),   "50000.50".into(),  "true".into()],
                vec!["Bob".into(),     "25".into(),   "40000.00".into(),  "false".into()],
                vec!["Charlie".into(), "35".into(),   "60000.75".into(),  "true".into()],
            ],
        }
    }

    #[test]
    fn test_column_names() {
        let t = build_table(make_workbook());
        assert_eq!(t.column_names(), vec!["Name", "Age", "Salary", "Active"]);
    }

    #[test]
    fn test_type_inference() {
        let t = build_table(make_workbook());
        assert_eq!(t.get_column("Name").unwrap().col_type,   ColumnType::Text);
        assert_eq!(t.get_column("Age").unwrap().col_type,    ColumnType::Integer);
        assert_eq!(t.get_column("Salary").unwrap().col_type, ColumnType::Float);
        assert_eq!(t.get_column("Active").unwrap().col_type, ColumnType::Boolean);
    }

    #[test]
    fn test_row_values() {
        let t = build_table(make_workbook());
        let age = t.get_column("Age").unwrap();
        assert_eq!(age.values, vec!["30", "25", "35"]);
    }

    #[test]
    fn test_empty_workbook() {
        let wb = RawWorkbook {
            source: "empty.xlsx".into(),
            sheet:  "Sheet1".into(),
            rows:   vec![],
        };
        let t = build_table(wb);
        assert_eq!(t.col_count(), 0);
        assert_eq!(t.row_count(), 0);
    }

    #[test]
    fn test_headers_only() {
        let wb = RawWorkbook {
            source: "test.xlsx".into(),
            sheet:  "Sheet1".into(),
            rows:   vec![
                vec!["Name".into(), "Age".into()],
            ],
        };
        let t = build_table(wb);
        assert_eq!(t.col_count(), 2);
        assert_eq!(t.row_count(), 0);
    }

    #[test]
    fn test_from_json() {
        let json = r#"
        {
            "source": "workbook.xlsx",
            "sheet":  "Sales",
            "rows": [
                ["Name",  "Age"],
                ["Alice", "30"],
                ["Bob",   "25"]
            ]
        }
        "#;
        let t = build_table_from_json(json).unwrap();
        assert_eq!(t.source, "workbook.xlsx");
        assert_eq!(t.col_count(), 2);
        assert_eq!(t.row_count(), 2);
    }
}