use pq_diagnostics::Span;

/// Every meaningful unit of the M-like language.
/// Every token carries a Span so we know exactly
/// where in the source it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Token { kind, span }
    }

    pub fn is_eof(&self) -> bool {
        self.kind == TokenKind::Eof
    }
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.kind)
    }
}

/// The kind of a token — what it actually is.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // ── literals ──────────────────────────────────────────────────────────
    StringLit(String),   // "hello"
    IntLit(i64),         // 42
    FloatLit(f64),       // 3.14
    BoolLit(bool),       // true / false
    NullLit,             // null

    // ── identifiers ───────────────────────────────────────────────────────
    Ident(String),       // Source, Age, Table ...

    // ── keywords ──────────────────────────────────────────────────────────
    Let,                 // let
    In,                  // in
    Each,                // each

    // ── logical keywords (also usable as infix/prefix operators) ─────────
    And,                 // and
    Or,                  // or
    Not,                 // not

    // ── comparison operators ──────────────────────────────────────────────
    Eq,                  // =
    NotEq,               // <>
    Gt,                  // >
    Lt,                  // <
    GtEq,                // >=
    LtEq,                // <=

    // ── arithmetic operators ──────────────────────────────────────────────
    Plus,                // +
    Minus,               // -
    Star,                // *
    Slash,               // /
    Ampersand,           // &
    // ── punctuation ───────────────────────────────────────────────────────
    Dot,                 // .
    Comma,               // ,
    FatArrow,            // =>  (lambda arrow)
    LParen,              // (
    RParen,              // )
    LBrace,              // {
    RBrace,              // }
    LBracket,            // [
    RBracket,            // ]

    // ── end of input ──────────────────────────────────────────────────────
    Eof,
}

impl std::fmt::Display for TokenKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            TokenKind::StringLit(s) => write!(f, "\"{}\"", s),
            TokenKind::IntLit(n)    => write!(f, "{}", n),
            TokenKind::FloatLit(n)  => write!(f, "{}", n),
            TokenKind::BoolLit(b)   => write!(f, "{}", b),
            TokenKind::NullLit      => write!(f, "null"),
            TokenKind::Ident(s)     => write!(f, "{}", s),
            TokenKind::Let          => write!(f, "let"),
            TokenKind::In           => write!(f, "in"),
            TokenKind::Each         => write!(f, "each"),
            TokenKind::And          => write!(f, "and"),
            TokenKind::Or           => write!(f, "or"),
            TokenKind::Not          => write!(f, "not"),
            TokenKind::Eq           => write!(f, "="),
            TokenKind::NotEq        => write!(f, "<>"),
            TokenKind::Gt           => write!(f, ">"),
            TokenKind::Lt           => write!(f, "<"),
            TokenKind::GtEq         => write!(f, ">="),
            TokenKind::LtEq         => write!(f, "<="),
            TokenKind::Plus         => write!(f, "+"),
            TokenKind::Minus        => write!(f, "-"),
            TokenKind::Star         => write!(f, "*"),
            TokenKind::Slash        => write!(f, "/"),
            TokenKind::Ampersand    => write!(f, "&"),
            TokenKind::Dot          => write!(f, "."),
            TokenKind::Comma        => write!(f, ","),
            TokenKind::FatArrow     => write!(f, "=>"),
            TokenKind::LParen       => write!(f, "("),
            TokenKind::RParen       => write!(f, ")"),
            TokenKind::LBrace       => write!(f, "{{"),
            TokenKind::RBrace       => write!(f, "}}"),
            TokenKind::LBracket     => write!(f, "["),
            TokenKind::RBracket     => write!(f, "]"),
            TokenKind::Eof          => write!(f, "<EOF>"),
        }
    }
}