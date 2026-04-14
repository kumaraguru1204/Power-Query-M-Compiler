use pq_types::ColumnType;

/// A single column in the evolving schema.
#[derive(Debug, Clone)]
pub struct SchemaColumn {
    pub name:     String,
    pub col_type: ColumnType,
}

/// The schema at a given point in the pipeline.
/// Starts as the original table schema.
/// Each step updates it to reflect what that step produces.
///
/// This is what we validate against — not the original table.
/// Because after RemoveColumns, the removed column is gone.
/// After RenameColumns, the old name is gone.
/// After AddColumn, the new column exists.
#[derive(Debug, Clone)]
pub struct Schema {
    pub columns: Vec<SchemaColumn>,
}

impl Schema {
    /// Build initial schema from table columns.
    pub fn from_columns(columns: &[(String, ColumnType)]) -> Self {
        Schema {
            columns: columns
                .iter()
                .map(|(name, t)| SchemaColumn {
                    name:     name.clone(),
                    col_type: t.clone(),
                })
                .collect(),
        }
    }

    /// Does a column with this name exist?
    pub fn has_column(&self, name: &str) -> bool {
        self.columns.iter().any(|c| c.name == name)
    }

    /// Get a column by name.
    pub fn get_column(&self, name: &str) -> Option<&SchemaColumn> {
        self.columns.iter().find(|c| c.name == name)
    }

    /// All column names.
    pub fn column_names(&self) -> Vec<&str> {
        self.columns.iter().map(|c| c.name.as_str()).collect()
    }

    /// Apply AddColumn — adds a new column to the schema.
    pub fn add_column(&mut self, name: String, col_type: ColumnType) {
        self.columns.push(SchemaColumn { name, col_type });
    }

    /// Apply RemoveColumns — removes columns from the schema.
    pub fn remove_columns(&mut self, names: &[String]) {
        self.columns.retain(|c| !names.contains(&c.name));
    }

    /// Apply RenameColumns — renames columns in the schema.
    pub fn rename_columns(&mut self, renames: &[(String, String)]) {
        for col in self.columns.iter_mut() {
            if let Some((_, new)) = renames.iter().find(|(old, _)| old == &col.name) {
                col.name = new.clone();
            }
        }
    }

    /// Apply ChangeTypes — updates column types in the schema.
    pub fn change_types(&mut self, changes: &[(String, ColumnType)]) {
        for col in self.columns.iter_mut() {
            if let Some((_, new_type)) = changes.iter().find(|(name, _)| name == &col.name) {
                col.col_type = new_type.clone();
            }
        }
    }

    /// Find the closest column name for "did you mean?" suggestions.
    pub fn closest_match(&self, name: &str) -> Option<&str> {
        self.columns
            .iter()
            .filter_map(|c| {
                let dist = edit_distance(&c.name, name);
                if dist <= 3 { Some((dist, c.name.as_str())) } else { None }
            })
            .min_by_key(|(dist, _)| *dist)
            .map(|(_, name)| name)
    }
}

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let m = a.len();
    let n = b.len();
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for i in 0..=m { dp[i][0] = i; }
    for j in 0..=n { dp[0][j] = j; }
    for i in 1..=m {
        for j in 1..=n {
            dp[i][j] = if a[i-1] == b[j-1] {
                dp[i-1][j-1]
            } else {
                1 + dp[i-1][j].min(dp[i][j-1]).min(dp[i-1][j-1])
            };
        }
    }
    dp[m][n]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_schema() -> Schema {
        Schema::from_columns(&[
            ("Name".into(),   ColumnType::Text),
            ("Age".into(),    ColumnType::Integer),
            ("Salary".into(), ColumnType::Float),
        ])
    }

    #[test]
    fn test_has_column() {
        let s = make_schema();
        assert!(s.has_column("Age"));
        assert!(!s.has_column("Missing"));
    }

    #[test]
    fn test_add_column() {
        let mut s = make_schema();
        s.add_column("Bonus".into(), ColumnType::Float);
        assert!(s.has_column("Bonus"));
        assert_eq!(s.columns.len(), 4);
    }

    #[test]
    fn test_remove_columns() {
        let mut s = make_schema();
        s.remove_columns(&["Age".into()]);
        assert!(!s.has_column("Age"));
        assert_eq!(s.columns.len(), 2);
    }

    #[test]
    fn test_rename_columns() {
        let mut s = make_schema();
        s.rename_columns(&[("Name".into(), "FullName".into())]);
        assert!(!s.has_column("Name"));
        assert!(s.has_column("FullName"));
    }

    #[test]
    fn test_change_types() {
        let mut s = make_schema();
        s.change_types(&[("Age".into(), ColumnType::Float)]);
        assert_eq!(
            s.get_column("Age").unwrap().col_type,
            ColumnType::Float
        );
    }

    #[test]
    fn test_closest_match() {
        let s = make_schema();
        assert_eq!(s.closest_match("Nane"), Some("Name"));
    }
}