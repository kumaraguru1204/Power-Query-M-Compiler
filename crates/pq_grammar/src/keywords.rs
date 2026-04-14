/// Every reserved keyword in our M-like language.
/// These cannot be used as identifiers.
#[derive(Debug, Clone, PartialEq)]
pub enum Keyword {
    Let,
    In,
    Each,
    True,
    False,
    And,   // and  — logical AND
    Or,    // or   — logical OR
    Not,   // not  — logical NOT (unary)
    Null,  // null — null literal
}

/// All keywords as static string slices.
/// The lexer checks every identifier against this list.
pub const KEYWORDS: &[(&str, Keyword)] = &[
    ("let",   Keyword::Let),
    ("in",    Keyword::In),
    ("each",  Keyword::Each),
    ("true",  Keyword::True),
    ("false", Keyword::False),
    ("and",   Keyword::And),
    ("or",    Keyword::Or),
    ("not",   Keyword::Not),
    ("null",  Keyword::Null),
];

/// Check if a string is a keyword.
pub fn lookup_keyword(s: &str) -> Option<Keyword> {
    KEYWORDS
        .iter()
        .find(|(k, _)| *k == s)
        .map(|(_, v)| v.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyword_lookup() {
        assert_eq!(lookup_keyword("let"),  Some(Keyword::Let));
        assert_eq!(lookup_keyword("in"),   Some(Keyword::In));
        assert_eq!(lookup_keyword("each"), Some(Keyword::Each));
        assert_eq!(lookup_keyword("and"),  Some(Keyword::And));
        assert_eq!(lookup_keyword("or"),   Some(Keyword::Or));
        assert_eq!(lookup_keyword("not"),  Some(Keyword::Not));
        assert_eq!(lookup_keyword("null"), Some(Keyword::Null));
        assert_eq!(lookup_keyword("foo"),  None);
    }
}