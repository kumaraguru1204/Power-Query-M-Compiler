use pq_diagnostics::Span;
use pq_grammar::operators::{Operator, UnaryOp};
use pq_types::ColumnType;

/// A value expression paired with its source location and optional inferred type.
///
/// `inferred_type` starts as `None` and may be filled in by the type-checker
/// pass, making type information available to later passes without a separate
/// annotation map.
#[derive(Debug, Clone)]
pub struct ExprNode {
    pub expr:          Expr,
    pub span:          Span,
    /// Type inferred by the type-checker; `None` until checked.
    pub inferred_type: Option<ColumnType>,
}

impl ExprNode {
    /// Construct a node with no inferred type yet.
    pub fn new(expr: Expr, span: Span) -> Self {
        ExprNode { expr, span, inferred_type: None }
    }

    /// Builder: attach an inferred type.
    pub fn with_type(mut self, t: ColumnType) -> Self {
        self.inferred_type = Some(t);
        self
    }
}

/// Every kind of expression the language supports.
///
/// Designed to cover the full M expression language so semantic validation
/// (resolver + type-checker) can operate on a rich, unambiguous tree.
#[derive(Debug, Clone)]
pub enum Expr {
    // ── literals ──────────────────────────────────────────────────────────

    /// Integer literal: `42`
    IntLit(i64),

    /// Float literal: `3.14`
    FloatLit(f64),

    /// Boolean literal: `true` / `false`
    BoolLit(bool),

    /// String literal: `"hello"`
    StringLit(String),

    /// Null literal: `null`
    NullLit,

    // ── identifiers / column references ──────────────────────────────────

    /// Bare identifier: a variable name, step reference, or the implicit
    /// `_` lambda parameter.
    ///
    /// Distinct from `ColumnAccess` so the resolver can tell a variable
    /// reference (`Age`, `_`, `Order.Ascending`) apart from a bracketed
    /// field access (`[Age]`).  Use `"_"` when desugaring `each`.
    Identifier(String),

    /// Explicit bracket column access in row context: `[Age]`
    ///
    /// Strictly reserved for the `[col]` bracket syntax.  The resolver
    /// validates the field name against the current table schema.
    ColumnAccess(String),

    /// Field access on a named record variable: `row[FieldName]`
    ///
    /// Produced by an explicit named lambda such as `(row) => row[Age] * 2`.
    /// The `record` expression is the variable that holds the row record and
    /// `field` is the column name to access.  Semantically equivalent to
    /// `ColumnAccess(field)` when `record` is the implicit row parameter, but
    /// retains the source-level variable name for round-trip formatting.
    FieldAccess {
        record: Box<ExprNode>,
        field:  String,
    },

    // ── compound expressions ──────────────────────────────────────────────

    /// Binary operation: `Age > 25`, `Salary + 5000.0`, `A and B`
    BinaryOp {
        left:  Box<ExprNode>,
        op:    Operator,
        right: Box<ExprNode>,
    },

    /// Unary prefix operation: `not Active`, `-Amount`
    UnaryOp {
        op:      UnaryOp,
        operand: Box<ExprNode>,
    },

    // ── higher-order / callable expressions ──────────────────────────────

    /// Function call in expression context: `Text.Length([Name])`,
    /// `Number.From([Age])`, `List.Sum({1, 2, 3})`
    ///
    /// `name` is the fully-qualified M name, e.g. `"Text.Length"`.
    FunctionCall {
        name: String,
        args: Vec<ExprNode>,
    },

    /// Explicit lambda: `(x) => x > 0`, `() => 1`, `(x, y) => x + y`
    ///
    /// `params` holds every declared parameter name.  Zero-argument lambdas
    /// have an empty vec; `each <expr>` desugars to `params: vec!["_"]`.
    Lambda {
        params: Vec<String>,
        body:   Box<ExprNode>,
    },


    // ── collection literals ───────────────────────────────────────────────

    /// List literal: `{1, 2, 3}`, `{"a", "b"}`, `{Source, Other}`
    List(Vec<ExprNode>),

    /// Record literal: `[Name = "Alice", Age = 30]`
    ///
    /// Fields are `(field_name, value_expr)` pairs.
    Record(Vec<(String, ExprNode)>),
}

impl Expr {
    /// Is this expression a simple scalar literal?
    pub fn is_literal(&self) -> bool {
        matches!(
            self,
            Expr::IntLit(_)
            | Expr::FloatLit(_)
            | Expr::BoolLit(_)
            | Expr::StringLit(_)
            | Expr::NullLit
        )
    }

    /// Is this a bare identifier or bracket column-access reference?
    pub fn is_column_ref(&self) -> bool {
        matches!(self, Expr::Identifier(_) | Expr::ColumnAccess(_) | Expr::FieldAccess { .. })
    }

    /// Is this a lambda expression?  (`each` desugars to `Lambda { params: ["_"], .. }`)
    pub fn is_lambda_like(&self) -> bool {
        matches!(self, Expr::Lambda { .. })
    }
}