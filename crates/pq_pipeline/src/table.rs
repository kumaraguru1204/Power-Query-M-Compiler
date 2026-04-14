use pq_types::ColumnType;
use serde::Deserialize;

/// Raw JSON input from the UI.
/// Every value is a plain string — no types assigned yet.
/// This is exactly what arrives from the frontend.
#[derive(Debug, Deserialize)]
pub struct RawWorkbook {
    pub source: String,
    pub sheet:  String,
    pub rows:   Vec<Vec<String>>,
}

/// A single typed column.
/// After build_table() runs, every column has
/// a name, an inferred type, and its raw string values.
#[derive(Debug, Clone)]
pub struct Column {
    /// column header name
    pub name:     String,

    /// inferred data type
    pub col_type: ColumnType,

    /// raw string values (one per row)
    pub values:   Vec<String>,
}

/// The core data model of the engine.
/// Everything the engine does operates on a Table.
#[derive(Debug, Clone)]
pub struct Table {
    /// original file name
    pub source:  String,

    /// sheet name
    pub sheet:   String,

    /// columns in order
    pub columns: Vec<Column>,
}

impl Table {
    /// Find a column by name.
    /// Used by the resolver and type checker.
    pub fn get_column(&self, name: &str) -> Option<&Column> {
        self.columns.iter().find(|c| c.name == name)
    }

    /// All column names in order.
    pub fn column_names(&self) -> Vec<&str> {
        self.columns.iter().map(|c| c.name.as_str()).collect()
    }

    /// Does a column with this name exist?
    pub fn has_column(&self, name: &str) -> bool {
        self.columns.iter().any(|c| c.name == name)
    }

    /// Number of rows in the table.
    pub fn row_count(&self) -> usize {
        self.columns
            .first()
            .map(|c| c.values.len())
            .unwrap_or(0)
    }

    /// Number of columns in the table.
    pub fn col_count(&self) -> usize {
        self.columns.len()
    }
}

impl std::fmt::Display for Table {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(f, "Table: {} / {}", self.source, self.sheet)?;
        writeln!(f, "  {} rows x {} columns", self.row_count(), self.col_count())?;
        for col in &self.columns {
            writeln!(f, "  {:<16} {:?}", col.name, col.col_type)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pq_types::ColumnType;

    fn make_table() -> Table {
        Table {
            source:  "test.xlsx".into(),
            sheet:   "Sheet1".into(),
            columns: vec![
                Column {
                    name:     "Name".into(),
                    col_type: ColumnType::Text,
                    values:   vec!["Alice".into(), "Bob".into()],
                },
                Column {
                    name:     "Age".into(),
                    col_type: ColumnType::Integer,
                    values:   vec!["30".into(), "25".into()],
                },
            ],
        }
    }

    #[test]
    fn test_get_column() {
        let t = make_table();
        assert!(t.get_column("Name").is_some());
        assert!(t.get_column("Missing").is_none());
    }

    #[test]
    fn test_has_column() {
        let t = make_table();
        assert!(t.has_column("Age"));
        assert!(!t.has_column("Salary"));
    }

    #[test]
    fn test_row_count() {
        let t = make_table();
        assert_eq!(t.row_count(), 2);
    }

    #[test]
    fn test_col_count() {
        let t = make_table();
        assert_eq!(t.col_count(), 2);
    }

    #[test]
    fn test_column_names() {
        let t = make_table();
        assert_eq!(t.column_names(), vec!["Name", "Age"]);
    }
}