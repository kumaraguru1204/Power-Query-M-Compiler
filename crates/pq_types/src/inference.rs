use crate::ColumnType;

/// Infer the type of a column from its raw string values.
///
/// Rules (tried in order):
///   1. all values parse as i64  → Integer
///   2. all values parse as f64  → Float
///   3. all values are true/false → Boolean
///   4. anything else             → Text
///
/// Empty columns default to Text.
pub fn infer_type(values: &[String]) -> ColumnType {
    if values.is_empty() {
        return ColumnType::Text;
    }

    if values.iter().all(|v| v.parse::<i64>().is_ok()) {
        return ColumnType::Integer;
    }

    if values.iter().all(|v| v.parse::<f64>().is_ok()) {
        return ColumnType::Float;
    }

    if values
        .iter()
        .all(|v| matches!(v.to_lowercase().as_str(), "true" | "false"))
    {
        return ColumnType::Boolean;
    }

    ColumnType::Text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_integer() {
        let v = vec!["1".into(), "2".into(), "3".into()];
        assert_eq!(infer_type(&v), ColumnType::Integer);
    }

    #[test]
    fn test_float() {
        let v = vec!["1.1".into(), "2.2".into(), "3.3".into()];
        assert_eq!(infer_type(&v), ColumnType::Float);
    }

    #[test]
    fn test_boolean() {
        let v = vec!["true".into(), "false".into(), "true".into()];
        assert_eq!(infer_type(&v), ColumnType::Boolean);
    }

    #[test]
    fn test_text() {
        let v = vec!["Alice".into(), "Bob".into()];
        assert_eq!(infer_type(&v), ColumnType::Text);
    }

    #[test]
    fn test_mixed_falls_to_text() {
        let v = vec!["1".into(), "hello".into()];
        assert_eq!(infer_type(&v), ColumnType::Text);
    }

    #[test]
    fn test_empty() {
        assert_eq!(infer_type(&[]), ColumnType::Text);
    }
}