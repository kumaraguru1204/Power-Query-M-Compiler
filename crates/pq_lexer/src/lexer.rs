use pq_diagnostics::{Diagnostic, Span};
use pq_grammar::keywords::lookup_keyword;
use crate::token::{Token, TokenKind};

/// Errors the lexer can produce.
#[derive(Debug)]
pub enum LexError {
    UnterminatedString(Span),
    UnexpectedChar(char, Span),
    InvalidNumber(String, Span),
}

impl LexError {
    /// Convert a LexError into a Diagnostic for reporting.
    pub fn to_diagnostic(&self) -> Diagnostic {
        match self {
            LexError::UnterminatedString(span) => {
                Diagnostic::error("E101", "unterminated string literal")
                    .with_label(span.clone(), "string started here, never closed")
                    .with_suggestion("add a closing '\"' to end the string")
            }
            LexError::UnexpectedChar(c, span) => {
                Diagnostic::error(
                    "E102",
                    format!("unexpected character '{}'", c)
                )
                    .with_label(span.clone(), "unexpected character here")
            }
            LexError::InvalidNumber(n, span) => {
                Diagnostic::error(
                    "E103",
                    format!("invalid number literal '{}'", n)
                )
                    .with_label(span.clone(), "invalid number here")
            }
        }
    }
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            LexError::UnterminatedString(_) =>
                write!(f, "unterminated string literal"),
            LexError::UnexpectedChar(c, _) =>
                write!(f, "unexpected character '{}'", c),
            LexError::InvalidNumber(n, _) =>
                write!(f, "invalid number '{}'", n),
        }
    }
}

pub type LexResult = Result<Vec<Token>, LexError>;

/// Breaks a formula string into a flat list of tokens.
/// Every token carries a Span with its exact line and column.
pub struct Lexer {
    /// source characters
    input:  Vec<char>,

    /// current position in input
    pos:    usize,

    /// current line number (1-based)
    line:   usize,

    /// current column number (1-based)
    col:    usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Lexer {
            input: input.chars().collect(),
            pos:   0,
            line:  1,
            col:   1,
        }
    }

    // ── position tracking ─────────────────────────────────────────────────

    fn peek(&self) -> Option<char> {
        self.input.get(self.pos).copied()
    }

    fn peek_next(&self) -> Option<char> {
        self.input.get(self.pos + 1).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.input.get(self.pos).copied();
        if let Some(c) = ch {
            self.pos += 1;
            if c == '\n' {
                self.line += 1;
                self.col   = 1;
            } else {
                self.col  += 1;
            }
        }
        ch
    }

    fn current_span(&self, start_pos: usize, start_line: usize, start_col: usize) -> Span {
        Span::new(start_pos, self.pos, start_line, start_col)
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    // ── readers ───────────────────────────────────────────────────────────

    fn read_string(
        &mut self,
        start_pos:  usize,
        start_line: usize,
        start_col:  usize,
    ) -> Result<Token, LexError> {
        self.advance(); // consume opening "
        let mut s = String::new();

        loop {
            match self.advance() {
                Some('"') => {
                    let span = self.current_span(start_pos, start_line, start_col);
                    return Ok(Token::new(TokenKind::StringLit(s), span));
                }
                Some(c) => s.push(c),
                None => {
                    let span = self.current_span(start_pos, start_line, start_col);
                    return Err(LexError::UnterminatedString(span));
                }
            }
        }
    }

    fn read_number(
        &mut self,
        start_pos:  usize,
        start_line: usize,
        start_col:  usize,
    ) -> Result<Token, LexError> {
        let mut s    = String::new();
        let mut dots = 0u8;

        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                s.push(c);
                self.advance();
            } else if c == '.' {
                dots += 1;
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }

        let span = self.current_span(start_pos, start_line, start_col);

        if dots == 0 {
            s.parse::<i64>()
                .map(|n| Token::new(TokenKind::IntLit(n), span.clone()))
                .map_err(|_| LexError::InvalidNumber(s, span))
        } else if dots == 1 {
            s.parse::<f64>()
                .map(|n| Token::new(TokenKind::FloatLit(n), span.clone()))
                .map_err(|_| LexError::InvalidNumber(s, span))
        } else {
            Err(LexError::InvalidNumber(s, span))
        }
    }

    fn read_ident(
        &mut self,
        start_pos:  usize,
        start_line: usize,
        start_col:  usize,
    ) -> Token {
        let mut s = String::new();

        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }

        let span = self.current_span(start_pos, start_line, start_col);

        // check if this identifier is a keyword
        // we use pq_grammar's keyword registry here
        let kind = match lookup_keyword(&s) {
            Some(pq_grammar::keywords::Keyword::Let)   => TokenKind::Let,
            Some(pq_grammar::keywords::Keyword::In)    => TokenKind::In,
            Some(pq_grammar::keywords::Keyword::Each)  => TokenKind::Each,
            Some(pq_grammar::keywords::Keyword::True)  => TokenKind::BoolLit(true),
            Some(pq_grammar::keywords::Keyword::False) => TokenKind::BoolLit(false),
            Some(pq_grammar::keywords::Keyword::And)   => TokenKind::And,
            Some(pq_grammar::keywords::Keyword::Or)    => TokenKind::Or,
            Some(pq_grammar::keywords::Keyword::Not)   => TokenKind::Not,
            Some(pq_grammar::keywords::Keyword::Null)  => TokenKind::NullLit,
            None => TokenKind::Ident(s),
        };

        Token::new(kind, span)
    }

    // ── main tokenizer ────────────────────────────────────────────────────

    pub fn tokenize(&mut self) -> LexResult {
        let mut tokens = Vec::new();

        loop {
            self.skip_whitespace();

            let start_pos  = self.pos;
            let start_line = self.line;
            let start_col  = self.col;

            match self.peek() {
                // end of input
                None => {
                    let span = self.current_span(start_pos, start_line, start_col);
                    tokens.push(Token::new(TokenKind::Eof, span));
                    break;
                }

                Some(c) => {
                    let tok = match c {
                        // string literal
                        '"' => self.read_string(start_pos, start_line, start_col)?,

                        // number literal
                        '0'..='9' => self.read_number(start_pos, start_line, start_col)?,

                        // identifier or keyword
                        'a'..='z' | 'A'..='Z' | '_' => {
                            self.read_ident(start_pos, start_line, start_col)
                        }

                        // punctuation — single character
                        '.' => {
                            self.advance();
                            let span = self.current_span(start_pos, start_line, start_col);
                            Token::new(TokenKind::Dot, span)
                        }
                        ',' => {
                            self.advance();
                            let span = self.current_span(start_pos, start_line, start_col);
                            Token::new(TokenKind::Comma, span)
                        }
                        '(' => {
                            self.advance();
                            let span = self.current_span(start_pos, start_line, start_col);
                            Token::new(TokenKind::LParen, span)
                        }
                        ')' => {
                            self.advance();
                            let span = self.current_span(start_pos, start_line, start_col);
                            Token::new(TokenKind::RParen, span)
                        }
                        '{' => {
                            self.advance();
                            let span = self.current_span(start_pos, start_line, start_col);
                            Token::new(TokenKind::LBrace, span)
                        }
                        '}' => {
                            self.advance();
                            let span = self.current_span(start_pos, start_line, start_col);
                            Token::new(TokenKind::RBrace, span)
                        }
                        '[' => {
                            self.advance();
                            let span = self.current_span(start_pos, start_line, start_col);
                            Token::new(TokenKind::LBracket, span)
                        }
                        ']' => {
                            self.advance();
                            let span = self.current_span(start_pos, start_line, start_col);
                            Token::new(TokenKind::RBracket, span)
                        }
                        '+' => {
                            self.advance();
                            let span = self.current_span(start_pos, start_line, start_col);
                            Token::new(TokenKind::Plus, span)
                        }
                        '-' => {
                            self.advance();
                            let span = self.current_span(start_pos, start_line, start_col);
                            Token::new(TokenKind::Minus, span)
                        }
                        '*' => {
                            self.advance();
                            let span = self.current_span(start_pos, start_line, start_col);
                            Token::new(TokenKind::Star, span)
                        }
                        '/' => {
                            self.advance();
                            let span = self.current_span(start_pos, start_line, start_col);
                            Token::new(TokenKind::Slash, span)
                        }

                        // = or =>
                        '=' => {
                            self.advance();
                            if self.peek() == Some('>') {
                                self.advance();
                                let span = self.current_span(start_pos, start_line, start_col);
                                Token::new(TokenKind::FatArrow, span)
                            } else {
                                let span = self.current_span(start_pos, start_line, start_col);
                                Token::new(TokenKind::Eq, span)
                            }
                        }

                        // > or >=
                        '>' => {
                            self.advance();
                            if self.peek() == Some('=') {
                                self.advance();
                                let span = self.current_span(start_pos, start_line, start_col);
                                Token::new(TokenKind::GtEq, span)
                            } else {
                                let span = self.current_span(start_pos, start_line, start_col);
                                Token::new(TokenKind::Gt, span)
                            }
                        }

                        // < or <= or <>
                        '<' => {
                            self.advance();
                            match self.peek() {
                                Some('=') => {
                                    self.advance();
                                    let span = self.current_span(start_pos, start_line, start_col);
                                    Token::new(TokenKind::LtEq, span)
                                }
                                Some('>') => {
                                    self.advance();
                                    let span = self.current_span(start_pos, start_line, start_col);
                                    Token::new(TokenKind::NotEq, span)
                                }
                                _ => {
                                    let span = self.current_span(start_pos, start_line, start_col);
                                    Token::new(TokenKind::Lt, span)
                                }
                            }
                        }

                        // &
                        '&' => {
                            self.advance();
                            let span = self.current_span(start_pos, start_line, start_col);
                            Token::new(TokenKind::Ampersand, span)
                        }

                        // #"..." quoted identifier
                        '#' => {
                            if self.peek_next() == Some('"') {
                                self.advance(); // consume #
                                self.advance(); // consume opening "
                                let mut s = String::new();
                                loop {
                                    match self.advance() {
                                        Some('"') => break,
                                        Some(c)   => s.push(c),
                                        None => {
                                            let span = self.current_span(start_pos, start_line, start_col);
                                            return Err(LexError::UnterminatedString(span));
                                        }
                                    }
                                }
                                let span = self.current_span(start_pos, start_line, start_col);
                                Token::new(TokenKind::Ident(s), span)
                            } else {
                                self.advance();
                                let span = self.current_span(start_pos, start_line, start_col);
                                return Err(LexError::UnexpectedChar('#', span));
                            }
                        }

                        other => {
                            self.advance();
                            let span = self.current_span(start_pos, start_line, start_col);
                            return Err(LexError::UnexpectedChar(other, span));
                        }
                    };

                    tokens.push(tok);
                }
            }
        }

        Ok(tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokenize(input: &str) -> Vec<TokenKind> {
        Lexer::new(input)
            .tokenize()
            .unwrap()
            .into_iter()
            .map(|t| t.kind)
            .collect()
    }

    #[test]
    fn test_keywords() {
        let kinds = tokenize("let in each");
        assert_eq!(kinds[0], TokenKind::Let);
        assert_eq!(kinds[1], TokenKind::In);
        assert_eq!(kinds[2], TokenKind::Each);
    }

    #[test]
    fn test_bool_literals() {
        let kinds = tokenize("true false");
        assert_eq!(kinds[0], TokenKind::BoolLit(true));
        assert_eq!(kinds[1], TokenKind::BoolLit(false));
    }

    #[test]
    fn test_int_literal() {
        let kinds = tokenize("42");
        assert_eq!(kinds[0], TokenKind::IntLit(42));
    }

    #[test]
    fn test_float_literal() {
        let kinds = tokenize("3.14");
        assert_eq!(kinds[0], TokenKind::FloatLit(3.14));
    }

    #[test]
    fn test_string_literal() {
        let kinds = tokenize("\"hello\"");
        assert_eq!(kinds[0], TokenKind::StringLit("hello".into()));
    }

    #[test]
    fn test_operators() {
        let kinds = tokenize(">= <= <> > < =");
        assert_eq!(kinds[0], TokenKind::GtEq);
        assert_eq!(kinds[1], TokenKind::LtEq);
        assert_eq!(kinds[2], TokenKind::NotEq);
        assert_eq!(kinds[3], TokenKind::Gt);
        assert_eq!(kinds[4], TokenKind::Lt);
        assert_eq!(kinds[5], TokenKind::Eq);
    }

    #[test]
    fn test_arithmetic() {
        let kinds = tokenize("+ - * /");
        assert_eq!(kinds[0], TokenKind::Plus);
        assert_eq!(kinds[1], TokenKind::Minus);
        assert_eq!(kinds[2], TokenKind::Star);
        assert_eq!(kinds[3], TokenKind::Slash);
    }

    #[test]
    fn test_span_tracking() {
        let tokens = Lexer::new("let x").tokenize().unwrap();
        assert_eq!(tokens[0].span.line, 1);
        assert_eq!(tokens[0].span.col,  1);
        assert_eq!(tokens[1].span.col,  5);
    }

    #[test]
    fn test_multiline_span() {
        let tokens = Lexer::new("let\n    x").tokenize().unwrap();
        assert_eq!(tokens[0].span.line, 1);
        assert_eq!(tokens[1].span.line, 2);
    }

    #[test]
    fn test_unterminated_string() {
        let result = Lexer::new("\"hello").tokenize();
        assert!(result.is_err());
        matches!(result.unwrap_err(), LexError::UnterminatedString(_));
    }

    #[test]
    fn test_unexpected_char() {
        let result = Lexer::new("@").tokenize();
        assert!(result.is_err());
    }

    #[test]
    fn test_ampersand() {
        let kinds = tokenize("a & b");
        assert_eq!(kinds[0], TokenKind::Ident("a".into()));
        assert_eq!(kinds[1], TokenKind::Ampersand);
        assert_eq!(kinds[2], TokenKind::Ident("b".into()));
    }

    #[test]
    fn test_quoted_ident() {
        let kinds = tokenize(r#"#"Spaced Col""#);
        assert_eq!(kinds[0], TokenKind::Ident("Spaced Col".into()));
    }

    #[test]
    fn test_bare_hash_error() {
        let result = Lexer::new("#x").tokenize();
        assert!(result.is_err());
        matches!(result.unwrap_err(), LexError::UnexpectedChar('#', _));
    }
}