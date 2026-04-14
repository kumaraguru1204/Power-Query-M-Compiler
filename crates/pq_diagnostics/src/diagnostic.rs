use crate::Span;

/// Severity of a diagnostic.
#[derive(Debug, Clone, PartialEq)]
pub enum DiagnosticKind {
    /// Hard error — compilation stops.
    Error,

    /// Warning — compilation continues but user should fix.
    Warning,

    /// Informational hint.
    Hint,
}

impl std::fmt::Display for DiagnosticKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            DiagnosticKind::Error   => write!(f, "error"),
            DiagnosticKind::Warning => write!(f, "warning"),
            DiagnosticKind::Hint    => write!(f, "hint"),
        }
    }
}

/// A label points to a specific span in the source
/// and adds a message to it.
/// Like the red/blue underlines in compiler error output.
#[derive(Debug, Clone)]
pub struct Label {
    pub span:    Span,
    pub message: String,
}

impl Label {
    pub fn new(span: Span, message: impl Into<String>) -> Self {
        Label { span, message: message.into() }
    }
}

/// A single diagnostic message.
///
/// Example output:
///
///   error[E001]: unknown step reference
///     --> formula:4:37
///      |
///    4 |     WithBonus = Table.AddColumn(ChangedTypes, ...)
///      |                                ^^^^^^^^^^^^ step 'ChangedTypes' does not exist
///      |
///      = help: did you mean 'PromotedHeaders'?
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// Error / Warning / Hint
    pub kind: DiagnosticKind,

    /// Short error code like "E001"
    pub code: &'static str,

    /// Main message — what went wrong
    pub message: String,

    /// Labels pointing to locations in the source
    pub labels: Vec<Label>,

    /// Optional suggestion for how to fix it
    pub suggestion: Option<String>,
}

impl Diagnostic {
    pub fn error(code: &'static str, message: impl Into<String>) -> Self {
        Diagnostic {
            kind:       DiagnosticKind::Error,
            code,
            message:    message.into(),
            labels:     vec![],
            suggestion: None,
        }
    }

    pub fn warning(code: &'static str, message: impl Into<String>) -> Self {
        Diagnostic {
            kind:       DiagnosticKind::Warning,
            code,
            message:    message.into(),
            labels:     vec![],
            suggestion: None,
        }
    }

    /// Add a label pointing to a location in the source.
    pub fn with_label(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push(Label::new(span, message));
        self
    }

    /// Add a suggestion for how to fix the error.
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}[{}]: {}", self.kind, self.code, self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_builder() {
        let d = Diagnostic::error("E001", "unknown step")
            .with_label(Span::new(0, 5, 1, 1), "used here")
            .with_suggestion("did you mean 'Source'?");

        assert_eq!(d.kind, DiagnosticKind::Error);
        assert_eq!(d.code, "E001");
        assert_eq!(d.labels.len(), 1);
        assert!(d.suggestion.is_some());
    }

    #[test]
    fn test_display() {
        let d = Diagnostic::error("E001", "unknown step");
        assert_eq!(format!("{}", d), "error[E001]: unknown step");
    }
}