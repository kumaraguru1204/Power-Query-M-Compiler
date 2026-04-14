use pq_diagnostics::{Diagnostic, Span};
use pq_types::ColumnType;

/// Every type error the checker can produce.
#[derive(Debug)]
pub enum TypeError {
    /// Left and right sides of a binary op have incompatible types.
    TypeMismatch {
        left:  ColumnType,
        right: ColumnType,
        span:  Span,
    },

    /// Arithmetic operator used on a non-numeric type.
    ArithmeticOnNonNumeric {
        col_type: ColumnType,
        span:     Span,
    },

    /// Comparison operator used on incomparable types.
    ComparisonOnIncomparable {
        col_type: ColumnType,
        span:     Span,
    },
}

impl TypeError {
    pub fn to_diagnostic(&self) -> Diagnostic {
        match self {
            TypeError::TypeMismatch { left, right, span } => {
                Diagnostic::error(
                    "E401",
                    format!(
                        "type mismatch: cannot use '{}' and '{}' together",
                        left, right
                    ),
                )
                    .with_label(span.clone(), "type mismatch here")
                    .with_suggestion(
                        format!(
                            "both sides must be the same type, \
                         or one Integer and one Float"
                        )
                    )
            }

            TypeError::ArithmeticOnNonNumeric { col_type, span } => {
                Diagnostic::error(
                    "E402",
                    format!(
                        "arithmetic not allowed on type '{}'",
                        col_type
                    ),
                )
                    .with_label(span.clone(), "non-numeric type used here")
                    .with_suggestion(
                        "arithmetic operators (+, -, *, /) \
                     require Integer or Float columns"
                    )
            }

            TypeError::ComparisonOnIncomparable { col_type, span } => {
                Diagnostic::error(
                    "E403",
                    format!(
                        "cannot compare type '{}'",
                        col_type
                    ),
                )
                    .with_label(span.clone(), "incomparable type used here")
            }
        }
    }
}

impl std::fmt::Display for TypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            TypeError::TypeMismatch { left, right, .. } =>
                write!(f, "type mismatch: {} vs {}", left, right),
            TypeError::ArithmeticOnNonNumeric { col_type, .. } =>
                write!(f, "arithmetic not allowed on {}", col_type),
            TypeError::ComparisonOnIncomparable { col_type, .. } =>
                write!(f, "cannot compare {}", col_type),
        }
    }
}