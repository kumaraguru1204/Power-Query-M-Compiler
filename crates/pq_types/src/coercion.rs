use crate::ColumnType;

/// The result of trying to coerce two types together.
#[derive(Debug, Clone, PartialEq)]
pub enum CoercionResult {
    /// Both types are the same — no coercion needed.
    Same(ColumnType),

    /// Types are different but compatible.
    /// The result type is what the expression evaluates to.
    /// Example: Integer + Float → Float
    Coerced(ColumnType),

    /// Types are incompatible — this is a type error.
    Incompatible,
}

/// Try to coerce two types together for a binary operation.
///
/// Rules:
///   Integer op Integer  → Same(Integer)
///   Float   op Float    → Same(Float)
///   Integer op Float    → Coerced(Float)   (integer widens to float)
///   Float   op Integer  → Coerced(Float)
///   Text    op Text     → Same(Text)        (only for = and <>)
///   Boolean op Boolean  → Same(Boolean)     (only for = and <>)
///   anything else       → Incompatible
pub fn coerce_types(left: &ColumnType, right: &ColumnType) -> CoercionResult {
    match (left, right) {
        // exact match
        (l, r) if l == r => CoercionResult::Same(left.clone()),

        // integer + float → float (numeric widening)
        (ColumnType::Integer, ColumnType::Float)
        | (ColumnType::Float,  ColumnType::Integer) => {
            CoercionResult::Coerced(ColumnType::Float)
        }

        // everything else is incompatible
        _ => CoercionResult::Incompatible,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_same_types() {
        assert_eq!(
            coerce_types(&ColumnType::Integer, &ColumnType::Integer),
            CoercionResult::Same(ColumnType::Integer)
        );
    }

    #[test]
    fn test_int_float_widens() {
        assert_eq!(
            coerce_types(&ColumnType::Integer, &ColumnType::Float),
            CoercionResult::Coerced(ColumnType::Float)
        );
        assert_eq!(
            coerce_types(&ColumnType::Float, &ColumnType::Integer),
            CoercionResult::Coerced(ColumnType::Float)
        );
    }

    #[test]
    fn test_incompatible() {
        assert_eq!(
            coerce_types(&ColumnType::Text, &ColumnType::Integer),
            CoercionResult::Incompatible
        );
    }
}