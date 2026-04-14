/// Every data type our engine understands.
/// Maps directly to M language types.
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnType {
    Integer,      // Int64.Type
    Float,        // Number.Type
    Boolean,      // Logical.Type
    Text,         // Text.Type
    Date,         // Date.Type
    DateTime,     // DateTime.Type
    DateTimeZone, // DateTimeZone.Type
    Duration,     // Duration.Type
    Time,         // Time.Type
    Currency,     // Currency.Type
    Binary,       // Binary.Type
    Null,         // no value
    /// A function (lambda) value.
    /// The inner type is the return type produced by the body.
    /// For `each` lambdas the implicit parameter is the current row record.
    Function(Box<ColumnType>),
    /// A homogeneous list value, e.g. `{1, 2, 3}` → `List<Integer>`.
    /// The inner type is the element type.
    List(Box<ColumnType>),
}

impl ColumnType {
    /// Convert to M language type string.
    pub fn to_m_type(&self) -> &str {
        match self {
            ColumnType::Integer      => "Int64.Type",
            ColumnType::Float        => "Number.Type",
            ColumnType::Boolean      => "Logical.Type",
            ColumnType::Text         => "Text.Type",
            ColumnType::Date         => "Date.Type",
            ColumnType::DateTime     => "DateTime.Type",
            ColumnType::DateTimeZone => "DateTimeZone.Type",
            ColumnType::Duration     => "Duration.Type",
            ColumnType::Time         => "Time.Type",
            ColumnType::Currency     => "Currency.Type",
            ColumnType::Binary       => "Binary.Type",
            ColumnType::Null         => "Null.Type",
            ColumnType::Function(_)  => "Function.Type",
            ColumnType::List(_)      => "List.Type",
        }
    }

    /// Parse from M language type string.
    pub fn from_m_type(s: &str) -> Option<ColumnType> {
        match s {
            "Int64.Type"        => Some(ColumnType::Integer),
            "Number.Type"       => Some(ColumnType::Float),
            "Logical.Type"      => Some(ColumnType::Boolean),
            "Text.Type"         => Some(ColumnType::Text),
            "Date.Type"         => Some(ColumnType::Date),
            "DateTime.Type"     => Some(ColumnType::DateTime),
            "Currency.Type"     => Some(ColumnType::Currency),
            "DateTimeZone.Type" => Some(ColumnType::DateTimeZone),
            "Duration.Type"     => Some(ColumnType::Duration),
            "Time.Type"         => Some(ColumnType::Time),
            "Any.Type"          => Some(ColumnType::Text),
            "Binary.Type"       => Some(ColumnType::Binary),
            "Null.Type"         => Some(ColumnType::Null),
            _                   => None,
        }
    }

    /// Is this type numeric?
    /// Used by type checker to validate arithmetic expressions.
    pub fn is_numeric(&self) -> bool {
        matches!(self, ColumnType::Integer | ColumnType::Float)
    }

    /// Is this type comparable?
    /// All scalar types except Null can be compared; functions and lists cannot.
    pub fn is_comparable(&self) -> bool {
        !matches!(self, ColumnType::Null | ColumnType::Function(_) | ColumnType::List(_) | ColumnType::Binary)
    }
}

impl std::fmt::Display for ColumnType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ColumnType::List(inner)    => write!(f, "List<{}>", inner),
            ColumnType::Function(ret)  => write!(f, "Function<{}>", ret),
            other                      => write!(f, "{}", other.to_m_type()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let types = vec![
            ColumnType::Integer,
            ColumnType::Float,
            ColumnType::Boolean,
            ColumnType::Text,
        ];
        for t in types {
            let m = t.to_m_type();
            let back = ColumnType::from_m_type(m).unwrap();
            assert_eq!(t, back);
        }
    }

    #[test]
    fn test_numeric() {
        assert!(ColumnType::Integer.is_numeric());
        assert!(ColumnType::Float.is_numeric());
        assert!(!ColumnType::Text.is_numeric());
        assert!(!ColumnType::Boolean.is_numeric());
        assert!(!ColumnType::DateTime.is_numeric());
        assert!(!ColumnType::Currency.is_numeric());
        assert!(!ColumnType::Binary.is_numeric());
    }

    #[test]
    fn test_new_variants_roundtrip() {
        let types = vec![
            ColumnType::DateTime,
            ColumnType::Currency,
            ColumnType::DateTimeZone,
            ColumnType::Duration,
            ColumnType::Time,
            ColumnType::Binary,
        ];
        for t in types {
            let m = t.to_m_type();
            let back = ColumnType::from_m_type(m).unwrap();
            assert_eq!(t, back);
        }
    }

    #[test]
    fn test_comparable() {
        assert!(ColumnType::DateTime.is_comparable());
        assert!(ColumnType::Currency.is_comparable());
        assert!(ColumnType::Time.is_comparable());
        assert!(!ColumnType::Binary.is_comparable());
    }

    #[test]
    fn test_any_type_maps_to_text() {
        assert_eq!(ColumnType::from_m_type("Any.Type"), Some(ColumnType::Text));
    }

    #[test]
    fn test_null_type_maps_to_null() {
        assert_eq!(ColumnType::from_m_type("Null.Type"), Some(ColumnType::Null));
    }
}