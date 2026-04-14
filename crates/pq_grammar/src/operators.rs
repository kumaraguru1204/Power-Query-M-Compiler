/// Every binary operator in our M-like language.
#[derive(Debug, Clone, PartialEq)]
pub enum Operator {
    // comparison
    Eq,      // =
    NotEq,   // <>
    Gt,      // >
    Lt,      // <
    GtEq,    // >=
    LtEq,    // <=

    // arithmetic
    Add,     // +
    Sub,     // -
    Mul,     // *
    Div,     // /

    // concatenation
    Concat,  // &

    // logical (lower precedence than comparison)
    And,     // and
    Or,      // or
}

/// Unary (prefix) operators.
#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Not,  // not expr  — logical negation
    Neg,  // -expr     — arithmetic negation
}

impl UnaryOp {
    pub fn to_symbol(&self) -> &str {
        match self {
            UnaryOp::Not => "not",
            UnaryOp::Neg => "-",
        }
    }
}

impl std::fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.to_symbol())
    }
}

/// Operator precedence.
/// Higher number = binds tighter.
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct Precedence(pub u8);

impl Operator {
    pub fn precedence(&self) -> Precedence {
        match self {
            // logical — lowest
            Operator::Or               => Precedence(5),
            Operator::And              => Precedence(6),

            // comparison
            Operator::Eq
            | Operator::NotEq
            | Operator::Gt
            | Operator::Lt
            | Operator::GtEq
            | Operator::LtEq          => Precedence(10),

            // arithmetic
            Operator::Add
            | Operator::Sub           => Precedence(20),

            // concatenation — same precedence as add/sub
            Operator::Concat          => Precedence(20),

            Operator::Mul
            | Operator::Div           => Precedence(30),
        }
    }

    /// Is this operator left associative?
    pub fn is_left_associative(&self) -> bool {
        true
    }

    /// Symbol used in M syntax.
    pub fn to_symbol(&self) -> &str {
        match self {
            Operator::Eq    => "=",
            Operator::NotEq => "<>",
            Operator::Gt    => ">",
            Operator::Lt    => "<",
            Operator::GtEq  => ">=",
            Operator::LtEq  => "<=",
            Operator::Add   => "+",
            Operator::Sub   => "-",
            Operator::Mul   => "*",
            Operator::Div   => "/",
            Operator::And   => "and",
            Operator::Or    => "or",
            Operator::Concat => "&",
        }
    }

    /// Is this a comparison operator? Produces Boolean.
    pub fn is_comparison(&self) -> bool {
        matches!(
            self,
            Operator::Eq
            | Operator::NotEq
            | Operator::Gt
            | Operator::Lt
            | Operator::GtEq
            | Operator::LtEq
        )
    }

    /// Is this an arithmetic operator? Requires numeric operands.
    pub fn is_arithmetic(&self) -> bool {
        matches!(
            self,
            Operator::Add
            | Operator::Sub
            | Operator::Mul
            | Operator::Div
        )
    }

    /// Is this a logical operator? Requires Boolean operands, produces Boolean.
    pub fn is_logical(&self) -> bool {
        matches!(self, Operator::And | Operator::Or)
    }

    /// Is this a concatenation operator? Produces Text.
    pub fn is_concatenation(&self) -> bool {
        matches!(self, Operator::Concat)
    }
}

impl std::fmt::Display for Operator {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.to_symbol())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_precedence_ordering() {
        assert!(Operator::Mul.precedence() > Operator::Add.precedence());
        assert!(Operator::Add.precedence() > Operator::Eq.precedence());
        assert!(Operator::Eq.precedence()  > Operator::And.precedence());
        assert!(Operator::And.precedence() > Operator::Or.precedence());
    }

    #[test]
    fn test_comparison_vs_arithmetic() {
        assert!(Operator::Gt.is_comparison());
        assert!(!Operator::Gt.is_arithmetic());
        assert!(Operator::Add.is_arithmetic());
        assert!(!Operator::Add.is_comparison());
    }

    #[test]
    fn test_logical_operators() {
        assert!(Operator::And.is_logical());
        assert!(Operator::Or.is_logical());
        assert!(!Operator::Add.is_logical());
        assert!(!Operator::Eq.is_logical());
    }
}