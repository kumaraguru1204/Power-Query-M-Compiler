use crate::diagnostic::{Diagnostic, DiagnosticKind};

/// Renders diagnostics to the terminal in a human readable format.
///
/// Output style:
///
///   error[E001]: unknown step reference
///     --> formula:4:37
///      |
///    4 |     WithBonus = Table.AddColumn(ChangedTypes, ...)
///      |                                ^^^^^^^^^^^^ step 'ChangedTypes' does not exist
///      = help: did you mean 'PromotedHeaders'?
pub struct Reporter<'a> {
    /// The original source formula.
    /// Used to extract the relevant line for display.
    source: &'a str,
}

impl<'a> Reporter<'a> {
    pub fn new(source: &'a str) -> Self {
        Reporter { source }
    }

    /// Render a single diagnostic to a String.
    pub fn render(&self, diagnostic: &Diagnostic) -> String {
        let mut out = String::new();

        // ── header line ───────────────────────────────────────────────────
        // error[E001]: unknown step reference
        out.push_str(&format!(
            "{}[{}]: {}\n",
            diagnostic.kind,
            diagnostic.code,
            diagnostic.message
        ));

        // ── labels ────────────────────────────────────────────────────────
        for label in &diagnostic.labels {
            let span = &label.span;

            if span.is_dummy() {
                continue;
            }

            // --> formula:line:col
            out.push_str(&format!(
                "  --> formula:{}:{}\n",
                span.line, span.col
            ));

            // extract the source line
            let source_line = self
                .source
                .lines()
                .nth(span.line.saturating_sub(1))
                .unwrap_or("");

            let line_num = span.line.to_string();
            let padding  = " ".repeat(line_num.len());

            // blank line with pipe
            out.push_str(&format!("{}  |\n", padding));

            // The source line, with the error span highlighted in bold red.
            // Splitting into before/highlight/after lets the color survive
            // terminal line-wrap — the red characters are always at the right
            // place regardless of how the terminal wraps the line.
            let col_offset = span.col.saturating_sub(1);
            let span_chars = span.len().max(1);
            let chars: Vec<char> = source_line.chars().collect();
            let hl_start = col_offset.min(chars.len());
            let hl_end   = (col_offset + span_chars).min(chars.len());
            let before:    String = chars[..hl_start].iter().collect();
            let highlight: String = chars[hl_start..hl_end].iter().collect();
            let after:     String = chars[hl_end..].iter().collect();

            out.push_str(&format!(
                "{}  | {}\x1b[1;31m{}\x1b[0m{}\n",
                line_num, before, highlight, after
            ));

            // the underline carets (kept as-is)
            let underline  = "^".repeat(span_chars);
            out.push_str(&format!(
                "{}  | {}\x1b[1;31m{}\x1b[0m {}\n",
                padding,
                " ".repeat(col_offset),
                underline,
                label.message
            ));

            // blank line with pipe
            out.push_str(&format!("{}  |\n", padding));
        }

        // ── suggestion ────────────────────────────────────────────────────
        if let Some(suggestion) = &diagnostic.suggestion {
            out.push_str(&format!("  = help: {}\n", suggestion));
        }

        out
    }

    /// Render all diagnostics and return as one string.
    pub fn render_all(&self, diagnostics: &[Diagnostic]) -> String {
        diagnostics
            .iter()
            .map(|d| self.render(d))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Print all diagnostics to stderr.
    pub fn emit_all(&self, diagnostics: &[Diagnostic]) {
        for d in diagnostics {
            eprintln!("{}", self.render(d));
        }
    }

    /// Returns true if any diagnostic is an error.
    pub fn has_errors(diagnostics: &[Diagnostic]) -> bool {
        diagnostics
            .iter()
            .any(|d| d.kind == DiagnosticKind::Error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Diagnostic, Span};

    #[test]
    fn test_render_error() {
        let source = "let\n    WithBonus = Table.AddColumn(ChangedTypes, \"Bonus\", each Salary + 1000.0)\nin\n    WithBonus";
        let reporter = Reporter::new(source);

        let d = Diagnostic::error("E001", "unknown step reference")
            .with_label(Span::new(36, 48, 2, 33), "step 'ChangedTypes' does not exist")
            .with_suggestion("did you mean 'Source'?");

        let rendered = reporter.render(&d);
        assert!(rendered.contains("error[E001]"));
        assert!(rendered.contains("formula:2:33"));
        assert!(rendered.contains("help:"));
    }

    #[test]
    fn test_has_errors() {
        let diagnostics = vec![
            Diagnostic::warning("W001", "unused step"),
            Diagnostic::error("E001", "unknown step"),
        ];
        assert!(Reporter::has_errors(&diagnostics));
    }

    #[test]
    fn test_no_errors() {
        let diagnostics = vec![
            Diagnostic::warning("W001", "unused step"),
        ];
        assert!(!Reporter::has_errors(&diagnostics));
    }
}