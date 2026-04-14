use pq_diagnostics::{Diagnostic, Span};
use pq_lexer::token::TokenKind;

/// Every error the parser can produce.
/// Each variant carries a Span so we know
/// exactly where in the source the error occurred.
#[derive(Debug)]
pub enum ParseError {
    /// Got a token we did not expect.
    UnexpectedToken {
        expected: String,
        got:      TokenKind,
        span:     Span,
    },

    /// Ran out of tokens too early.
    UnexpectedEof {
        expected: String,
    },

    /// An unknown M type string like "Foo.Type".
    UnknownType {
        type_str: String,
        span:     Span,
    },

    /// A function call we do not recognize.
    UnknownFunction {
        qualified: String,
        span:      Span,
    },

    /// A sort order we do not recognize.
    UnknownSortOrder {
        got:  String,
        span: Span,
    },
}

impl ParseError {
    /// Convert this error into a Diagnostic for reporting.
    pub fn to_diagnostic(&self) -> Diagnostic {
        match self {
            ParseError::UnexpectedToken { expected, got, span } => {
                Diagnostic::error(
                    "E201",
                    format!("expected {} but got '{}'", expected, got),
                )
                    .with_label(span.clone(), "unexpected token here")
            }

            ParseError::UnexpectedEof { expected } => {
                Diagnostic::error(
                    "E202",
                    format!("unexpected end of input, expected {}", expected),
                )
            }

            ParseError::UnknownType { type_str, span } => {
                Diagnostic::error(
                    "E203",
                    format!("unknown type '{}'", type_str),
                )
                    .with_label(span.clone(), "unknown type here")
                    .with_suggestion(
                        "valid types are: Int64.Type, Number.Type, Text.Type, Logical.Type, Date.Type"
                    )
            }

            ParseError::UnknownFunction { qualified, span } => {
                Diagnostic::error(
                    "E204",
                    format!("unknown function '{}'", qualified),
                )
                    .with_label(span.clone(), "unknown function here")
                    .with_suggestion(
                        "valid functions: Excel.Workbook, Table.PromoteHeaders, \
                     Table.TransformColumnTypes, Table.SelectRows, Table.AddColumn, \
                     Table.RemoveColumns, Table.RenameColumns, Table.Sort"
                    )
            }

            ParseError::UnknownSortOrder { got, span } => {
                Diagnostic::error(
                    "E205",
                    format!("unknown sort order '{}'", got),
                )
                    .with_label(span.clone(), "unknown sort order here")
                    .with_suggestion(
                        "valid sort orders: Order.Ascending, Order.Descending"
                    )
            }
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ParseError::UnexpectedToken { expected, got, .. } =>
                write!(f, "expected {} but got '{}'", expected, got),
            ParseError::UnexpectedEof { expected } =>
                write!(f, "unexpected end of input, expected {}", expected),
            ParseError::UnknownType { type_str, .. } =>
                write!(f, "unknown type '{}'", type_str),
            ParseError::UnknownFunction { qualified, .. } =>
                write!(f, "unknown function '{}'", qualified),
            ParseError::UnknownSortOrder { got, .. } =>
                write!(f, "unknown sort order '{}'", got),
        }
    }
}

pub type ParseResult<T> = Result<T, ParseError>;