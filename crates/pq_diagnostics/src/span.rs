/// A span points to an exact location in the source formula.
/// Every token and every AST node carries a span.
/// This is what lets us say:
///   "error at line 3, column 12"
///   instead of just
///   "error somewhere"
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Span {
    /// byte offset from start of input where this span begins
    pub start: usize,

    /// byte offset where this span ends (exclusive)
    pub end: usize,

    /// line number (1-based)
    pub line: usize,

    /// column number (1-based)
    pub col: usize,
}

impl Span {
    pub fn new(start: usize, end: usize, line: usize, col: usize) -> Self {
        Span { start, end, line, col }
    }

    /// A dummy span for generated nodes that have no source location.
    pub fn dummy() -> Self {
        Span { start: 0, end: 0, line: 0, col: 0 }
    }

    /// Is this a real span or a dummy?
    pub fn is_dummy(&self) -> bool {
        self.line == 0
    }

    /// How many characters does this span cover?
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Merge two spans into one that covers both.
    /// Used to span an entire expression from its
    /// leftmost to rightmost token.
    pub fn merge(&self, other: &Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end:   self.end.max(other.end),
            line:  self.line.min(other.line),
            col:   self.col.min(other.col),
        }
    }
}

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge() {
        let a = Span::new(0, 5, 1, 1);
        let b = Span::new(3, 10, 1, 4);
        let merged = a.merge(&b);
        assert_eq!(merged.start, 0);
        assert_eq!(merged.end, 10);
    }

    #[test]
    fn test_dummy() {
        let s = Span::dummy();
        assert!(s.is_dummy());
    }

    #[test]
    fn test_len() {
        let s = Span::new(2, 7, 1, 3);
        assert_eq!(s.len(), 5);
    }
}