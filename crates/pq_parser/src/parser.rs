use pq_diagnostics::Span;
use pq_grammar::functions::{lookup_function, lookup_qualified, ArgKind};
use pq_grammar::operators::{Operator, UnaryOp};
use pq_lexer::token::{Token, TokenKind};
use pq_ast::{
    expr::{Expr, ExprNode},
    step::{Step, StepKind, SortOrder, JoinKind, AggregateSpec},
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
    fn parse_transform_list(&mut self) -> ParseResult<Vec<(String, ExprNode, Option<ColumnType>)>> {
        self.expect_token(TokenKind::LBrace)?;
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

    fn parse_step_body(
        &mut self,
        namespace: &str,
        function:  &str,
        fn_span:   Span,
    ) -> ParseResult<StepKind> {
        let sig = lookup_function(namespace, function)
            .ok_or_else(|| ParseError::UnknownFunction {
                qualified: format!("{}.{}", namespace, function),
                span:      fn_span,
            })?;

        // Accumulators for the various argument kinds.
        let mut step_refs          = vec![];
        let mut str_args           = vec![];
        let mut type_list          = None;
        let mut col_lists: Vec<Vec<String>> = vec![];
        let mut rename_list        = None;
        let mut sort_list          = None;
        let mut each_exprs: Vec<ExprNode> = vec![];
        let mut int_args           = vec![];
        let mut value_args         = vec![];
        let mut record_lit         = None;
        let mut record_list        = None;
        let mut aggregate_list     = None;
        let mut join_kind          = None;
        let mut transform_list     = None;
        let mut step_ref_list      = vec![];
        let mut opt_int_args        = vec![];
        let mut opt_value_arg       = None::<ExprNode>;
        let mut opt_record_lit_arg  = None::<Vec<(String, ExprNode)>>;
        let mut opt_join_kind_arg   = None::<JoinKind>;
        let mut opt_null_bool_args  = vec![];   // for OptNullableBool

        for (i, arg_kind) in sig.arg_hints.iter().enumerate() {
            let is_optional = matches!(
                arg_kind,
                ArgKind::OptValue
                | ArgKind::OptRecordLit
                | ArgKind::OptJoinKind
                | ArgKind::OptInteger
                | ArgKind::OptNullableBool
            );

            if is_optional {
                if self.peek_kind() == &TokenKind::RParen {
                    break;
                }
                self.expect_token(TokenKind::Comma)?;
            } else if i > 0 {
                self.expect_token(TokenKind::Comma)?;
            }

            match arg_kind {
                ArgKind::StepRef => {
                    let (name, _) = self.expect_ident()?;
                    step_refs.push(name);
                }
                // A list-function argument that can be either:
                //   • a bare step-name identifier (e.g. `MyList`)  → stored as step_ref
                //   • any expression (e.g. `{1,2,3}`, `List.Range(1,5)`) → stored in value_args
                //
                // Disambiguation: a bare step ref is an Ident whose next token is
                // NOT a dot (which would make it a qualified name / dotted expression).
                // Everything else is parsed as a full expression.
                ArgKind::StepRefOrValue => {
                    let is_bare_step_ref =
                        matches!(self.peek_kind(), TokenKind::Ident(_))
                        && !matches!(self.peek_offset(1), TokenKind::Dot);
                    if is_bare_step_ref {
                        let (name, _) = self.expect_ident()?;
                        step_refs.push(name);
                    } else {
                        value_args.push(self.parse_expr()?);
                    }
                }
                ArgKind::StringLit => {
                    let (s, _) = self.expect_string()?;
                    str_args.push(s);
                }
                ArgKind::TypeList => {
                    type_list = Some(self.parse_type_list()?);
                }
                ArgKind::ColumnList => {
                    col_lists.push(self.parse_col_list()?);
                }
                ArgKind::RenameList => {
                    rename_list = Some(self.parse_rename_list()?);
                }
                ArgKind::SortList => {
                    sort_list = Some(self.parse_sort_list()?);
                }
                ArgKind::EachExpr => {
                    if self.peek_kind() == &TokenKind::Each {
                        each_exprs.push(self.parse_each_expr()?);
                    } else {
                        each_exprs.push(self.parse_expr()?);
                    }
                }
                ArgKind::Integer => {
                    int_args.push(self.parse_integer()?);
                }
                ArgKind::Value => {
                    value_args.push(self.parse_expr()?);
                }
                ArgKind::RecordLit => {
                    record_lit = Some(self.parse_record_lit()?);
                }
                ArgKind::RecordList => {
                    record_list = Some(self.parse_record_list()?);
                }
                ArgKind::AggregateList => {
                    aggregate_list = Some(self.parse_aggregate_list()?);
                }
                ArgKind::JoinKind => {
                    join_kind = Some(self.parse_join_kind()?);
                }
                ArgKind::TransformList => {
                    transform_list = Some(self.parse_transform_list()?);
                }
                ArgKind::StepRefList => {
                    step_ref_list = self.parse_step_ref_list()?;
                }
                ArgKind::OptInteger => {
                    opt_int_args.push(self.parse_integer()?);
                }
                ArgKind::OptValue => {
                    opt_value_arg = Some(self.parse_expr()?);
                }
                ArgKind::OptRecordLit => {
                    opt_record_lit_arg = Some(self.parse_record_lit()?);
                }
                ArgKind::OptJoinKind => {
                    opt_join_kind_arg = Some(self.parse_join_kind()?);
                }
                ArgKind::FileContentsArg => {
                    // Parses:  File . Contents ( "path" )  or  File . Contents ( ident )
                    self.expect_ident_named("File")?;
                    self.expect_token(TokenKind::Dot)?;
                    self.expect_ident_named("Contents")?;
                    self.expect_token(TokenKind::LParen)?;
                    if matches!(self.peek_kind(), TokenKind::StringLit(_)) {
                        let (path, _) = self.expect_string()?;
                        str_args.push(path);
                    } else {
                        value_args.push(self.parse_expr()?);
                    }
                    self.expect_token(TokenKind::RParen)?;
                }
                ArgKind::OptNullableBool => {
                    // null → None, true/false → Some(bool)
                    let val = match self.peek_kind() {
                        TokenKind::NullLit => { self.advance(); None }
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
                    opt_null_bool_args.push(val);
                }
            }
        }

        // Suppress unused-variable warnings for args not yet wired to StepKind variants.
        let _ = (&int_args, &value_args, &record_lit, &record_list,
                 &join_kind, &opt_int_args, &opt_value_arg,
                 &opt_record_lit_arg, &opt_join_kind_arg);

        let kind = match (namespace, function) {
            ("Excel", "Workbook") => StepKind::Source {
                path:        str_args.remove(0),
                use_headers: opt_null_bool_args.first().copied().flatten(),
                delay_types: opt_null_bool_args.get(1).copied().flatten(),
            },
            ("Table", "PromoteHeaders") => StepKind::PromoteHeaders {
                input: step_refs.remove(0),
            },
            ("Table", "TransformColumnTypes") => StepKind::ChangeTypes {
                input:   step_refs.remove(0),
                columns: type_list.unwrap(),
            },
            ("Table", "SelectRows") => StepKind::Filter {
                input:     step_refs.remove(0),
                condition: each_exprs.remove(0),
            },
            ("Table", "AddColumn") => StepKind::AddColumn {
                input:      step_refs.remove(0),
                col_name:   str_args.remove(0),
                expression: each_exprs.remove(0),
            },
            ("Table", "RemoveColumns") => StepKind::RemoveColumns {
                input:   step_refs.remove(0),
                columns: col_lists.remove(0),
            },
            ("Table", "RenameColumns") => StepKind::RenameColumns {
                input:   step_refs.remove(0),
                renames: rename_list.unwrap(),
            },
            ("Table", "Sort") => StepKind::Sort {
                input: step_refs.remove(0),
                by:    sort_list.unwrap(),
            },
            ("Table", "TransformColumns") => StepKind::TransformColumns {
                input:      step_refs.remove(0),
                transforms: transform_list.unwrap_or_default(),
            },
            ("Table", "Group") => StepKind::Group {
                input:      step_refs.remove(0),
                by:         col_lists.pop().unwrap_or_default(),
                aggregates: aggregate_list
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(name, expr, col_type)| AggregateSpec { name, expression: expr, col_type })
                    .collect(),
            },

            // ── Simple row operations ────────────────────────────────────
            ("Table", "FirstN") => StepKind::FirstN {
                input: step_refs.remove(0),
                count: value_args.pop().or_else(|| int_args.pop().map(|n| ExprNode::new(Expr::IntLit(n), Span::dummy())))
                    .unwrap_or_else(|| ExprNode::new(Expr::IntLit(1), Span::dummy())),
            },
            ("Table", "LastN") => StepKind::LastN {
                input: step_refs.remove(0),
                count: value_args.pop().or_else(|| int_args.pop().map(|n| ExprNode::new(Expr::IntLit(n), Span::dummy())))
                    .unwrap_or_else(|| ExprNode::new(Expr::IntLit(1), Span::dummy())),
            },
            ("Table", "Skip") => StepKind::Skip {
                input: step_refs.remove(0),
                count: value_args.pop().or_else(|| int_args.pop().map(|n| ExprNode::new(Expr::IntLit(n), Span::dummy())))
                    .unwrap_or_else(|| ExprNode::new(Expr::IntLit(1), Span::dummy())),
            },
            ("Table", "Range") => {
                let input  = step_refs.remove(0);
                let offset = int_args.pop().map(|n| ExprNode::new(Expr::IntLit(n), Span::dummy()))
                    .unwrap_or_else(|| ExprNode::new(Expr::IntLit(0), Span::dummy()));
                let count  = int_args.pop().map(|n| ExprNode::new(Expr::IntLit(n), Span::dummy()))
                    .unwrap_or_else(|| ExprNode::new(Expr::IntLit(1), Span::dummy()));
                StepKind::Range { input, offset, count }
            },
            ("Table", "RemoveFirstN") => StepKind::RemoveFirstN {
                input: step_refs.remove(0),
                count: value_args.pop().or_else(|| int_args.pop().map(|n| ExprNode::new(Expr::IntLit(n), Span::dummy())))
                    .unwrap_or_else(|| ExprNode::new(Expr::IntLit(1), Span::dummy())),
            },
            ("Table", "RemoveLastN") => StepKind::RemoveLastN {
                input: step_refs.remove(0),
                count: value_args.pop().or_else(|| int_args.pop().map(|n| ExprNode::new(Expr::IntLit(n), Span::dummy())))
                    .unwrap_or_else(|| ExprNode::new(Expr::IntLit(1), Span::dummy())),
            },
            ("Table", "RemoveRows") => {
                let input  = step_refs.remove(0);
                let offset = int_args.pop().map(|n| ExprNode::new(Expr::IntLit(n), Span::dummy()))
                    .unwrap_or_else(|| ExprNode::new(Expr::IntLit(0), Span::dummy()));
                let count  = int_args.pop().map(|n| ExprNode::new(Expr::IntLit(n), Span::dummy()))
                    .unwrap_or_else(|| ExprNode::new(Expr::IntLit(1), Span::dummy()));
                StepKind::RemoveRows { input, offset, count }
            },
            ("Table", "ReverseRows") => StepKind::ReverseRows {
                input: step_refs.remove(0),
            },
            ("Table", "Distinct") => StepKind::Distinct {
                input:   step_refs.remove(0),
                columns: col_lists.pop().unwrap_or_default(),
            },
            ("Table", "Repeat") => StepKind::Repeat {
                input: step_refs.remove(0),
                count: int_args.pop().map(|n| ExprNode::new(Expr::IntLit(n), Span::dummy()))
                    .unwrap_or_else(|| ExprNode::new(Expr::IntLit(1), Span::dummy())),
            },
            ("Table", "AlternateRows") => {
                let input  = step_refs.remove(0);
                let offset = int_args.pop().map(|n| ExprNode::new(Expr::IntLit(n), Span::dummy()))
                    .unwrap_or_else(|| ExprNode::new(Expr::IntLit(0), Span::dummy()));
                let skip   = int_args.pop().map(|n| ExprNode::new(Expr::IntLit(n), Span::dummy()))
                    .unwrap_or_else(|| ExprNode::new(Expr::IntLit(1), Span::dummy()));
                let take   = int_args.pop().map(|n| ExprNode::new(Expr::IntLit(n), Span::dummy()))
                    .unwrap_or_else(|| ExprNode::new(Expr::IntLit(1), Span::dummy()));
                StepKind::AlternateRows { input, offset, skip, take }
            },
            ("Table", "FindText") => StepKind::FindText {
                input: step_refs.remove(0),
                text:  str_args.remove(0),
            },
            ("Table", "FillDown") => StepKind::FillDown {
                input:   step_refs.remove(0),
                columns: col_lists.remove(0),
            },
            ("Table", "FillUp") => StepKind::FillUp {
                input:   step_refs.remove(0),
                columns: col_lists.remove(0),
            },
            ("Table", "AddIndexColumn") => {
                let input    = step_refs.remove(0);
                let col_name = str_args.remove(0);
                let start    = opt_int_args.first().copied().unwrap_or(0);
                let step     = opt_int_args.get(1).copied().unwrap_or(1);
                StepKind::AddIndexColumn { input, col_name, start, step }
            },
            ("Table", "DuplicateColumn") => StepKind::DuplicateColumn {
                input:   step_refs.remove(0),
                src_col: str_args.remove(0),
                new_col: str_args.remove(0),
            },
            ("Table", "Unpivot") => StepKind::Unpivot {
                input:    step_refs.remove(0),
                columns:  col_lists.remove(0),
                attr_col: str_args.remove(0),
                val_col:  str_args.remove(0),
            },
            ("Table", "UnpivotOtherColumns") => StepKind::UnpivotOtherColumns {
                input:     step_refs.remove(0),
                keep_cols: col_lists.remove(0),
                attr_col:  str_args.remove(0),
                val_col:   str_args.remove(0),
            },
            ("Table", "Transpose") => StepKind::Transpose {
                input: step_refs.remove(0),
            },
            ("Table", "Combine") => StepKind::CombineTables {
                inputs: step_ref_list.clone(),
            },
            ("Table", "RemoveRowsWithErrors") => StepKind::RemoveRowsWithErrors {
                input:   step_refs.remove(0),
                columns: col_lists.pop().unwrap_or_default(),
            },
            ("Table", "SelectRowsWithErrors") => StepKind::SelectRowsWithErrors {
                input:   step_refs.remove(0),
                columns: col_lists.pop().unwrap_or_default(),
            },
            ("Table", "TransformRows") => StepKind::TransformRows {
                input:     step_refs.remove(0),
                transform: each_exprs.remove(0),
            },
            ("Table", "MatchesAllRows") => StepKind::MatchesAllRows {
                input:     step_refs.remove(0),
                condition: each_exprs.remove(0),
            },
            ("Table", "MatchesAnyRows") => StepKind::MatchesAnyRows {
                input:     step_refs.remove(0),
                condition: each_exprs.remove(0),
            },
            ("Table", "PrefixColumns") => StepKind::PrefixColumns {
                input:  step_refs.remove(0),
                prefix: str_args.remove(0),
            },
            ("Table", "DemoteHeaders") => StepKind::DemoteHeaders {
                input: step_refs.remove(0),
            },

            // ── Column operations ────────────────────────────────────────
            ("Table", "SelectColumns") => StepKind::SelectColumns {
                input:   step_refs.remove(0),
                columns: col_lists.remove(0),
            },
            ("Table", "ReorderColumns") => StepKind::ReorderColumns {
                input:   step_refs.remove(0),
                columns: col_lists.remove(0),
            },
            ("Table", "TransformColumnNames") => StepKind::TransformColumnNames {
                input:     step_refs.remove(0),
                transform: each_exprs.remove(0),
            },
            ("Table", "CombineColumns") => StepKind::CombineColumns {
                input:    step_refs.remove(0),
                columns:  col_lists.remove(0),
                combiner: each_exprs.remove(0),
                new_col:  str_args.remove(0),
            },
            ("Table", "SplitColumn") => StepKind::SplitColumn {
                input:    step_refs.remove(0),
                col_name: str_args.remove(0),
                splitter: each_exprs.remove(0),
            },
            ("Table", "ExpandTableColumn") => StepKind::ExpandTableColumn {
                input:    step_refs.remove(0),
                col_name: str_args.remove(0),
                columns:  col_lists.remove(0),
            },
            ("Table", "ExpandRecordColumn") => StepKind::ExpandRecordColumn {
                input:    step_refs.remove(0),
                col_name: str_args.remove(0),
                fields:   col_lists.remove(0),
            },
            ("Table", "Pivot") => StepKind::Pivot {
                input:     step_refs.remove(0),
                pivot_col: col_lists.remove(0),
                attr_col:  str_args.remove(0),
                val_col:   str_args.remove(0),
            },

            // ── Information functions ────────────────────────────────────
            ("Table", "RowCount") | ("Table", "ApproximateRowCount") => StepKind::RowCount {
                input: step_refs.remove(0),
            },
            ("Table", "ColumnCount") => StepKind::ColumnCount {
                input: step_refs.remove(0),
            },
            ("Table", "ColumnNames") => StepKind::TableColumnNames {
                input: step_refs.remove(0),
            },
            ("Table", "IsEmpty") => StepKind::TableIsEmpty {
                input: step_refs.remove(0),
            },
            ("Table", "Schema") => StepKind::TableSchema {
                input: step_refs.remove(0),
            },

            // ── Membership functions ─────────────────────────────────────
            ("Table", "HasColumns") => StepKind::HasColumns {
                input:   step_refs.remove(0),
                columns: col_lists.remove(0),
            },
            ("Table", "IsDistinct") => StepKind::TableIsDistinct {
                input: step_refs.remove(0),
            },

            // ── Joins ────────────────────────────────────────────────────
            ("Table", "Join") | ("Table", "FuzzyJoin") => StepKind::Join {
                left:       step_refs.remove(0),
                left_keys:  col_lists.remove(0),
                right:      step_refs.remove(0),
                right_keys: col_lists.remove(0),
                join_kind:  join_kind.unwrap_or(JoinKind::Inner),
            },
            ("Table", "NestedJoin") | ("Table", "FuzzyNestedJoin") => StepKind::NestedJoin {
                left:       step_refs.remove(0),
                left_keys:  col_lists.remove(0),
                right:      step_refs.remove(0),
                right_keys: col_lists.remove(0),
                new_col:    str_args.remove(0),
                join_kind:  join_kind.unwrap_or(JoinKind::Inner),
            },
            ("Table", "AddJoinColumn") => StepKind::NestedJoin {
                left:       step_refs.remove(0),
                left_keys:  col_lists.remove(0),
                right:      step_refs.remove(0),
                right_keys: col_lists.remove(0),
                new_col:    str_args.remove(0),
                join_kind:  opt_join_kind_arg.unwrap_or(JoinKind::Left),
            },

            // ── Ordering ─────────────────────────────────────────────────
            ("Table", "AddRankColumn") => StepKind::AddRankColumn {
                input:    step_refs.remove(0),
                col_name: str_args.remove(0),
                by:       sort_list.unwrap_or_default(),
            },
            ("Table", "Max") => StepKind::TableMax {
                input:    step_refs.remove(0),
                col_name: str_args.remove(0),
            },
            ("Table", "Min") => StepKind::TableMin {
                input:    step_refs.remove(0),
                col_name: str_args.remove(0),
            },
            ("Table", "MaxN") => StepKind::TableMaxN {
                input:    step_refs.remove(0),
                count:    int_args.pop().map(|n| ExprNode::new(Expr::IntLit(n), Span::dummy()))
                    .unwrap_or_else(|| ExprNode::new(Expr::IntLit(1), Span::dummy())),
                col_name: str_args.remove(0),
            },
            ("Table", "MinN") => StepKind::TableMinN {
                input:    step_refs.remove(0),
                count:    int_args.pop().map(|n| ExprNode::new(Expr::IntLit(n), Span::dummy()))
                    .unwrap_or_else(|| ExprNode::new(Expr::IntLit(1), Span::dummy())),
                col_name: str_args.remove(0),
            },

            // ── Value operations ─────────────────────────────────────────
            ("Table", "ReplaceValue") => StepKind::ReplaceValue {
                input:     step_refs.remove(0),
                old_value: value_args.remove(0),
                new_value: value_args.remove(0),
                replacer:  each_exprs.remove(0),
            },
            ("Table", "ReplaceErrorValues") => StepKind::ReplaceErrorValues {
                input:        step_refs.remove(0),
                replacements: transform_list.unwrap_or_default(),
            },
            ("Table", "InsertRows") => StepKind::InsertRows {
                input:  step_refs.remove(0),
                offset: int_args.pop().unwrap_or(0),
            },
            // List.Transform(list_or_step, each fn) ──────────────────────
            // list_expr may be an inline literal, a nested call, or a step ref.
            ("List", "Transform") => {
                let list_expr = if !step_refs.is_empty() {
                    ExprNode::new(Expr::Identifier(step_refs.remove(0)), Span::dummy())
                } else {
                    value_args.remove(0)
                };
                StepKind::ListTransform {
                    list_expr,
                    transform: each_exprs.remove(0),
                }
            }
            // List.Generate(initial, condition, next, optional selector)
            // initial  — Value / any expr (incl. zero-param lambda () => x)
            // condition — EachExpr (predicate)
            // next      — EachExpr (step function)
            // selector  — OptValue (optional projection)
            ("List", "Generate") => {
                let initial   = value_args.remove(0);
                let condition = each_exprs.remove(0);
                let next      = each_exprs.remove(0);
                let selector  = opt_value_arg;
                StepKind::ListGenerate { initial, condition, next, selector }
            }
            _ => StepKind::Passthrough {
                input: step_refs.pop()
                    .or_else(|| step_ref_list.first().cloned())
                    .unwrap_or_default(),
                func_name: format!("{}.{}", namespace, function),
            },
        };

        Ok(kind)
    }

    // ── top-level program parser ──────────────────────────────────────────

    fn parse_binding(&mut self) -> ParseResult<StepBinding> {
        let (name, name_span)   = self.expect_ident()?;
        self.expect_token(TokenKind::Eq)?;
        let (ns, func, fn_span) = self.parse_qualified_name()?;
        let lparen_span         = self.expect_token(TokenKind::LParen)?;
        let step_kind           = self.parse_step_body(&ns, &func, fn_span.clone())?;
        self.expect_token(TokenKind::RParen)?;
        let step_span           = fn_span.merge(&lparen_span);
        let step                = Step::new(step_kind, step_span);
        Ok(StepBinding::new(name, name_span, step))
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
        let (output, output_span) = self.expect_ident()?;
        Ok(Program { steps, output, output_span })
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
        let cond = match &p.steps[1].step.kind {
            StepKind::Filter { condition, .. } => condition,
            _ => panic!("expected Filter"),
        };
        assert!(matches!(cond.expr, Expr::Lambda { .. }));
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
        let expr = match &p.steps[1].step.kind {
            StepKind::AddColumn { expression, .. } => expression,
            _ => panic!("expected AddColumn"),
        };
        assert!(matches!(expr.expr, Expr::Lambda { .. }));
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
        assert!(matches!(p.steps[1].step.kind, StepKind::TransformColumns { .. }));
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
        assert!(matches!(p.steps[1].step.kind, StepKind::Group { .. }));
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
        let cond = match &p.steps[1].step.kind {
            StepKind::Filter { condition, .. } => condition,
            _ => panic!(),
        };
        if let Expr::Lambda { body: inner, .. } = &cond.expr {
            if let Expr::BinaryOp { left, .. } = &inner.expr {
                assert!(matches!(left.expr, Expr::ColumnAccess(_)));
            } else { panic!("expected BinaryOp"); }
        } else { panic!("expected Lambda"); }
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
        // Now produces ListTransform, not Passthrough.
        assert!(matches!(
            &p.steps[0].step.kind,
            StepKind::ListTransform { .. }
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
        // Second step is ListTransform with list_expr = Identifier("MyList").
        if let StepKind::ListTransform { list_expr, .. } = &p.steps[1].step.kind {
            assert!(matches!(&list_expr.expr, Expr::Identifier(name) if name == "MyList"));
        } else {
            panic!("expected ListTransform, got {:?}", p.steps[1].step.kind);
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
        // FunctionCall in list_expr position → ListTransform with a FunctionCall node.
        if let StepKind::ListTransform { list_expr, .. } = &p.steps[0].step.kind {
            assert!(matches!(&list_expr.expr, Expr::FunctionCall { name, .. } if name == "List.Range"));
        } else {
            panic!("expected ListTransform, got {:?}", p.steps[0].step.kind);
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
            StepKind::Passthrough { func_name, .. } if func_name == "List.Sum"
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
            StepKind::Passthrough { func_name, .. } if func_name == "Text.Combine"
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
        if let StepKind::Sort { by, .. } = &p.steps[1].step.kind {
            assert_eq!(by.len(), 2);
            assert_eq!(by[0], ("Name".into(), SortOrder::Ascending));
            assert_eq!(by[1], ("Age".into(), SortOrder::Descending));
        } else {
            panic!("expected Sort");
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
        if let StepKind::Group { aggregates, .. } = &p.steps[1].step.kind {
            assert_eq!(aggregates.len(), 1);
            // Default type should be Float
            assert_eq!(aggregates[0].col_type, ColumnType::Float);
        } else {
            panic!("expected Group");
        }
    }

    #[test]
    fn test_ampersand_concat_end_to_end() {
        let p = parse(r#"
            let
                Source  = Excel.Workbook(File.Contents("file.xlsx"), null, true),
                WithCol = Table.AddColumn(Source, "Full", each [First] & " " & [Last])
            in
                WithCol
        "#);
        assert_eq!(p.steps.len(), 2);
        if let StepKind::AddColumn { expression, .. } = &p.steps[1].step.kind {
            assert!(matches!(expression.expr, Expr::Lambda { .. }));
        } else {
            panic!("expected AddColumn");
        }
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
        assert!(matches!(
            &p.steps[2].step.kind,
            StepKind::NestedJoin { left, left_keys, right, right_keys, new_col, join_kind }
                if left == "Source" && right == "Other" && new_col == "Merged"
                && left_keys == &["ID"] && right_keys == &["ID"]
                && matches!(join_kind, pq_ast::step::JoinKind::Inner)
        ));
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
        if let StepKind::ChangeTypes { columns, .. } = &p.steps[1].step.kind {
            assert_eq!(columns[0].1, ColumnType::Float);
        } else {
            panic!("expected ChangeTypes");
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
        if let StepKind::Filter { condition, .. } = &p.steps[1].step.kind {
            assert!(matches!(condition.expr, Expr::Lambda { .. }));
        } else {
            panic!("expected Filter");
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
        if let StepKind::AddColumn { expression, col_name, .. } = &p.steps[1].step.kind {
            assert_eq!(col_name, "DoubleAge");
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
            panic!("expected AddColumn");
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
        if let StepKind::TransformColumns { transforms, .. } = &p.steps[1].step.kind {
            assert_eq!(transforms.len(), 1);
            assert_eq!(transforms[0].2, Some(ColumnType::Float));
        } else {
            panic!("expected TransformColumns");
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
}

