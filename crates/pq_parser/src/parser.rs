use pq_diagnostics::Span;
use pq_grammar::functions::{lookup_function, lookup_qualified, ArgKind};
use pq_grammar::operators::{Operator, UnaryOp};
use pq_lexer::token::{Token, TokenKind};
use pq_ast::{
    expr::{Expr, ExprNode},
    step::{Step, StepKind, SortOrder, JoinKind, AggregateSpec, MissingFieldKind},
    call_arg::CallArg,
    program::{Program, StepBinding},
};
use pq_types::ColumnType;
use crate::error::{ParseError, ParseResult};

pub struct Parser {
    tokens: Vec<Token>,
    pos:    usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    // ── token navigation ──────────────────────────────────────────────────

    fn peek(&self) -> &Token {
        self.tokens
            .get(self.pos)
            .unwrap_or_else(|| self.tokens.last().unwrap())
    }

    fn peek_kind(&self) -> &TokenKind {
        &self.peek().kind
    }

    /// Second look-ahead (one token past peek).
    fn peek_offset(&self, offset: usize) -> &TokenKind {
        self.tokens
            .get(self.pos + offset)
            .map(|t| &t.kind)
            .unwrap_or(&TokenKind::Eof)
    }

    fn advance(&mut self) -> Token {
        let tok = self.tokens
            .get(self.pos)
            .cloned()
            .unwrap_or_else(|| self.tokens.last().unwrap().clone());
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        tok
    }

    fn current_span(&self) -> Span {
        self.peek().span.clone()
    }

    fn expect_ident(&mut self) -> ParseResult<(String, Span)> {
        let tok = self.advance();
        match tok.kind {
            TokenKind::Ident(s) => Ok((s, tok.span)),
            other => Err(ParseError::UnexpectedToken {
                expected: "identifier".into(),
                got:      other,
                span:     tok.span,
            }),
        }
    }

    /// Consume an identifier that must have the exact given name.
    fn expect_ident_named(&mut self, name: &'static str) -> ParseResult<Span> {
        let tok = self.advance();
        match &tok.kind {
            TokenKind::Ident(s) if s == name => Ok(tok.span),
            other => Err(ParseError::UnexpectedToken {
                expected: format!("'{}'", name),
                got:      other.clone(),
                span:     tok.span,
            }),
        }
    }

    fn expect_string(&mut self) -> ParseResult<(String, Span)> {
        let tok = self.advance();
        match tok.kind {
            TokenKind::StringLit(s) => Ok((s, tok.span)),
            other => Err(ParseError::UnexpectedToken {
                expected: "string literal".into(),
                got:      other,
                span:     tok.span,
            }),
        }
    }

    fn expect_token(&mut self, expected: TokenKind) -> ParseResult<Span> {
        let tok = self.advance();
        if tok.kind == expected {
            Ok(tok.span)
        } else {
            Err(ParseError::UnexpectedToken {
                expected: format!("'{}'", expected),
                got:      tok.kind,
                span:     tok.span,
            })
        }
    }

    fn parse_qualified_name(&mut self) -> ParseResult<(String, String, Span)> {
        let (ns, ns_span)   = self.expect_ident()?;
        self.expect_token(TokenKind::Dot)?;
        let (func, fn_span) = self.expect_ident()?;
        let span            = ns_span.merge(&fn_span);
        Ok((ns, func, span))
    }

    // ── Pratt expression parser ───────────────────────────────────────────

    /// Returns the infix left/right binding powers for a binary operator token,
    /// or `None` if the token is not a binary operator.
    fn infix_bp(kind: &TokenKind) -> Option<(u8, u8)> {
        Some(match kind {
            // lowest precedence: logical or / and
            TokenKind::Or              => (1, 2),
            TokenKind::And             => (3, 4),
            // comparison
            TokenKind::Eq
            | TokenKind::NotEq
            | TokenKind::Gt
            | TokenKind::Lt
            | TokenKind::GtEq
            | TokenKind::LtEq         => (5, 6),
            // additive
            TokenKind::Plus
            | TokenKind::Minus
            | TokenKind::Ampersand => (7, 8),
            // multiplicative
            TokenKind::Star
            | TokenKind::Slash        => (9, 10),
            _ => return None,
        })
    }

    /// Map a consumed infix-operator token to its `Operator` variant.
    fn token_to_infix_op(kind: TokenKind) -> Operator {
        match kind {
            TokenKind::Or    => Operator::Or,
            TokenKind::And   => Operator::And,
            TokenKind::Eq    => Operator::Eq,
            TokenKind::NotEq => Operator::NotEq,
            TokenKind::Gt    => Operator::Gt,
            TokenKind::Lt    => Operator::Lt,
            TokenKind::GtEq  => Operator::GtEq,
            TokenKind::LtEq  => Operator::LtEq,
            TokenKind::Plus  => Operator::Add,
            TokenKind::Minus => Operator::Sub,
            TokenKind::Star  => Operator::Mul,
            TokenKind::Slash => Operator::Div,
            TokenKind::Ampersand => Operator::Concat,
            other => panic!("token_to_infix_op: not an infix op: {:?}", other),
        }
    }

    /// Top-level expression entry point.
    fn parse_expr(&mut self) -> ParseResult<ExprNode> {
        self.parse_pratt(0)
    }

    /// Pratt (top-down operator-precedence) parser.
    ///
    /// `min_bp` is the minimum left binding-power of the next infix operator
    /// that this call is allowed to consume.
    fn parse_pratt(&mut self, min_bp: u8) -> ParseResult<ExprNode> {
        // prefix / unary
        let mut lhs = self.parse_prefix()?;

        loop {
            // Peek at binding powers without holding a borrow on `self`.
            let (left_bp, right_bp) = match Self::infix_bp(self.peek_kind()) {
                Some(bp) => bp,
                None     => break,
            };
            if left_bp < min_bp { break; }

            let op_tok = self.advance();
            let op     = Self::token_to_infix_op(op_tok.kind);
            let rhs    = self.parse_pratt(right_bp)?;
            let span   = lhs.span.merge(&rhs.span);
            lhs = ExprNode::new(
                Expr::BinaryOp { left: Box::new(lhs), op, right: Box::new(rhs) },
                span,
            );
        }
        Ok(lhs)
    }

    /// Parse a unary prefix expression (`not`, `-`) or fall through to primary.
    fn parse_prefix(&mut self) -> ParseResult<ExprNode> {
        // Clone the kind so we release the immutable borrow before mutating.
        let kind = self.peek_kind().clone();
        match kind {
            TokenKind::Not => {
                let tok     = self.advance();
                let operand = self.parse_pratt(11)?; // higher bp than any infix
                let span    = tok.span.merge(&operand.span);
                Ok(ExprNode::new(
                    Expr::UnaryOp { op: UnaryOp::Not, operand: Box::new(operand) },
                    span,
                ))
            }
            TokenKind::Minus => {
                let tok     = self.advance();
                let operand = self.parse_pratt(11)?;
                let span    = tok.span.merge(&operand.span);
                Ok(ExprNode::new(
                    Expr::UnaryOp { op: UnaryOp::Neg, operand: Box::new(operand) },
                    span,
                ))
            }
            _ => self.parse_primary(),
        }
    }

    /// Parse an atomic (primary) expression.
    fn parse_primary(&mut self) -> ParseResult<ExprNode> {
        let kind = self.peek_kind().clone();
        match kind {

            // ── scalar literals ───────────────────────────────────────────
            TokenKind::IntLit(_)
            | TokenKind::FloatLit(_)
            | TokenKind::BoolLit(_)
            | TokenKind::StringLit(_) => {
                let tok = self.advance();
                let expr = match tok.kind {
                    TokenKind::IntLit(n)    => Expr::IntLit(n),
                    TokenKind::FloatLit(n)  => Expr::FloatLit(n),
                    TokenKind::BoolLit(b)   => Expr::BoolLit(b),
                    TokenKind::StringLit(s) => Expr::StringLit(s),
                    _ => unreachable!(),
                };
                Ok(ExprNode::new(expr, tok.span))
            }

            TokenKind::NullLit => {
                let tok = self.advance();
                Ok(ExprNode::new(Expr::NullLit, tok.span))
            }

            // ── each expr — sugar for (_ ) => body ───────────────────────
            TokenKind::Each => {
                self.parse_each_expr()
            }

            // ── [ … ] — column access [Col] or record literal [F = v] ────
            TokenKind::LBracket => {
                let start = self.advance(); // consume [

                // Two-token look-ahead: "[Ident ]" → ColumnAccess
                //                       anything else → Record literal
                let is_col_access = matches!(self.peek_kind(), TokenKind::Ident(_))
                    && matches!(self.peek_offset(1), TokenKind::RBracket);

                if is_col_access {
                    let (name, _) = self.expect_ident()?;
                    let end       = self.expect_token(TokenKind::RBracket)?;
                    let span      = start.span.merge(&end);
                    Ok(ExprNode::new(Expr::ColumnAccess(name), span))
                } else {
                    // Record literal: [Field = expr, ...]
                    let mut fields = vec![];
                    while self.peek_kind() != &TokenKind::RBracket
                        && self.peek_kind() != &TokenKind::Eof
                    {
                        let (fname, _) = self.expect_ident()?;
                        self.expect_token(TokenKind::Eq)?;
                        let val = self.parse_expr()?;
                        fields.push((fname, val));
                        if self.peek_kind() == &TokenKind::Comma {
                            self.advance();
                        }
                    }
                    let end  = self.expect_token(TokenKind::RBracket)?;
                    let span = start.span.merge(&end);
                    Ok(ExprNode::new(Expr::Record(fields), span))
                }
            }

            // ── ( … ) — grouped expression or lambda (params,…) => body ──
            TokenKind::LParen => {
                let start = self.advance(); // consume (

                // Detect a lambda by scanning ahead for the closing `)` followed
                // by `=>`.  The parameter list may be:
                //   ()           → zero params
                //   (x)          → one param
                //   (x, y, …)    → N params (all identifiers separated by commas)
                //
                // We do a speculative scan rather than multi-token look-ahead so
                // we don't need a fixed offset limit.
                let is_lambda = self.is_lambda_params();

                if is_lambda {
                    // Consume parameter name list.
                    let mut params = vec![];
                    while self.peek_kind() != &TokenKind::RParen
                        && self.peek_kind() != &TokenKind::Eof
                    {
                        let (p, _) = self.expect_ident()?;
                        params.push(p);
                        if self.peek_kind() == &TokenKind::Comma {
                            self.advance();
                        }
                    }
                    self.expect_token(TokenKind::RParen)?;
                    self.expect_token(TokenKind::FatArrow)?;
                    let body = self.parse_expr()?;
                    let span = start.span.merge(&body.span);
                    Ok(ExprNode::new(Expr::Lambda { params, body: Box::new(body) }, span))
                } else {
                    // Grouped expression — parse inner, discard outer parens.
                    let inner = self.parse_expr()?;
                    self.expect_token(TokenKind::RParen)?;
                    Ok(inner)
                }
            }

            // ── { … } — list literal ──────────────────────────────────────
            TokenKind::LBrace => {
                let start = self.advance(); // consume {
                let mut items = vec![];
                while self.peek_kind() != &TokenKind::RBrace
                    && self.peek_kind() != &TokenKind::Eof
                {
                    items.push(self.parse_expr()?);
                    if self.peek_kind() == &TokenKind::Comma {
                        self.advance();
                    }
                }
                let end  = self.expect_token(TokenKind::RBrace)?;
                let span = start.span.merge(&end);
                Ok(ExprNode::new(Expr::List(items), span))
            }

            // ── identifiers — bare Column or qualified FunctionCall ────────
            TokenKind::Ident(_) => {
                let (name, name_span) = self.expect_ident()?;

                if self.peek_kind() == &TokenKind::Dot {
                    self.advance(); // consume .
                    let (func, func_span) = self.expect_ident()?;

                    if self.peek_kind() == &TokenKind::LParen {
                        // FunctionCall: Namespace.Function(args...)
                        self.advance(); // consume (
                        let mut args = vec![];
                        while self.peek_kind() != &TokenKind::RParen
                            && self.peek_kind() != &TokenKind::Eof
                        {
                            args.push(self.parse_expr()?);
                            if self.peek_kind() == &TokenKind::Comma {
                                self.advance();
                            }
                        }
                        let end  = self.expect_token(TokenKind::RParen)?;
                        let span = name_span.merge(&end);
                        let qname = format!("{}.{}", name, func);
                        // Validate that the function exists in the grammar registry.
                        if lookup_qualified(&qname).is_none() {
                            return Err(ParseError::UnknownFunction {
                                qualified: qname,
                                span,
                            });
                        }
                        Ok(ExprNode::new(Expr::FunctionCall { name: qname, args }, span))
                    } else {
                        // Dotted identifier without a call — e.g. "Order.Ascending"
                        // treated as an opaque Identifier reference.
                        let span  = name_span.merge(&func_span);
                        let qname = format!("{}.{}", name, func);
                        Ok(ExprNode::new(Expr::Identifier(qname), span))
                    }
                } else {
                    // Check for field access: `ident[Field]`
                    // Three-token look-ahead: `[ Ident ]` → FieldAccess.
                    // `[ Ident =` would be a record literal — not field access.
                    if matches!(self.peek_kind(), TokenKind::LBracket)
                        && matches!(self.peek_offset(1), TokenKind::Ident(_))
                        && matches!(self.peek_offset(2), TokenKind::RBracket)
                    {
                        let base = ExprNode::new(Expr::Identifier(name), name_span.clone());
                        self.advance(); // consume [
                        let (field, _) = self.expect_ident()?;
                        let end  = self.expect_token(TokenKind::RBracket)?;
                        let span = name_span.merge(&end);
                        Ok(ExprNode::new(
                            Expr::FieldAccess { record: Box::new(base), field },
                            span,
                        ))
                    } else {
                        // Bare identifier — variable / step reference.
                        Ok(ExprNode::new(Expr::Identifier(name), name_span))
                    }
                }
            }

            other => {
                let span = self.current_span();
                self.advance();
                Err(ParseError::UnexpectedToken {
                    expected: "expression".into(),
                    got:      other,
                    span,
                })
            }
        }
    }

    // ── each / lambda helpers ─────────────────────────────────────────────

    /// Speculative scan: returns `true` when the token stream starting at the
    /// current position (immediately after the `(` has been consumed) matches
    /// the lambda parameter syntax `(ident, …) =>`.
    ///
    /// Specifically, a lambda header is a (possibly empty) comma-separated
    /// list of identifiers followed by `)` then `=>`.
    fn is_lambda_params(&self) -> bool {
        let mut offset = 0;
        loop {
            match self.peek_offset(offset) {
                // `)` — end of param list; next token must be `=>`.
                TokenKind::RParen => return matches!(self.peek_offset(offset + 1), TokenKind::FatArrow),
                // An identifier is a valid param name.
                TokenKind::Ident(_) => {
                    offset += 1;
                    // After a name we accept `,` (more params) or `)` (end).
                    match self.peek_offset(offset) {
                        TokenKind::Comma  => { offset += 1; continue; }
                        TokenKind::RParen => continue, // will be handled at top of loop
                        _ => return false,
                    }
                }
                _ => return false,
            }
        }
    }

    /// Parse `each <expr>` and desugar it to `Lambda { params: ["_"], body }`.
    ///
    /// `each Age > 25` becomes `Lambda { params: ["_"], body: Age > 25 }`,
    /// giving every callable form a single unambiguous AST representation.
    fn parse_each_expr(&mut self) -> ParseResult<ExprNode> {
        let tok = self.advance();
        if tok.kind != TokenKind::Each {
            return Err(ParseError::UnexpectedToken {
                expected: "'each'".into(),
                got:      tok.kind,
                span:     tok.span,
            });
        }
        let body = self.parse_pratt(0)?;
        let span = tok.span.merge(&body.span);
        Ok(ExprNode::new(Expr::Lambda { params: vec!["_".into()], body: Box::new(body) }, span))
    }

    // ── type / column / rename / sort parsers (step-arg context) ─────────

    fn parse_m_type(&mut self) -> ParseResult<ColumnType> {
        let (part1, span1) = self.expect_ident()?;

        // Handle "type text", "type number", etc. — lowercase text-form types
        if part1 == "type" {
            let (part2, span2) = self.expect_ident()?;
            let span = span1.merge(&span2);
            return match part2.as_str() {
                "text"         => Ok(ColumnType::Text),
                "number"       => Ok(ColumnType::Float),
                "logical"      => Ok(ColumnType::Boolean),
                "int64"        => Ok(ColumnType::Integer),
                "date"         => Ok(ColumnType::Date),
                "datetime"     => Ok(ColumnType::DateTime),
                "datetimezone" => Ok(ColumnType::DateTimeZone),
                "duration"     => Ok(ColumnType::Duration),
                "time"         => Ok(ColumnType::Time),
                "currency"     => Ok(ColumnType::Currency),
                "binary"       => Ok(ColumnType::Binary),
                "any"          => Ok(ColumnType::Text),
                "null"         => Ok(ColumnType::Null),
                _ => Err(ParseError::UnknownType {
                    type_str: format!("type {}", part2),
                    span,
                }),
            };
        }

        // Standard "Ident.Ident" form, e.g. Int64.Type
        self.expect_token(TokenKind::Dot)?;
        let (part2, span2) = self.expect_ident()?;
        let type_str       = format!("{}.{}", part1, part2);
        let span           = span1.merge(&span2);
        ColumnType::from_m_type(&type_str)
            .ok_or_else(|| ParseError::UnknownType {
                type_str,
                span,
            })
    }

    fn parse_type_pair(&mut self) -> ParseResult<(String, ColumnType)> {
        self.expect_token(TokenKind::LBrace)?;
        let (col_name, _) = self.expect_string()?;
        self.expect_token(TokenKind::Comma)?;
        let col_type      = self.parse_m_type()?;
        self.expect_token(TokenKind::RBrace)?;
        Ok((col_name, col_type))
    }

    fn parse_type_list(&mut self) -> ParseResult<Vec<(String, ColumnType)>> {
        self.expect_token(TokenKind::LBrace)?;
        // Single-pair shorthand: {"colName", type text}  (no inner braces)
        if matches!(self.peek_kind(), TokenKind::StringLit(_)) {
            let (col_name, _) = self.expect_string()?;
            self.expect_token(TokenKind::Comma)?;
            let col_type = self.parse_m_type()?;
            self.expect_token(TokenKind::RBrace)?;
            return Ok(vec![(col_name, col_type)]);
        }
        let mut pairs = vec![];
        while self.peek_kind() != &TokenKind::RBrace
            && self.peek_kind() != &TokenKind::Eof
        {
            pairs.push(self.parse_type_pair()?);
            if self.peek_kind() == &TokenKind::Comma {
                self.advance();
            }
        }
        self.expect_token(TokenKind::RBrace)?;
        Ok(pairs)
    }

    /// Parse a bare type list: `{type number, type text, ...}` (no column names).
    /// Used by `Table.ColumnsOfType`.
    fn parse_bare_type_list(&mut self) -> ParseResult<Vec<ColumnType>> {
        self.expect_token(TokenKind::LBrace)?;
        let mut types = vec![];
        while self.peek_kind() != &TokenKind::RBrace
            && self.peek_kind() != &TokenKind::Eof
        {
            types.push(self.parse_m_type()?);
            if self.peek_kind() == &TokenKind::Comma {
                self.advance();
            }
        }
        self.expect_token(TokenKind::RBrace)?;
        Ok(types)
    }

    fn parse_col_list(&mut self) -> ParseResult<Vec<String>> {
        self.expect_token(TokenKind::LBrace)?;
        let mut cols = vec![];
        while self.peek_kind() != &TokenKind::RBrace
            && self.peek_kind() != &TokenKind::Eof
        {
            let (name, _) = self.expect_string()?;
            cols.push(name);
            if self.peek_kind() == &TokenKind::Comma {
                self.advance();
            }
        }
        self.expect_token(TokenKind::RBrace)?;
        Ok(cols)
    }

    fn parse_rename_pair(&mut self) -> ParseResult<(String, String)> {
        self.expect_token(TokenKind::LBrace)?;
        let (old, _) = self.expect_string()?;
        self.expect_token(TokenKind::Comma)?;
        let (new, _) = self.expect_string()?;
        self.expect_token(TokenKind::RBrace)?;
        Ok((old, new))
    }

    fn parse_rename_list(&mut self) -> ParseResult<Vec<(String, String)>> {
        self.expect_token(TokenKind::LBrace)?;
        let mut pairs = vec![];
        while self.peek_kind() != &TokenKind::RBrace
            && self.peek_kind() != &TokenKind::Eof
        {
            pairs.push(self.parse_rename_pair()?);
            if self.peek_kind() == &TokenKind::Comma {
                self.advance();
            }
        }
        self.expect_token(TokenKind::RBrace)?;
        Ok(pairs)
    }

    fn parse_sort_order(&mut self) -> ParseResult<SortOrder> {
        let (order_kw, span) = self.expect_ident()?;
        if order_kw != "Order" {
            return Err(ParseError::UnknownSortOrder {
                got: order_kw,
                span,
            });
        }
        self.expect_token(TokenKind::Dot)?;
        let (direction, dir_span) = self.expect_ident()?;
        match direction.as_str() {
            "Ascending"  => Ok(SortOrder::Ascending),
            "Descending" => Ok(SortOrder::Descending),
            other => Err(ParseError::UnknownSortOrder {
                got:  other.into(),
                span: dir_span,
            }),
        }
    }

    fn parse_sort_pair(&mut self) -> ParseResult<(String, SortOrder)> {
        self.expect_token(TokenKind::LBrace)?;
        let (col, _) = self.expect_string()?;
        self.expect_token(TokenKind::Comma)?;
        let order    = self.parse_sort_order()?;
        self.expect_token(TokenKind::RBrace)?;
        Ok((col, order))
    }

    fn parse_sort_list(&mut self) -> ParseResult<Vec<(String, SortOrder)>> {
        self.expect_token(TokenKind::LBrace)?;
        let mut pairs = vec![];
        while self.peek_kind() != &TokenKind::RBrace
            && self.peek_kind() != &TokenKind::Eof
        {
            // Bare string → default Ascending; { "col", Order.X } → existing pair
            if matches!(self.peek_kind(), TokenKind::StringLit(_)) {
                let (col, _) = self.expect_string()?;
                pairs.push((col, SortOrder::Ascending));
            } else {
                pairs.push(self.parse_sort_pair()?);
            }
            if self.peek_kind() == &TokenKind::Comma {
                self.advance();
            }
        }
        self.expect_token(TokenKind::RBrace)?;
        Ok(pairs)
    }

    fn parse_integer(&mut self) -> ParseResult<i64> {
        let negative = if self.peek_kind() == &TokenKind::Minus {
            self.advance();
            true
        } else {
            false
        };
        let tok = self.advance();
        match tok.kind {
            TokenKind::IntLit(n) => Ok(if negative { -n } else { n }),
            other => Err(ParseError::UnexpectedToken {
                expected: "integer literal".into(),
                got:      other,
                span:     tok.span,
            }),
        }
    }

    /// Parse a record literal: `[Field = value, ...]`
    fn parse_record_lit(&mut self) -> ParseResult<Vec<(String, ExprNode)>> {
        self.expect_token(TokenKind::LBracket)?;
        let mut fields = vec![];
        while self.peek_kind() != &TokenKind::RBracket
            && self.peek_kind() != &TokenKind::Eof
        {
            let (name, _) = self.expect_ident()?;
            self.expect_token(TokenKind::Eq)?;
            let value = self.parse_expr()?;
            fields.push((name, value));
            if self.peek_kind() == &TokenKind::Comma {
                self.advance();
            }
        }
        self.expect_token(TokenKind::RBracket)?;
        Ok(fields)
    }

    /// Parse a list of record literals: `{[...], [...], ...}`
    fn parse_record_list(&mut self) -> ParseResult<Vec<Vec<(String, ExprNode)>>> {
        self.expect_token(TokenKind::LBrace)?;
        let mut records = vec![];
        while self.peek_kind() != &TokenKind::RBrace
            && self.peek_kind() != &TokenKind::Eof
        {
            records.push(self.parse_record_lit()?);
            if self.peek_kind() == &TokenKind::Comma {
                self.advance();
            }
        }
        self.expect_token(TokenKind::RBrace)?;
        Ok(records)
    }

    /// Parse one aggregate spec: `{"name", each expr, Type.X}` or `{"name", each expr}`
    fn parse_aggregate_triple(
        &mut self,
    ) -> ParseResult<(String, ExprNode, ColumnType)> {
        self.expect_token(TokenKind::LBrace)?;
        let (name, _) = self.expect_string()?;
        self.expect_token(TokenKind::Comma)?;
        let expr = if self.peek_kind() == &TokenKind::Each {
            self.parse_each_expr()?
        } else {
            self.parse_expr()?
        };
        // Type is optional — if no comma follows, default to Float
        let col_type = if self.peek_kind() == &TokenKind::Comma {
            self.advance();
            self.parse_m_type()?
        } else {
            ColumnType::Float
        };
        self.expect_token(TokenKind::RBrace)?;
        Ok((name, expr, col_type))
    }

    /// Parse a list of aggregate specs: `{{"name", each expr, Type.X}, ...}`
    fn parse_aggregate_list(
        &mut self,
    ) -> ParseResult<Vec<(String, ExprNode, ColumnType)>> {
        self.expect_token(TokenKind::LBrace)?;
        let mut triples = vec![];
        while self.peek_kind() != &TokenKind::RBrace
            && self.peek_kind() != &TokenKind::Eof
        {
            triples.push(self.parse_aggregate_triple()?);
            if self.peek_kind() == &TokenKind::Comma {
                self.advance();
            }
        }
        self.expect_token(TokenKind::RBrace)?;
        Ok(triples)
    }

    /// Parse a `JoinKind` enum value.
    fn parse_join_kind(&mut self) -> ParseResult<JoinKind> {
        let (ns, ns_span) = self.expect_ident()?;
        if ns != "JoinKind" {
            return Err(ParseError::UnexpectedToken {
                expected: "'JoinKind'".into(),
                got:      TokenKind::Ident(ns),
                span:     ns_span,
            });
        }
        self.expect_token(TokenKind::Dot)?;
        let (variant, variant_span) = self.expect_ident()?;
        match variant.as_str() {
            "Inner"     => Ok(JoinKind::Inner),
            "Left"      => Ok(JoinKind::Left),
            "Right"     => Ok(JoinKind::Right),
            "Full"      => Ok(JoinKind::Full),
            "LeftAnti"  => Ok(JoinKind::LeftAnti),
            "RightAnti" => Ok(JoinKind::RightAnti),
            other => Err(ParseError::UnexpectedToken {
                expected: "JoinKind variant (Inner/Left/Right/Full/LeftAnti/RightAnti)".into(),
                got:      TokenKind::Ident(other.into()),
                span:     variant_span,
            }),
        }
    }

/// Parse one transform pair: `{"col", each expr}`, `{"col", value}`, or `{"col", each expr, type}`
    fn parse_transform_pair(&mut self) -> ParseResult<(String, ExprNode, Option<ColumnType>)> {
        self.expect_token(TokenKind::LBrace)?;
        let (col, _) = self.expect_string()?;
        self.expect_token(TokenKind::Comma)?;
        let expr = if self.peek_kind() == &TokenKind::Each {
            self.parse_each_expr()? // returns Each(body)
        } else {
            self.parse_expr()?
        };
        // Optional type annotation
        let col_type = if self.peek_kind() == &TokenKind::Comma {
            self.advance();
            Some(self.parse_m_type()?)
        } else {
            None
        };
        self.expect_token(TokenKind::RBrace)?;
        Ok((col, expr, col_type))
    }

    /// Parse a list of step/table references: `{Source, OtherTable, ...}`
    fn parse_step_ref_list(&mut self) -> ParseResult<Vec<String>> {
        self.expect_token(TokenKind::LBrace)?;
        let mut refs = vec![];
        while self.peek_kind() != &TokenKind::RBrace
            && self.peek_kind() != &TokenKind::Eof
        {
            let (name, _) = self.expect_ident()?;
            refs.push(name);
            if self.peek_kind() == &TokenKind::Comma {
                self.advance();
            }
        }
        self.expect_token(TokenKind::RBrace)?;
        Ok(refs)
    }

    /// Parse a list of transform pairs: `{{"col", each expr}, ...}` or `{{"col", each expr, type}, ...}`
    /// Also handles the single-pair shorthand: `{"col", each expr}` (no inner braces).
    fn parse_transform_list(&mut self) -> ParseResult<Vec<(String, ExprNode, Option<ColumnType>)>> {
        self.expect_token(TokenKind::LBrace)?;
        // Single-pair shorthand: {"colName", each expr}  (no inner braces)
        if matches!(self.peek_kind(), TokenKind::StringLit(_)) {
            let (col, _) = self.expect_string()?;
            self.expect_token(TokenKind::Comma)?;
            let expr = if self.peek_kind() == &TokenKind::Each {
                self.parse_each_expr()?
            } else {
                self.parse_expr()?
            };
            let col_type = if self.peek_kind() == &TokenKind::Comma {
                self.advance();
                Some(self.parse_m_type()?)
            } else {
                None
            };
            self.expect_token(TokenKind::RBrace)?;
            return Ok(vec![(col, expr, col_type)]);
        }
        let mut pairs = vec![];
        while self.peek_kind() != &TokenKind::RBrace
            && self.peek_kind() != &TokenKind::Eof
        {
            pairs.push(self.parse_transform_pair()?);
            if self.peek_kind() == &TokenKind::Comma {
                self.advance();
            }
        }
        self.expect_token(TokenKind::RBrace)?;
        Ok(pairs)
    }

    // ── step body parser ──────────────────────────────────────────────────

    // ── step body parser ──────────────────────────────────────────────────────
    //
    // Parses one M function call step (the part AFTER the opening paren).
    // For every function registered in pq_grammar, we iterate its ArgKind hints
    // and parse each arg into a CallArg variant, building a Vec<CallArg>.
    // The result is always StepKind::FunctionCall { name, args } -- the only
    // exception is Excel.Workbook which produces StepKind::Source.
    fn parse_step_body(
        &mut self,
        namespace: &str,
        function:  &str,
        fn_span:   Span,
    ) -> ParseResult<StepKind> {
        let sig = lookup_function(namespace, function)
            .ok_or_else(|| ParseError::UnknownFunction {
                qualified: format!("{}.{}", namespace, function),
                span:      fn_span.clone(),
            })?;

        let mut args: Vec<CallArg> = Vec::with_capacity(sig.arg_hints.len());

        for (i, arg_kind) in sig.arg_hints.iter().enumerate() {
            let is_optional = matches!(
                arg_kind,
                ArgKind::OptValue
                | ArgKind::OptRecordLit
                | ArgKind::OptJoinKind
                | ArgKind::OptInteger
                | ArgKind::OptNullableBool
                | ArgKind::OptMissingField
                | ArgKind::OptCultureOrRecord
            );

            if is_optional {
                if self.peek_kind() == &TokenKind::RParen { break; }
                self.expect_token(TokenKind::Comma)?;
            } else if i > 0 {
                self.expect_token(TokenKind::Comma)?;
            }

            let arg: CallArg = match arg_kind {
                ArgKind::StepRef => {
                    let (name, _) = self.expect_ident()?;
                    CallArg::StepRef(name)
                }
                ArgKind::StepRefOrValue => {
                    let is_bare = matches!(self.peek_kind(), TokenKind::Ident(_))
                        && !matches!(self.peek_offset(1), TokenKind::Dot);
                    if is_bare {
                        let (name, _) = self.expect_ident()?;
                        CallArg::StepRef(name)
                    } else {
                        CallArg::Expr(self.parse_expr()?)
                    }
                }
                ArgKind::StringLit => {
                    let (s, _) = self.expect_string()?;
                    CallArg::Str(s)
                }
                ArgKind::TypeList => {
                    CallArg::TypeList(self.parse_type_list()?)
                }
                ArgKind::ColumnList => {
                    CallArg::ColList(self.parse_col_list()?)
                }
                ArgKind::ColumnListOrString => {
                    if matches!(self.peek_kind(), TokenKind::StringLit(_)) {
                        let (s, _) = self.expect_string()?;
                        CallArg::ColList(vec![s])
                    } else {
                        CallArg::ColList(self.parse_col_list()?)
                    }
                }
                ArgKind::RenameList => {
                    CallArg::RenameList(self.parse_rename_list()?)
                }
                ArgKind::SortList => {
                    CallArg::SortList(self.parse_sort_list()?)
                }
                ArgKind::EachExpr => {
                    let expr = if self.peek_kind() == &TokenKind::Each {
                        self.parse_each_expr()?
                    } else {
                        self.parse_expr()?
                    };
                    CallArg::Expr(expr)
                }
                ArgKind::Integer => {
                    CallArg::Int(self.parse_integer()?)
                }
                ArgKind::Value => {
                    CallArg::Expr(self.parse_expr()?)
                }
                ArgKind::RecordLit => {
                    let fields = self.parse_record_lit()?;
                    CallArg::Expr(ExprNode::new(Expr::Record(fields), fn_span.clone()))
                }
                ArgKind::RecordList => {
                    let recs = self.parse_record_list()?;
                    let items: Vec<ExprNode> = recs.into_iter()
                        .map(|fields| ExprNode::new(Expr::Record(fields), fn_span.clone()))
                        .collect();
                    CallArg::Expr(ExprNode::new(Expr::List(items), fn_span.clone()))
                }
                ArgKind::AggregateList => {
                    let triples = self.parse_aggregate_list()?;
                    let specs: Vec<AggregateSpec> = triples.into_iter()
                        .map(|(name, expression, col_type)| AggregateSpec { name, expression, col_type })
                        .collect();
                    CallArg::AggList(specs)
                }
                ArgKind::JoinKind => {
                    CallArg::JoinKindArg(self.parse_join_kind()?)
                }
                ArgKind::TransformList => {
                    CallArg::TransformList(self.parse_transform_list()?)
                }
                ArgKind::StepRefList => {
                    CallArg::StepRefList(self.parse_step_ref_list()?)
                }
                ArgKind::OptInteger => {
                    CallArg::OptInt(Some(self.parse_integer()?))
                }
                ArgKind::OptValue => {
                    CallArg::Expr(self.parse_expr()?)
                }
                ArgKind::OptRecordLit => {
                    let fields = self.parse_record_lit()?;
                    CallArg::Expr(ExprNode::new(Expr::Record(fields), fn_span.clone()))
                }
                ArgKind::OptJoinKind => {
                    CallArg::JoinKindArg(self.parse_join_kind()?)
                }
                ArgKind::FileContentsArg => {
                    self.expect_ident_named("File")?;
                    self.expect_token(TokenKind::Dot)?;
                    self.expect_ident_named("Contents")?;
                    self.expect_token(TokenKind::LParen)?;
                    let result = if matches!(self.peek_kind(), TokenKind::StringLit(_)) {
                        let (path, _) = self.expect_string()?;
                        CallArg::Str(path)
                    } else {
                        CallArg::Expr(self.parse_expr()?)
                    };
                    self.expect_token(TokenKind::RParen)?;
                    result
                }
                ArgKind::OptNullableBool => {
                    let val = match self.peek_kind() {
                        TokenKind::NullLit    => { self.advance(); None }
                        TokenKind::BoolLit(b) => { let b = *b; self.advance(); Some(b) }
                        other => {
                            let span = self.current_span();
                            return Err(ParseError::UnexpectedToken {
                                expected: "null or boolean".into(),
                                got:      other.clone(),
                                span,
                            });
                        }
                    };
                    CallArg::NullableBool(val)
                }
                ArgKind::OptMissingField => {
                    let (ns, span) = self.expect_ident()?;
                    if ns != "MissingField" {
                        return Err(ParseError::UnexpectedToken {
                            expected: "MissingField".into(),
                            got:      TokenKind::Ident(ns),
                            span,
                        });
                    }
                    self.expect_token(TokenKind::Dot)?;
                    let (variant, var_span) = self.expect_ident()?;
                    let mf = match variant.as_str() {
                        "Error"   => MissingFieldKind::Error,
                        "Ignore"  => MissingFieldKind::Ignore,
                        "UseNull" => MissingFieldKind::UseNull,
                        other => return Err(ParseError::UnexpectedToken {
                            expected: "Error, Ignore, or UseNull".into(),
                            got:      TokenKind::Ident(other.into()),
                            span:     var_span,
                        }),
                    };
                    CallArg::OptMissingField(Some(mf))
                }
                ArgKind::BareTypeList => {
                    CallArg::BareTypeList(self.parse_bare_type_list()?)
                }
                ArgKind::OptCultureOrRecord => {
                    let mut culture: Option<String> = None;
                    let mut missing: Option<MissingFieldKind> = None;
                    if matches!(self.peek_kind(), TokenKind::StringLit(_)) {
                        let (s, _) = self.expect_string()?;
                        culture = Some(s);
                    } else {
                        self.expect_token(TokenKind::LBracket)?;
                        while self.peek_kind() != &TokenKind::RBracket
                            && self.peek_kind() != &TokenKind::Eof
                        {
                            let (key, _) = self.expect_ident()?;
                            self.expect_token(TokenKind::Eq)?;
                            match key.as_str() {
                                "Culture" => {
                                    let (s, _) = self.expect_string()?;
                                    culture = Some(s);
                                }
                                "MissingField" => {
                                    let (ns, ns_span) = self.expect_ident()?;
                                    if ns != "MissingField" {
                                        return Err(ParseError::UnexpectedToken {
                                            expected: "MissingField".into(),
                                            got:      TokenKind::Ident(ns),
                                            span:     ns_span,
                                        });
                                    }
                                    self.expect_token(TokenKind::Dot)?;
                                    let (variant, var_span) = self.expect_ident()?;
                                    missing = Some(match variant.as_str() {
                                        "Error"   => MissingFieldKind::Error,
                                        "Ignore"  => MissingFieldKind::Ignore,
                                        "UseNull" => MissingFieldKind::UseNull,
                                        other => return Err(ParseError::UnexpectedToken {
                                            expected: "Error, Ignore, or UseNull".into(),
                                            got:      TokenKind::Ident(other.into()),
                                            span:     var_span,
                                        }),
                                    });
                                }
                                _ => { let _ = self.parse_expr()?; }
                            }
                            if self.peek_kind() == &TokenKind::Comma {
                                self.advance();
                            }
                        }
                        self.expect_token(TokenKind::RBracket)?;
                    }
                    CallArg::OptCulture(culture, missing)
                }
            };
            args.push(arg);
        }

        // Excel.Workbook is the only function that maps to StepKind::Source.
        // It uses FileContentsArg (->Str) + OptNullableBool x2.
        if namespace == "Excel" && function == "Workbook" {
            let path = args.iter().find_map(|a| a.as_str())
                .unwrap_or("").to_string();
            let bools: Vec<Option<bool>> = args.iter()
                .filter_map(|a| a.as_nullable_bool())
                .collect();
            return Ok(StepKind::Source {
                path,
                use_headers: bools.first().copied().flatten(),
                delay_types: bools.get(1).copied().flatten(),
            });
        }

        // Every other registered M function -> generic FunctionCall.
        let name = format!("{}.{}", namespace, function);
        Ok(StepKind::FunctionCall { name, args })
    }

    // ── top-level program parser ──────────────────────────────────────────

    fn parse_binding(&mut self) -> ParseResult<StepBinding> {
        let (name, name_span) = self.expect_ident()?;
        self.expect_token(TokenKind::Eq)?;

        // Detect workbook navigation: `Ident { [ ... ] } [ Ident ]`.
        // This is the standard sheet selector that follows a Source step,
        // e.g. `Sheet = Source{[Item="X", Kind="Sheet"]}[Data]`.
        if matches!(self.peek_kind(), TokenKind::Ident(_))
            && matches!(self.peek_offset(1), TokenKind::LBrace)
            && matches!(self.peek_offset(2), TokenKind::LBracket)
        {
            let (step_kind, step_span) = self.parse_navigate_sheet()?;
            let step                   = Step::new(step_kind, step_span);
            return Ok(StepBinding::new(name, name_span, step));
        }

        // Detect direct value bindings: `Name = <non-call expression>`.
        //
        // A binding is a value binding when the RHS does NOT start with
        // a function-call shape `Ident(.Ident)*(`.  Everything else —
        // literals, list / record literals, lambdas, identifier
        // references, parenthesised expressions, etc. — becomes a
        // [`StepKind::ValueBinding`].
        if Self::is_value_binding_start(self) {
            let expr = self.parse_expr()?;
            let span = expr.span.clone();
            return Ok(StepBinding::new(
                name,
                name_span,
                Step::new(StepKind::ValueBinding { expr }, span),
            ));
        }

        let (ns, func, fn_span) = self.parse_qualified_name()?;
        let lparen_span         = self.expect_token(TokenKind::LParen)?;
        let step_kind           = self.parse_step_body(&ns, &func, fn_span.clone())?;
        self.expect_token(TokenKind::RParen)?;
        let step_span           = fn_span.merge(&lparen_span);
        let step                = Step::new(step_kind, step_span);
        Ok(StepBinding::new(name, name_span, step))
    }

    /// Look-ahead heuristic: does the next token (and its successors) look
    /// like a function-call step (`Ident(.Ident)*(`)?  If not, the binding
    /// must be a value binding parsed as a generic expression.
    fn is_value_binding_start(p: &Self) -> bool {
        // Anything that is clearly not a function call can be parsed as
        // a value expression.
        match p.peek_kind() {
            // Literals
            TokenKind::IntLit(_)
            | TokenKind::FloatLit(_)
            | TokenKind::StringLit(_)
            | TokenKind::BoolLit(_)
            | TokenKind::NullLit
            // Compound value forms
            | TokenKind::LBrace          // list literal {...}
            | TokenKind::LBracket        // record literal [...]
            | TokenKind::Each            // each lambda
            | TokenKind::Minus           // unary -
            | TokenKind::Not             // unary not
                => true,

            // `(...)` could be a parenthesised expression OR an explicit
            // lambda `(x) => ...`. Either way it's a value expression.
            TokenKind::LParen => true,

            // For an identifier RHS, walk the qualified name and check
            // whether it terminates in `(` (call) or anything else (ref).
            TokenKind::Ident(_) => {
                let mut i = 1;
                loop {
                    match p.peek_offset(i) {
                        TokenKind::Dot => i += 2,             // skip `.Ident`
                        TokenKind::LParen => return false,    // call → existing path
                        _ => return true,                     // bare ref / op
                    }
                }
            }
            _ => false,
        }
    }

    /// Parse a workbook navigation expression of the form
    /// `Source{[Item="Sheet1", Kind="Sheet"]}[Data]`.
    ///
    /// The leading identifier names the upstream `Source` step. The braces
    /// hold a record literal whose `Item=` and `Kind=` fields select a single
    /// row from the navigation table; the trailing bracket field-access
    /// extracts the actual sheet table from that row.
    fn parse_navigate_sheet(&mut self) -> ParseResult<(StepKind, Span)> {
        let (input, input_span) = self.expect_ident()?;
        self.expect_token(TokenKind::LBrace)?;

        // Parse the inner record literal `[Field = "value", ...]`.
        self.expect_token(TokenKind::LBracket)?;
        let mut item       = String::new();
        let mut sheet_kind = String::new();
        while self.peek_kind() != &TokenKind::RBracket
            && self.peek_kind() != &TokenKind::Eof
        {
            let (fname, fspan) = self.expect_ident()?;
            self.expect_token(TokenKind::Eq)?;
            let (val, _)       = self.expect_string()?;
            match fname.as_str() {
                "Item" => item = val,
                "Kind" => sheet_kind = val,
                other  => return Err(ParseError::UnexpectedToken {
                    expected: "'Item' or 'Kind'".into(),
                    got:      TokenKind::Ident(other.to_string()),
                    span:     fspan,
                }),
            }
            if self.peek_kind() == &TokenKind::Comma {
                self.advance();
            }
        }
        self.expect_token(TokenKind::RBracket)?;
        self.expect_token(TokenKind::RBrace)?;

        // Trailing `[Data]` field access.
        self.expect_token(TokenKind::LBracket)?;
        let (field, _)   = self.expect_ident()?;
        let end_span     = self.expect_token(TokenKind::RBracket)?;
        let span         = input_span.merge(&end_span);

        Ok((StepKind::NavigateSheet { input, item, sheet_kind, field }, span))
    }

    pub fn parse(&mut self) -> ParseResult<Program> {
        self.expect_token(TokenKind::Let)?;
        let mut steps = vec![];
        loop {
            steps.push(self.parse_binding()?);
            match self.peek_kind() {
                TokenKind::Comma => { self.advance(); }
                TokenKind::In    => { self.advance(); break; }
                _ => return Err(ParseError::UnexpectedToken {
                    expected: "',' or 'in'".into(),
                    got:      self.peek_kind().clone(),
                    span:     self.current_span(),
                }),
            }
        }
        let in_expr = self.parse_expr()?;
        // Bare-identifier output keeps full back-compat: store the name
        // directly and leave `output_expr` as None.  Anything else
        // (FunctionCall, FieldAccess, arithmetic, etc.) is preserved in
        // `output_expr`; `output` becomes a placeholder.
        let (output, output_span, output_expr) = match &in_expr.expr {
            Expr::Identifier(name) => (name.clone(), in_expr.span.clone(), None),
            _                      => ("<expr>".to_string(), in_expr.span.clone(), Some(in_expr.clone())),
        };
        Ok(Program { steps, output, output_span, output_expr })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pq_lexer::Lexer;

    fn parse(input: &str) -> Program {
        let tokens = Lexer::new(input).tokenize().unwrap();
        Parser::new(tokens).parse().unwrap()
    }

    #[test]
    fn test_source_only() {
        let p = parse(r#"let Source = Excel.Workbook(File.Contents("file.xlsx"), null, true) in Source"#);
        assert_eq!(p.steps.len(), 1);
        assert_eq!(p.output, "Source");
    }

    #[test]
    fn test_promote_headers() {
        let p = parse(r#"
            let
                Source          = Excel.Workbook(File.Contents("file.xlsx"), null, true),
                PromotedHeaders = Table.PromoteHeaders(Source)
            in
                PromotedHeaders
        "#);
        assert_eq!(p.steps.len(), 2);
    }

    #[test]
    fn test_filter_step() {
        let p = parse(r#"
            let
                Source   = Excel.Workbook(File.Contents("file.xlsx"), null, true),
                Filtered = Table.SelectRows(Source, each Age > 25)
            in
                Filtered
        "#);
        assert_eq!(p.steps.len(), 2);
        // condition must be Lambda { param: "_", body: BinaryOp(...) }
        if let StepKind::FunctionCall { name, args } = &p.steps[1].step.kind {
            assert_eq!(name, "Table.SelectRows");
            if let Some(pq_ast::call_arg::CallArg::Expr(cond)) = args.get(1) {
                assert!(matches!(cond.expr, Expr::Lambda { .. }));
            } else { panic!("expected Expr arg"); }
        } else { panic!("expected FunctionCall"); }
    }

    #[test]
    fn test_filter_with_and() {
        let p = parse(r#"
            let
                Source   = Excel.Workbook(File.Contents("file.xlsx"), null, true),
                Filtered = Table.SelectRows(Source, each Age > 25 and Active = true)
            in
                Filtered
        "#);
        assert_eq!(p.steps.len(), 2);
    }

    #[test]
    fn test_filter_with_not() {
        let p = parse(r#"
            let
                Source   = Excel.Workbook(File.Contents("file.xlsx"), null, true),
                Filtered = Table.SelectRows(Source, each not Active)
            in
                Filtered
        "#);
        assert_eq!(p.steps.len(), 2);
    }

    #[test]
    fn test_add_column() {
        let p = parse(r#"
            let
                Source    = Excel.Workbook(File.Contents("file.xlsx"), null, true),
                WithBonus = Table.AddColumn(Source, "Bonus", each Salary + 1000.0)
            in
                WithBonus
        "#);
        assert_eq!(p.steps.len(), 2);
        if let StepKind::FunctionCall { name, args } = &p.steps[1].step.kind {
            assert_eq!(name, "Table.AddColumn");
            if let Some(pq_ast::call_arg::CallArg::Expr(expr)) = args.get(2) {
                assert!(matches!(expr.expr, Expr::Lambda { .. }));
            } else { panic!("expected Expr arg"); }
        } else { panic!("expected FunctionCall"); }
    }

    #[test]
    fn test_add_column_with_function_call() {
        let p = parse(r#"
            let
                Source  = Excel.Workbook(File.Contents("file.xlsx"), null, true),
                WithLen = Table.AddColumn(Source, "NameLen", each Text.Length([Name]))
            in
                WithLen
        "#);
        assert_eq!(p.steps.len(), 2);
    }

    #[test]
    fn test_remove_columns() {
        let p = parse(r#"
            let
                Source  = Excel.Workbook(File.Contents("file.xlsx"), null, true),
                Removed = Table.RemoveColumns(Source, {"Age", "Active"})
            in
                Removed
        "#);
        assert_eq!(p.steps.len(), 2);
    }

    #[test]
    fn test_rename_columns() {
        let p = parse(r#"
            let
                Source  = Excel.Workbook(File.Contents("file.xlsx"), null, true),
                Renamed = Table.RenameColumns(Source, {{"Age", "Years"}})
            in
                Renamed
        "#);
        assert_eq!(p.steps.len(), 2);
    }

    #[test]
    fn test_sort() {
        let p = parse(r#"
            let
                Source = Excel.Workbook(File.Contents("file.xlsx"), null, true),
                Sorted = Table.Sort(Source, {{"Age", Order.Ascending}})
            in
                Sorted
        "#);
        assert_eq!(p.steps.len(), 2);
    }

    #[test]
    fn test_transform_columns() {
        let p = parse(r#"
            let
                Source      = Excel.Workbook(File.Contents("file.xlsx"), null, true),
                Transformed = Table.TransformColumns(Source, {{"Age", each _ + 1}})
            in
                Transformed
        "#);
        assert_eq!(p.steps.len(), 2);
        assert!(matches!(&p.steps[1].step.kind, StepKind::FunctionCall { name, .. } if name == "Table.TransformColumns"));
    }

    #[test]
    fn test_group() {
        let p = parse(r#"
            let
                Source  = Excel.Workbook(File.Contents("file.xlsx"), null, true),
                Grouped = Table.Group(Source, {"Name"}, {{"Total", each List.Sum([Salary]), Number.Type}})
            in
                Grouped
        "#);
        assert_eq!(p.steps.len(), 2);
        assert!(matches!(&p.steps[1].step.kind, StepKind::FunctionCall { name, .. } if name == "Table.Group"));
    }

    #[test]
    fn test_column_access_bracket() {
        let p = parse(r#"
            let
                Source   = Excel.Workbook(File.Contents("file.xlsx"), null, true),
                Filtered = Table.SelectRows(Source, each [Age] > 25)
            in
                Filtered
        "#);
        if let StepKind::FunctionCall { name, args } = &p.steps[1].step.kind {
            assert_eq!(name, "Table.SelectRows");
            if let Some(pq_ast::call_arg::CallArg::Expr(cond)) = args.get(1) {
                if let Expr::Lambda { body: inner, .. } = &cond.expr {
                    if let Expr::BinaryOp { left, .. } = &inner.expr {
                        assert!(matches!(left.expr, Expr::ColumnAccess(_)));
                    } else { panic!("expected BinaryOp"); }
                } else { panic!("expected Lambda"); }
            } else { panic!("expected Expr arg"); }
        } else { panic!("expected FunctionCall"); }
    }

    #[test]
    fn test_null_literal() {
        let p = parse(r#"
            let
                Source   = Excel.Workbook(File.Contents("file.xlsx"), null, true),
                Filtered = Table.SelectRows(Source, each Age = null)
            in
                Filtered
        "#);
        assert_eq!(p.steps.len(), 2);
    }

    #[test]
    fn test_lambda_expression_in_add_column() {
        let p = parse(r#"
            let
                Source  = Excel.Workbook(File.Contents("file.xlsx"), null, true),
                WithCol = Table.AddColumn(Source, "Flag", each (x) => x > 0)
            in
                WithCol
        "#);
        assert_eq!(p.steps.len(), 2);
    }

    #[test]
    fn test_unknown_function_error() {
        let tokens = Lexer::new(
            r#"let X = Foo.Bar("a", "b") in X"#
        ).tokenize().unwrap();
        let result = Parser::new(tokens).parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_full_pipeline() {
        // ...existing code...
    }

    // ── StepRefOrValue (List.* with raw list literals) ────────────────────

    /// List.Transform with a raw list literal `{1,2,3}` as the first argument.
    /// This is the exact case the user reported as failing.
    #[test]
    fn test_list_transform_raw_list_literal() {
        let p = parse(r#"
            let
                Result = List.Transform({1, 2, 3}, each Text.Length(_))
            in
                Result
        "#);
        assert_eq!(p.steps.len(), 1);
        // Now produces FunctionCall(List.Transform).
        assert!(matches!(
            &p.steps[0].step.kind,
            StepKind::FunctionCall { name, .. } if name == "List.Transform"
        ));
    }

    /// List.Transform with a step reference as the first argument (old behaviour
    /// must still work after the StepRefOrValue change).
    #[test]
    fn test_list_transform_step_ref() {
        let p = parse(r#"
            let
                MyList = List.Range(1, 5),
                Result = List.Transform(MyList, each Text.Length(_))
            in
                Result
        "#);
        assert_eq!(p.steps.len(), 2);
        // Second step is List.Transform with list_expr = Identifier("MyList").
        if let StepKind::FunctionCall { name, args } = &p.steps[1].step.kind {
            assert_eq!(name, "List.Transform");
            let step_ref = args.get(0).and_then(|a| a.as_step_ref());
            assert_eq!(step_ref, Some("MyList"), "first arg should be StepRef(MyList)");
        } else {
            panic!("expected FunctionCall, got {:?}", p.steps[1].step.kind);
        }
    }

    /// List.Transform with a nested function call `List.Range(1, 5)` as the
    /// first argument — also rejected by the old StepRef-only parser.
    #[test]
    fn test_list_transform_nested_call() {
        let p = parse(r#"
            let
                Result = List.Transform(List.Range(1, 5), each Text.Length(_))
            in
                Result
        "#);
        assert_eq!(p.steps.len(), 1);
        // FunctionCall in list_expr position → List.Transform with a FunctionCall node.
        if let StepKind::FunctionCall { name, args } = &p.steps[0].step.kind {
            assert_eq!(name, "List.Transform");
            if let Some(pq_ast::call_arg::CallArg::Expr(list_expr)) = args.get(0) {
                assert!(matches!(&list_expr.expr, Expr::FunctionCall { name: ref n, .. } if n == "List.Range"));
            } else { panic!("expected Expr arg"); }
        } else {
            panic!("expected FunctionCall, got {:?}", p.steps[0].step.kind);
        }
    }

    /// List.Sum with an inline integer list.
    #[test]
    fn test_list_sum_raw_list() {
        let p = parse(r#"
            let Result = List.Sum({10, 20, 30}) in Result
        "#);
        assert_eq!(p.steps.len(), 1);
        assert!(matches!(
            &p.steps[0].step.kind,
            StepKind::FunctionCall { name, .. } if name == "List.Sum"
        ));
    }

    /// Text.Combine with an inline string list.
    #[test]
    fn test_text_combine_raw_list() {
        let p = parse(r#"
            let Result = Text.Combine({"a", "b", "c"}, "-") in Result
        "#);
        assert_eq!(p.steps.len(), 1);
        assert!(matches!(
            &p.steps[0].step.kind,
            StepKind::FunctionCall { name, .. } if name == "Text.Combine"
        ));
    }

    // ── Phase 3–5 new tests ──────────────────────────────────────────────

    #[test]
    fn test_bare_ascending_sort() {
        let p = parse(r#"
            let
                Source = Excel.Workbook(File.Contents("file.xlsx"), null, true),
                Sorted = Table.Sort(Source, {"Name", {"Age", Order.Descending}})
            in
                Sorted
        "#);
        if let StepKind::FunctionCall { name, args } = &p.steps[1].step.kind {
            assert_eq!(name, "Table.Sort");
            if let Some(pq_ast::call_arg::CallArg::SortList(by)) = args.get(1) {
                assert_eq!(by.len(), 2);
                assert_eq!(by[0], ("Name".into(), SortOrder::Ascending));
                assert_eq!(by[1], ("Age".into(), SortOrder::Descending));
            } else { panic!("expected SortList arg"); }
        } else {
            panic!("expected FunctionCall");
        }
    }

    #[test]
    fn test_2_element_aggregate() {
        let p = parse(r#"
            let
                Source  = Excel.Workbook(File.Contents("file.xlsx"), null, true),
                Grouped = Table.Group(Source, {"Name"}, {{"Total", each List.Sum([Salary])}})
            in
                Grouped
        "#);
        if let StepKind::FunctionCall { name, args } = &p.steps[1].step.kind {
            assert_eq!(name, "Table.Group");
            if let Some(pq_ast::call_arg::CallArg::AggList(aggregates)) = args.get(2) {
            assert_eq!(aggregates.len(), 1);
            // Default type should be Float
            assert_eq!(aggregates[0].col_type, ColumnType::Float);
            } else { panic!("expected AggList arg"); }
        } else {
            panic!("expected FunctionCall");
            panic!("expected FunctionCall");
        }
    }

    fn test_ampersand_concat_end_to_end() {
        let p = parse(r#"
            let
                Source  = Excel.Workbook(File.Contents("file.xlsx"), null, true),
                WithCol = Table.AddColumn(Source, "Full", each [First] & " " & [Last])
            in
                WithCol
        "#);
        assert_eq!(p.steps.len(), 2);
        if let StepKind::FunctionCall { name, args } = &p.steps[1].step.kind {
            assert_eq!(name, "Table.AddColumn");
            if let Some(pq_ast::call_arg::CallArg::Expr(expression)) = args.get(2) {
                assert!(matches!(expression.expr, Expr::Lambda { .. }));
            } else { panic!("expected Expr arg"); }
        } else { panic!("expected FunctionCall"); }
    }
    #[test]
    fn test_nested_join_dual_step_refs_col_lists() {
        let p = parse(r#"
            let
                Source = Excel.Workbook(File.Contents("file.xlsx"), null, true),
                Other  = Excel.Workbook(File.Contents("other.xlsx"), null, true),
                Joined = Table.NestedJoin(Source, {"ID"}, Other, {"ID"}, "Merged", JoinKind.Inner)
            in
                Joined
        "#);
        assert_eq!(p.steps.len(), 3);
        if let StepKind::FunctionCall { name, args } = &p.steps[2].step.kind {
            assert_eq!(name, "Table.NestedJoin");
            let left  = args.get(0).and_then(|a| a.as_step_ref()).unwrap_or("");
            let right = args.get(2).and_then(|a| a.as_step_ref()).unwrap_or("");
            let new_col = args.get(4).and_then(|a| a.as_str()).unwrap_or("");
            let left_keys = args.get(1).and_then(|a| a.as_col_list()).unwrap_or(&[]);
            let right_keys = args.get(3).and_then(|a| a.as_col_list()).unwrap_or(&[]);
            let join_kind = args.get(5).and_then(|a| a.as_join_kind());
            assert_eq!(left, "Source"); assert_eq!(right, "Other"); assert_eq!(new_col, "Merged");
            assert_eq!(left_keys, &["ID"]); assert_eq!(right_keys, &["ID"]);
            assert!(matches!(join_kind, Some(&pq_ast::step::JoinKind::Inner)));
        } else { panic!("expected FunctionCall"); }
    }

    #[test]
    fn test_type_number_keyword_form() {
        let p = parse(r#"
            let
                Source  = Excel.Workbook(File.Contents("file.xlsx"), null, true),
                Typed   = Table.TransformColumnTypes(Source, {{"Age", type number}})
            in
                Typed
        "#);
        if let StepKind::FunctionCall { name, args } = &p.steps[1].step.kind {
            assert_eq!(name, "Table.TransformColumnTypes");
            if let Some(pq_ast::call_arg::CallArg::TypeList(columns)) = args.get(1) {
                assert_eq!(columns[0].1, ColumnType::Float);
            } else { panic!("expected TypeList arg"); }
        } else {
            panic!("expected FunctionCall");
        }
    }
    #[test]
    fn test_explicit_lambda_in_each_position() {
        let p = parse(r#"
            let
                Source   = Excel.Workbook(File.Contents("file.xlsx"), null, true),
                Filtered = Table.SelectRows(Source, (row) => row > 0)
            in
                Filtered
        "#);
        if let StepKind::FunctionCall { name, args } = &p.steps[1].step.kind {
            assert_eq!(name, "Table.SelectRows");
            if let Some(CallArg::Expr(condition)) = args.get(1) {
                assert!(matches!(condition.expr, Expr::Lambda { .. }));
            } else {
                panic!("expected Expr arg for condition");
            }
        } else {
            panic!("expected FunctionCall");
        }
    }

    #[test]
    fn test_named_lambda_field_access() {
        // `(row) => row[Age] * 2` — the canonical named-parameter idiom.
        // The body must be parsed as BinaryOp(FieldAccess("Age"), Mul, IntLit(2)).
        let p = parse(r#"
            let
                Source = Excel.Workbook(File.Contents("file.xlsx"), null, true),
                Added  = Table.AddColumn(Source, "DoubleAge", (row) => row[Age] * 2)
            in
                Added
        "#);
        if let StepKind::FunctionCall { name, args } = &p.steps[1].step.kind {
            assert_eq!(name, "Table.AddColumn");
            let col_name = args.get(1).and_then(|a| a.as_str()).unwrap_or("");
            assert_eq!(col_name, "DoubleAge");
            if let Some(CallArg::Expr(expression)) = args.get(2) {
                if let Expr::Lambda { params, body } = &expression.expr {
                    assert_eq!(params, &["row"]);
                    assert!(matches!(body.expr, Expr::BinaryOp { .. }),
                        "body should be a BinaryOp, got {:?}", body.expr);
                    if let Expr::BinaryOp { left, .. } = &body.expr {
                        assert!(matches!(left.expr, Expr::FieldAccess { .. }),
                            "left side of * should be FieldAccess, got {:?}", left.expr);
                        if let Expr::FieldAccess { field, .. } = &left.expr {
                            assert_eq!(field, "Age");
                        }
                    }
                } else {
                    panic!("expected Lambda, got {:?}", expression.expr);
                }
            } else {
                panic!("expected Expr arg for expression");
            }
        } else {
            panic!("expected FunctionCall");
        }
    }

    #[test]
    fn test_3_element_transform_pair() {
        let p = parse(r#"
            let
                Source      = Excel.Workbook(File.Contents("file.xlsx"), null, true),
                Transformed = Table.TransformColumns(Source, {{"Age", each _ + 1, type number}})
            in
                Transformed
        "#);
        if let StepKind::FunctionCall { name, args } = &p.steps[1].step.kind {
            assert_eq!(name, "Table.TransformColumns");
            if let Some(CallArg::TransformList(transforms)) = args.get(1) {
                assert_eq!(transforms.len(), 1);
                assert_eq!(transforms[0].2, Some(ColumnType::Float));
            } else {
                panic!("expected TransformList arg");
            }
        } else {
            panic!("expected FunctionCall");
        }
    }

    #[test]
    fn test_aggregate_missing_comma_is_error() {
        // Missing comma between "Custom" and each — must fail to parse.
        let tokens = Lexer::new(r#"
            let
                Source = Excel.Workbook(File.Contents("file.xlsx"), null, true),
                Grouped = Table.Group(Source, {"Age"}, {
                    {"Custom" each List.Range(1, 5)}
                })
            in
                Grouped
        "#).tokenize().unwrap();
        let result = Parser::new(tokens).parse();
        assert!(result.is_err(), "missing comma after string in aggregate triple should be a parse error");
    }

    #[test]
    fn test_unknown_function_in_expression_is_error() {
        // List.Rage is not a registered function — must fail to parse.
        let tokens = Lexer::new(r#"
            let
                Source = Excel.Workbook(File.Contents("file.xlsx"), null, true),
                Grouped = Table.Group(Source, {"Age"}, {
                    {"Custom", each List.Rage(1, 5)}
                })
            in
                Grouped
        "#).tokenize().unwrap();
        let result = Parser::new(tokens).parse();
        assert!(result.is_err(), "List.Rage is not a valid function and should be a parse error");
    }

    #[test]
    fn test_list_select_inline() {
        let p = parse(r#"let X = List.Select({1, -3, 4, 9, -2}, each _ > 0) in X"#);
        assert!(matches!(&p.steps[0].step.kind, StepKind::FunctionCall { name, .. } if name == "List.Select"));
    }

    #[test]
    fn test_list_select_step_ref() {
        let p = parse(r#"
            let
                Source   = {1, 2, 3, 4, 5},
                Filtered = List.Select(Source, each _ > 2)
            in Filtered
        "#);
        assert!(matches!(&p.steps[1].step.kind, StepKind::FunctionCall { name, .. } if name == "List.Select"));
    }

    #[test]
    fn test_list_select_nested_call() {
        let p = parse(r#"let X = List.Select(List.Range({1,2,3,4,5}, 0, 3), each _ > 1) in X"#);
        assert!(matches!(&p.steps[0].step.kind, StepKind::FunctionCall { name, .. } if name == "List.Select"));
    }

    // ── List.Difference ─────────────────────────────────────────────────────

    #[test]
    fn test_list_difference_two_literals() {
        let p = parse(r#"let X = List.Difference({1, 2, 3, 4, 5}, {4, 5, 3}) in X"#);
        assert!(matches!(&p.steps[0].step.kind, StepKind::FunctionCall { name, .. } if name == "List.Difference"));
    }

    #[test]
    fn test_list_difference_with_each_criteria() {
        let p = parse(r#"let X = List.Difference({"a","B"}, {"A","b"}, each Text.Lower(_)) in X"#);
        assert!(matches!(&p.steps[0].step.kind, StepKind::FunctionCall { name, args, .. } if name == "List.Difference" && args.len() == 3));
    }

    #[test]
    fn test_list_difference_two_arg_lambda_criteria() {
        let p = parse(r#"let X = List.Difference({1,2,3}, {2}, (x, y) => x = y) in X"#);
        assert!(matches!(&p.steps[0].step.kind, StepKind::FunctionCall { name, args, .. } if name == "List.Difference" && args.len() == 3));
    }

    #[test]
    fn test_list_difference_nested_calls() {
        let p = parse(r#"let X = List.Difference(List.Range({1,2,3,4,5}, 0, 4), List.Distinct({2,2,4})) in X"#);
        assert!(matches!(&p.steps[0].step.kind, StepKind::FunctionCall { name, .. } if name == "List.Difference"));
    }

    // ── List.Intersect ──────────────────────────────────────────────────────

    #[test]
    fn test_list_intersect_inline_lists() {
        let p = parse(r#"let X = List.Intersect({{1, 2, 3}, {2, 3, 5}, {2, 3, 7}}) in X"#);
        assert!(matches!(&p.steps[0].step.kind, StepKind::FunctionCall { name, args, .. } if name == "List.Intersect" && args.len() == 1));
    }

    #[test]
    fn test_list_intersect_step_ref() {
        let p = parse(r#"
            let
                A = {1, 2, 3},
                B = {2, 3, 4},
                X = List.Intersect({A, B})
            in X
        "#);
        assert!(matches!(&p.steps[2].step.kind, StepKind::FunctionCall { name, .. } if name == "List.Intersect"));
    }

    #[test]
    fn test_list_intersect_with_each_criteria() {
        let p = parse(r#"let X = List.Intersect({{"a","B"}, {"A","b"}}, each Text.Lower(_)) in X"#);
        assert!(matches!(&p.steps[0].step.kind, StepKind::FunctionCall { name, args, .. } if name == "List.Intersect" && args.len() == 2));
    }

    #[test]
    fn test_list_intersect_nested_call() {
        let p = parse(r#"let X = List.Intersect(List.Distinct({1, 2, 2, 3})) in X"#);
        assert!(matches!(&p.steps[0].step.kind, StepKind::FunctionCall { name, .. } if name == "List.Intersect"));
    }
}
