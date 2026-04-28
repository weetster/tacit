//! Authoring-view parser: BPE-compact text → (Node, SidecarNode).
//!
//! Tracks a binding stack for DeBruijn index computation.
//! Projection rules: plans/candidates/authoring-bpe-compact.md § "Direction 1".

use std::fmt;

use tacit_canonical::ast::Node;

use crate::authoring::lex::{lex, LexError, Token};

/// A hole diagnostic emitted during recoverable parse errors.
#[derive(Debug, Clone)]
pub struct HoleDiag {
    pub diag_id: String,
    pub message: String,
}
use crate::sidecar::SidecarNode;

#[derive(Debug, Clone)]
pub enum ParseError {
    Lex(LexError),
    Structural(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Lex(e) => write!(f, "lex: {}", e),
            ParseError::Structural(s) => write!(f, "{}", s),
        }
    }
}

impl std::error::Error for ParseError {}

impl From<LexError> for ParseError {
    fn from(e: LexError) -> Self {
        ParseError::Lex(e)
    }
}

fn err<T>(msg: impl Into<String>) -> Result<T, ParseError> {
    Err(ParseError::Structural(msg.into()))
}

/// Parse authoring-view bytes into a typed AST node + parallel sidecar tree.
pub fn parse_authoring(input: &[u8]) -> Result<(Node, SidecarNode), ParseError> {
    let tokens = lex(input)?;
    let mut p = Parser {
        tokens,
        pos: 0,
        stack: Vec::new(),
        holes: Vec::new(),
    };
    let (node, sidecar) = if matches!(p.peek(), Some(Token::Module)) {
        p.parse_module()?
    } else {
        p.parse_expr()?
    };
    if p.pos != p.tokens.len() {
        return err(format!("trailing tokens after position {}", p.pos));
    }
    Ok((node, sidecar))
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    /// Binding stack: names in scope, innermost last.
    stack: Vec<String>,
    /// Holes emitted during recovery; callers may inspect these.
    pub holes: Vec<HoleDiag>,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) {
        self.pos += 1;
    }

    fn consume(&mut self, expected: &Token, what: &str) -> Result<(), ParseError> {
        match self.peek() {
            Some(t) if t == expected => {
                self.advance();
                Ok(())
            }
            Some(t) => err(format!("expected {} but got {:?}", what, t)),
            None => err(format!("expected {} but got end of input", what)),
        }
    }

    /// Skip tokens up to (but not consuming) the next `;` or `}` at depth 0.
    /// Used by the first pass of parse_rec to skip binding bodies.
    fn skip_to_delimiter(&mut self) -> Result<(), ParseError> {
        let mut depth = 0i32;
        loop {
            match self.peek() {
                Some(Token::LBrace) | Some(Token::LParen) => {
                    depth += 1;
                    self.advance();
                }
                Some(Token::RBrace) | Some(Token::RParen) if depth > 0 => {
                    depth -= 1;
                    self.advance();
                }
                Some(Token::RBrace) | Some(Token::Semicolon) if depth == 0 => return Ok(()),
                None => return err("unexpected end of input in rec block"),
                _ => {
                    self.advance();
                }
            }
        }
    }

    /// Skip tokens up to (but not consuming) the next `=` at depth 0.
    /// Used by the first pass of parse_rec to skip a binding's type annotation.
    fn skip_to_eq(&mut self) -> Result<(), ParseError> {
        let mut depth = 0i32;
        loop {
            match self.peek() {
                Some(Token::LBrace) | Some(Token::LParen) => {
                    depth += 1;
                    self.advance();
                }
                Some(Token::RBrace) | Some(Token::RParen) if depth > 0 => {
                    depth -= 1;
                    self.advance();
                }
                Some(Token::Eq) if depth == 0 => return Ok(()),
                None => return err("unexpected end of input in rec binding type"),
                _ => {
                    self.advance();
                }
            }
        }
    }

    /// Skip forward to (but not consuming) the next `;`, `}`, or EOF.
    /// Used for error recovery per ADR 0040.
    fn advance_to_sync(&mut self) {
        let mut depth = 0i32;
        loop {
            match self.peek() {
                Some(Token::LBrace) | Some(Token::LParen) => {
                    depth += 1;
                    self.advance();
                }
                Some(Token::RBrace) | Some(Token::RParen) if depth > 0 => {
                    depth -= 1;
                    self.advance();
                }
                Some(Token::RBrace) | Some(Token::Semicolon) if depth == 0 => return,
                None => return,
                _ => {
                    self.advance();
                }
            }
        }
    }

    /// Emit a `Hole` node at the current position and advance to the next sync point.
    /// Used in place of hard parse errors where recovery is possible (ADR 0040).
    fn recover_expr(&mut self, diag_id: &str, msg: &str) -> (Node, SidecarNode) {
        self.holes.push(HoleDiag {
            diag_id: diag_id.to_string(),
            message: msg.to_string(),
        });
        self.advance_to_sync();
        let hole = Node::Hole {
            diag_id: diag_id.to_string(),
            payload: Box::new(Node::Str {
                value: msg.to_string(),
            }),
        };
        (hole, SidecarNode::default())
    }

    fn consume_ident(&mut self, what: &str) -> Result<String, ParseError> {
        match self.peek().cloned() {
            Some(Token::Ident(name)) => {
                self.advance();
                Ok(name)
            }
            // Allow `_` as a bare identifier (for record field keys, etc.)
            Some(Token::Underscore) => {
                self.advance();
                Ok("_".to_string())
            }
            Some(t) => err(format!("expected {} but got {:?}", what, t)),
            None => err(format!("expected {} but got end of input", what)),
        }
    }

    /// Look up a name in the binding stack. Returns DeBruijn index (0 = innermost).
    fn lookup(&self, name: &str) -> Option<u64> {
        self.stack
            .iter()
            .rev()
            .position(|n| n == name)
            .map(|i| i as u64)
    }

    /// True if the next token can start an atomic expression (not a structural keyword).
    fn can_start_atom(&self) -> bool {
        matches!(
            self.peek(),
            Some(
                Token::Ident(_)
                    | Token::Int(_)
                    | Token::Str(_)
                    | Token::At
                    | Token::Underscore
                    | Token::LParen
                    | Token::LBrace
            )
        )
    }

    // -------------------------------------------------------------------------
    // Expression parsing
    // -------------------------------------------------------------------------

    pub fn parse_expr(&mut self) -> Result<(Node, SidecarNode), ParseError> {
        match self.peek() {
            Some(Token::Lambda) => self.parse_lambda(),
            Some(Token::Let) => self.parse_let(),
            Some(Token::Rec) => self.parse_rec(),
            Some(Token::If) => self.parse_if(),
            Some(Token::Match) => self.parse_match(),
            // `module` is only valid at the top level (dispatched from parse_authoring).
            // Appearing in expression position is a parse error; recover with a Hole.
            Some(Token::Module) => {
                let msg = "unexpected 'module' keyword in expression position";
                Ok(self.recover_expr("module-binding-error", msg))
            }
            _ => self.parse_app_expr(),
        }
    }

    fn parse_module(&mut self) -> Result<(Node, SidecarNode), ParseError> {
        self.consume(&Token::Module, "'module'")?;
        self.consume(&Token::LBrace, "'{'")?;

        // Empty module.
        if matches!(self.peek(), Some(Token::RBrace)) {
            self.advance();
            let sc = SidecarNode {
                binders: Some(vec![]),
                children: Some(vec![]),
                ..Default::default()
            };
            return Ok((Node::Module { bindings: vec![] }, sc));
        }

        // First pass: collect names, skip bodies (same two-pass pattern as parse_rec).
        let save_pos = self.pos;
        let mut names: Vec<String> = Vec::new();
        loop {
            match self.consume_ident("binding name") {
                Ok(name) => names.push(name),
                Err(_) => {
                    // Malformed binding name: skip to next `;`/`}` and stop collecting.
                    self.advance_to_sync();
                    break;
                }
            }
            if matches!(self.peek(), Some(Token::Colon)) {
                self.advance();
                self.skip_to_eq()?;
            }
            if !matches!(self.peek(), Some(Token::Eq)) {
                return err("expected '=' in module binding");
            }
            self.advance();
            self.skip_to_delimiter()?;
            if matches!(self.peek(), Some(Token::Semicolon)) {
                self.advance();
                if matches!(self.peek(), Some(Token::RBrace)) {
                    break; // trailing semicolon
                }
            } else {
                break;
            }
        }
        self.consume(&Token::RBrace, "'}' after last module binding")?;

        // Restore and push all names simultaneously (same convention as parse_rec).
        self.pos = save_pos;
        for name in names.iter().rev() {
            self.stack.push(name.clone());
        }

        // Second pass: parse binding expressions with all names in scope.
        let mut binding_nodes: Vec<Node> = Vec::new();
        let mut binding_scs: Vec<Option<SidecarNode>> = Vec::new();
        let n = names.len();

        for i in 0..n {
            let _name = self.consume_ident("binding name")?;
            let type_ann: Option<(Node, SidecarNode)> = if matches!(self.peek(), Some(Token::Colon))
            {
                self.advance();
                Some(self.parse_expr()?)
            } else {
                None
            };
            self.consume(&Token::Eq, "'='")?;
            let (expr, expr_sc) = self.parse_expr()?;

            let (final_node, binding_sc) = if let Some((t_node, t_sc)) = type_ann {
                let ann_sc = SidecarNode {
                    children: Some(vec![Some(expr_sc), Some(t_sc)]),
                    ..Default::default()
                };
                (
                    Node::Ann {
                        expr: Box::new(expr),
                        type_: Box::new(t_node),
                    },
                    ann_sc,
                )
            } else {
                (expr, expr_sc)
            };
            binding_nodes.push(final_node);
            binding_scs.push(Some(binding_sc));

            if i + 1 < n {
                self.consume(&Token::Semicolon, "';'")?;
            }
        }

        // Allow optional trailing semicolon.
        if matches!(self.peek(), Some(Token::Semicolon)) {
            self.advance();
        }
        self.consume(&Token::RBrace, "'}'")?;

        for _ in 0..n {
            self.stack.pop();
        }

        let sc = SidecarNode {
            binders: Some(names),
            children: Some(binding_scs),
            ..Default::default()
        };
        Ok((
            Node::Module {
                bindings: binding_nodes,
            },
            sc,
        ))
    }

    fn parse_lambda(&mut self) -> Result<(Node, SidecarNode), ParseError> {
        self.consume(&Token::Lambda, "'lambda'")?;
        let param = self.consume_ident("parameter name")?;
        self.consume(&Token::Dot, "'.'")?;
        self.stack.push(param.clone());
        let (body, body_sc) = self.parse_expr()?;
        self.stack.pop();
        let sc = SidecarNode {
            binder: Some(param),
            children: Some(vec![Some(body_sc)]),
            ..Default::default()
        };
        Ok((
            Node::Lam {
                body: Box::new(body),
            },
            sc,
        ))
    }

    fn parse_let(&mut self) -> Result<(Node, SidecarNode), ParseError> {
        self.consume(&Token::Let, "'let'")?;
        let binder = self.consume_ident("binder name")?;

        // Optional type annotation: let x: T = ...
        let type_ann = if matches!(self.peek(), Some(Token::Colon)) {
            self.advance();
            Some(self.parse_expr()?)
        } else {
            None
        };

        self.consume(&Token::Eq, "'='")?;
        let (rhs, rhs_sc) = self.parse_expr()?;
        self.consume(&Token::In, "'in'")?;
        self.stack.push(binder.clone());
        let (body, body_sc) = self.parse_expr()?;
        self.stack.pop();

        if let Some((type_node, type_sc)) = type_ann {
            // let x: T = V in B → Let(Ann(V, T), B)
            let ann_sc = SidecarNode {
                children: Some(vec![Some(rhs_sc), Some(type_sc)]),
                ..Default::default()
            };
            let ann = Node::Ann {
                expr: Box::new(rhs),
                type_: Box::new(type_node),
            };
            let sc = SidecarNode {
                binder: Some(binder),
                children: Some(vec![Some(ann_sc), Some(body_sc)]),
                ..Default::default()
            };
            Ok((
                Node::Let {
                    rhs: Box::new(ann),
                    body: Box::new(body),
                },
                sc,
            ))
        } else {
            let sc = SidecarNode {
                binder: Some(binder),
                children: Some(vec![Some(rhs_sc), Some(body_sc)]),
                ..Default::default()
            };
            Ok((
                Node::Let {
                    rhs: Box::new(rhs),
                    body: Box::new(body),
                },
                sc,
            ))
        }
    }

    fn parse_rec(&mut self) -> Result<(Node, SidecarNode), ParseError> {
        self.consume(&Token::Rec, "'rec'")?;
        self.consume(&Token::LBrace, "'{'")?;

        // First pass: scan to collect binding names without parsing expressions.
        // All names must be in scope simultaneously for the second pass.
        let save_pos = self.pos;
        let mut names: Vec<String> = Vec::new();
        loop {
            // Each binding starts with "name (: type)? =".
            let name = self.consume_ident("binding name")?;
            names.push(name);
            // Skip optional ": type".
            if matches!(self.peek(), Some(Token::Colon)) {
                self.advance();
                self.skip_to_eq()?; // skip type expr
            }
            // Expect "=", then skip binding body to ";" or "}".
            if !matches!(self.peek(), Some(Token::Eq)) {
                return err("expected '=' in rec binding");
            }
            self.advance();
            self.skip_to_delimiter()?; // skip binding expr
            if matches!(self.peek(), Some(Token::Semicolon)) {
                self.advance();
            } else {
                break;
            }
        }
        self.consume(&Token::RBrace, "'}' after last rec binding")?;

        // Restore and push all names simultaneously.
        // Push N-1 first, ..., 0 last so names[0] ends up at top (= var 0).
        self.pos = save_pos;
        for name in names.iter().rev() {
            self.stack.push(name.clone());
        }

        // Second pass: parse binding expressions with all names in scope.
        let mut binding_nodes: Vec<Node> = Vec::new();
        let mut binding_scs: Vec<Option<SidecarNode>> = Vec::new();
        let n = names.len();

        for i in 0..n {
            let _name = self.consume_ident("binding name")?;
            let type_ann: Option<(Node, SidecarNode)> = if matches!(self.peek(), Some(Token::Colon))
            {
                self.advance();
                Some(self.parse_expr()?)
            } else {
                None
            };
            self.consume(&Token::Eq, "'='")?;
            let (expr, expr_sc) = self.parse_expr()?;

            let (final_node, binding_sc) = if let Some((t_node, t_sc)) = type_ann {
                let ann_sc = SidecarNode {
                    children: Some(vec![Some(expr_sc), Some(t_sc.clone())]),
                    ..Default::default()
                };
                let node = Node::Ann {
                    expr: Box::new(expr),
                    type_: Box::new(t_node),
                };
                (node, ann_sc)
            } else {
                (expr, expr_sc)
            };
            binding_nodes.push(final_node);
            binding_scs.push(Some(binding_sc));

            if i + 1 < n {
                self.consume(&Token::Semicolon, "';'")?;
            }
        }

        self.consume(&Token::RBrace, "'}'")?;
        self.consume(&Token::In, "'in'")?;

        let (body, body_sc) = self.parse_expr()?;

        for _ in 0..n {
            self.stack.pop();
        }

        let mut children = binding_scs;
        children.push(Some(body_sc));

        let sc = SidecarNode {
            binders: Some(names),
            children: Some(children),
            ..Default::default()
        };
        Ok((
            Node::Rec {
                bindings: binding_nodes,
                body: Box::new(body),
            },
            sc,
        ))
    }

    fn parse_if(&mut self) -> Result<(Node, SidecarNode), ParseError> {
        self.consume(&Token::If, "'if'")?;
        let (cond, cond_sc) = self.parse_app_expr()?;
        self.consume(&Token::Then, "'then'")?;
        let (then, then_sc) = self.parse_app_expr()?;
        self.consume(&Token::Else, "'else'")?;
        let (else_, else_sc) = self.parse_expr()?;
        let sc = SidecarNode {
            children: Some(vec![Some(cond_sc), Some(then_sc), Some(else_sc)]),
            ..Default::default()
        };
        Ok((
            Node::If {
                cond: Box::new(cond),
                then: Box::new(then),
                else_: Box::new(else_),
            },
            sc,
        ))
    }

    fn parse_match(&mut self) -> Result<(Node, SidecarNode), ParseError> {
        self.consume(&Token::Match, "'match'")?;
        let (scrut, scrut_sc) = self.parse_app_expr()?;
        self.consume(&Token::With, "'with'")?;

        let mut arm_nodes: Vec<Node> = Vec::new();
        let mut arm_scs: Vec<Option<SidecarNode>> = Vec::new();

        while matches!(self.peek(), Some(Token::Pipe)) {
            let (arm_node, arm_sc) = self.parse_arm()?;
            arm_nodes.push(arm_node);
            arm_scs.push(Some(arm_sc));
        }
        if arm_nodes.is_empty() {
            return err("match requires at least one arm");
        }

        let mut children = vec![Some(scrut_sc)];
        children.extend(arm_scs);
        let sc = SidecarNode {
            children: Some(children),
            ..Default::default()
        };
        Ok((
            Node::Match {
                scrutinee: Box::new(scrut),
                arms: arm_nodes,
            },
            sc,
        ))
    }

    fn parse_arm(&mut self) -> Result<(Node, SidecarNode), ParseError> {
        self.consume(&Token::Pipe, "'|'")?;
        let (pat_node, pat_sc, pat_vars) = self.parse_pattern()?;
        self.consume(&Token::FatArrow, "'=>'")?;

        // Push pat-vars in textual order; last one ends up innermost (= DeBruijn 0).
        let save_len = self.stack.len();
        for name in &pat_vars {
            self.stack.push(name.clone());
        }
        let (body, body_sc) = self.parse_expr()?;
        self.stack.truncate(save_len);

        let sc = SidecarNode {
            children: Some(vec![Some(pat_sc), Some(body_sc)]),
            ..Default::default()
        };
        Ok((
            Node::Arm {
                pattern: Box::new(pat_node),
                body: Box::new(body),
            },
            sc,
        ))
    }

    // -------------------------------------------------------------------------
    // Application and atoms
    // -------------------------------------------------------------------------

    fn parse_app_expr(&mut self) -> Result<(Node, SidecarNode), ParseError> {
        // Use parse_proj_atom for the head so that `r.x.y` in head position works.
        let (head_node, head_sc, is_ctor) = self.parse_proj_atom()?;

        if is_ctor {
            let ctor_name = match &head_node {
                Node::Ctor { name, .. } => name.clone(),
                _ => unreachable!(),
            };
            let mut args: Vec<Node> = Vec::new();
            let mut arg_scs: Vec<Option<SidecarNode>> = Vec::new();
            while self.can_start_atom() {
                let (arg, sc, _) = self.parse_proj_atom()?;
                args.push(arg);
                arg_scs.push(Some(sc));
            }
            let sc = SidecarNode {
                children: if arg_scs.is_empty() {
                    None
                } else {
                    Some(arg_scs)
                },
                ..Default::default()
            };
            return Ok((
                Node::Ctor {
                    name: ctor_name,
                    args,
                },
                sc,
            ));
        }

        let mut lhs = head_node;
        let mut lhs_sc = head_sc;
        while self.can_start_atom() {
            let (rhs, rhs_sc, _) = self.parse_proj_atom()?;
            let sc = SidecarNode {
                children: Some(vec![Some(lhs_sc), Some(rhs_sc)]),
                ..Default::default()
            };
            lhs = Node::App {
                fn_: Box::new(lhs),
                arg: Box::new(rhs),
            };
            lhs_sc = sc;
        }
        Ok((lhs, lhs_sc))
    }

    /// Parse one atomic expression, then any `.field` projections.
    /// Returns (node, sidecar, is_ctor_head). is_ctor_head means caller collects args.
    fn parse_proj_atom(&mut self) -> Result<(Node, SidecarNode, bool), ParseError> {
        let (mut node, mut sc, is_ctor) = self.parse_head_atom()?;
        // Ctors don't take proj suffixes in the authoring view.
        if is_ctor {
            return Ok((node, sc, true));
        }
        // Projection: no spaces around `.` (compact form).
        while matches!(self.peek(), Some(Token::Dot)) {
            self.advance();
            let field = self.consume_ident("field name")?;
            sc = SidecarNode {
                children: Some(vec![Some(sc)]),
                ..Default::default()
            };
            node = Node::Proj {
                record: Box::new(node),
                field,
            };
        }
        Ok((node, sc, false))
    }

    /// Parse one atomic expression; returns (node, sidecar, is_ctor_head).
    /// is_ctor_head = true → caller is responsible for collecting ctor args.
    fn parse_head_atom(&mut self) -> Result<(Node, SidecarNode, bool), ParseError> {
        match self.peek().cloned() {
            Some(Token::Ident(name)) => {
                self.advance();
                if let Some(idx) = self.lookup(&name) {
                    Ok((Node::Var { index: idx }, SidecarNode::default(), false))
                } else if name.chars().next().is_some_and(|c| c.is_uppercase()) {
                    // Unbound capitalized ident → ctor head; args collected by caller.
                    Ok((
                        Node::Ctor { name, args: vec![] },
                        SidecarNode::default(),
                        true,
                    ))
                } else {
                    // Unbound lowercase ident → hole (per projection rules).
                    let msg = format!("identifier '{}' not in scope", name);
                    let payload = Node::Str { value: msg };
                    let node = Node::Hole {
                        diag_id: "unbound-name".to_string(),
                        payload: Box::new(payload),
                    };
                    Ok((node, SidecarNode::default(), false))
                }
            }
            Some(Token::Int(s)) => {
                self.advance();
                Ok((Node::int_from_decimal(&s), SidecarNode::default(), false))
            }
            Some(Token::Str(s)) => {
                self.advance();
                Ok((Node::Str { value: s }, SidecarNode::default(), false))
            }
            Some(Token::At) => {
                self.advance();
                let name = self.consume_ident("symbol name")?;
                Ok((Node::Sym { name }, SidecarNode::default(), false))
            }
            Some(Token::Underscore) => {
                self.advance();
                let payload = Node::Str {
                    value: "missing expression".to_string(),
                };
                let node = Node::Hole {
                    diag_id: "expected-expr".to_string(),
                    payload: Box::new(payload),
                };
                Ok((node, SidecarNode::default(), false))
            }
            Some(Token::LParen) => {
                self.advance();
                let (e, e_sc) = self.parse_expr()?;
                // Check for standalone ann: (E : T)
                if matches!(self.peek(), Some(Token::Colon)) {
                    self.advance();
                    let (t, t_sc) = self.parse_expr()?;
                    self.consume(&Token::RParen, "')'")?;
                    let sc = SidecarNode {
                        children: Some(vec![Some(e_sc), Some(t_sc)]),
                        ..Default::default()
                    };
                    return Ok((
                        Node::Ann {
                            expr: Box::new(e),
                            type_: Box::new(t),
                        },
                        sc,
                        false,
                    ));
                }
                self.consume(&Token::RParen, "')'")?;
                Ok((e, e_sc, false))
            }
            Some(Token::LBrace) => {
                let (node, sc) = self.parse_record()?;
                Ok((node, sc, false))
            }
            Some(t) => {
                let msg = format!("expected expression, got {:?}", t);
                self.advance();
                let (n, s) = self.recover_expr("unexpected-token", &msg);
                Ok((n, s, false))
            }
            None => {
                let (n, s) =
                    self.recover_expr("expected-expr", "expected expression, got end of input");
                Ok((n, s, false))
            }
        }
    }

    fn parse_record(&mut self) -> Result<(Node, SidecarNode), ParseError> {
        self.consume(&Token::LBrace, "'{'")?;
        let mut fields: Vec<(String, Node)> = Vec::new();
        let mut authoring_order: Vec<String> = Vec::new();
        let mut val_scs: Vec<SidecarNode> = Vec::new();

        if !matches!(self.peek(), Some(Token::RBrace)) {
            loop {
                let key = self.consume_ident("field name")?;
                self.consume(&Token::Colon, "':'")?;
                let (val, val_sc) = self.parse_expr()?;
                authoring_order.push(key.clone());
                fields.push((key, val));
                val_scs.push(val_sc);
                if matches!(self.peek(), Some(Token::Comma)) {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        self.consume(&Token::RBrace, "'}'")?;

        // Compute canonical order (alphabetical by field name bytes).
        let mut sorted_indices: Vec<usize> = (0..fields.len()).collect();
        sorted_indices.sort_by(|&a, &b| fields[a].0.as_bytes().cmp(fields[b].0.as_bytes()));

        // field_order[i] = canonical index at authoring position i.
        // sorted_indices[canonical_index] = authoring_index
        // We need: for authoring position i, which canonical index?
        // sorted_indices maps canonical pos → authoring pos.
        // We want authoring pos → canonical pos = inverse permutation.
        let mut field_order = vec![0usize; fields.len()];
        for (canon_pos, &auth_pos) in sorted_indices.iter().enumerate() {
            field_order[auth_pos] = canon_pos;
        }

        // Build canonical-order fields list.
        let canonical_fields: Vec<(String, Node)> =
            sorted_indices.iter().map(|&i| fields[i].clone()).collect();

        // Sidecar: 2N children (null for sym, val_sc for value) in canonical order.
        // field_order if not identity.
        let is_identity = field_order.iter().enumerate().all(|(i, &v)| i == v);
        let sidecar_field_order = if is_identity { None } else { Some(field_order) };

        let mut children: Vec<Option<SidecarNode>> = Vec::with_capacity(fields.len() * 2);
        for &auth_i in &sorted_indices {
            children.push(None); // sym entry: no metadata
            children.push(Some(val_scs[auth_i].clone()));
        }
        // Trim trailing None entries
        while children.last().is_some_and(|c| c.is_none()) {
            children.pop();
        }
        let children_opt = if children.is_empty() {
            None
        } else {
            Some(children)
        };

        let sc = SidecarNode {
            field_order: sidecar_field_order,
            children: children_opt,
            ..Default::default()
        };
        Ok((
            Node::Record {
                fields: canonical_fields,
            },
            sc,
        ))
    }

    // -------------------------------------------------------------------------
    // Pattern parsing
    // -------------------------------------------------------------------------

    /// True if next token can start an atomic pattern (including integer literals).
    fn can_start_pattern_atom(&self) -> bool {
        matches!(
            self.peek(),
            Some(Token::Ident(_) | Token::Underscore | Token::Int(_))
        )
    }

    /// Parse a full pattern. Returns (node, sidecar, pat_var_names_in_textual_order).
    fn parse_pattern(&mut self) -> Result<(Node, SidecarNode, Vec<String>), ParseError> {
        match self.peek().cloned() {
            Some(Token::Underscore) => {
                self.advance();
                Ok((Node::PatWild, SidecarNode::default(), vec![]))
            }
            Some(Token::Int(s)) => {
                // Integer literal in pattern position → pat-int (ADR 0037).
                self.advance();
                let value = if s == "-0" { "0".to_string() } else { s };
                Ok((Node::PatInt { value }, SidecarNode::default(), vec![]))
            }
            Some(Token::Ident(name)) if name.chars().next().is_some_and(|c| c.is_uppercase()) => {
                self.advance();
                let ctor_name = name;
                let mut sub_pats: Vec<Node> = Vec::new();
                let mut sub_scs: Vec<Option<SidecarNode>> = Vec::new();
                let mut all_vars: Vec<String> = Vec::new();

                while self.can_start_pattern_atom() {
                    let (p, p_sc, vars) = self.parse_pattern_atom()?;
                    sub_pats.push(p);
                    sub_scs.push(Some(p_sc));
                    all_vars.extend(vars);
                }

                // Sidecar: leading null for ctor-name sym, then sub-pattern entries.
                let mut children = vec![None];
                children.extend(sub_scs);
                let sc = SidecarNode {
                    children: Some(children),
                    ..Default::default()
                };
                Ok((
                    Node::PatCtor {
                        name: ctor_name,
                        sub_patterns: sub_pats,
                    },
                    sc,
                    all_vars,
                ))
            }
            Some(Token::Ident(name)) => {
                self.advance();
                // Lowercase ident → pat-var (creates new binding).
                let sc = SidecarNode {
                    binder: Some(name.clone()),
                    ..Default::default()
                };
                Ok((Node::PatVar, sc, vec![name]))
            }
            Some(t) => {
                // Unknown token in pattern position — recover with a Hole (ADR 0040).
                let msg = format!("expected pattern, got {:?}", t);
                let (hole, sc) = self.recover_expr("expected-pattern", &msg);
                Ok((hole, sc, vec![]))
            }
            None => {
                let (hole, sc) =
                    self.recover_expr("expected-pattern", "expected pattern, got end of input");
                Ok((hole, sc, vec![]))
            }
        }
    }

    /// Parse an atomic pattern (sub-pattern of a pat-ctor). No nested non-nullary ctors.
    fn parse_pattern_atom(&mut self) -> Result<(Node, SidecarNode, Vec<String>), ParseError> {
        match self.peek().cloned() {
            Some(Token::Underscore) => {
                self.advance();
                Ok((Node::PatWild, SidecarNode::default(), vec![]))
            }
            Some(Token::Int(s)) => {
                // Integer literal in atomic pattern position → pat-int (ADR 0037).
                self.advance();
                let value = if s == "-0" { "0".to_string() } else { s };
                Ok((Node::PatInt { value }, SidecarNode::default(), vec![]))
            }
            Some(Token::Ident(name)) if name.chars().next().is_some_and(|c| c.is_uppercase()) => {
                self.advance();
                let sc = SidecarNode {
                    children: Some(vec![None]),
                    ..Default::default()
                };
                Ok((
                    Node::PatCtor {
                        name,
                        sub_patterns: vec![],
                    },
                    sc,
                    vec![],
                ))
            }
            Some(Token::Ident(name)) => {
                self.advance();
                let sc = SidecarNode {
                    binder: Some(name.clone()),
                    ..Default::default()
                };
                Ok((Node::PatVar, sc, vec![name]))
            }
            Some(t) => {
                let msg = format!("expected pattern atom, got {:?}", t);
                self.advance();
                let (n, s) = self.recover_expr("unexpected-token", &msg);
                Ok((n, s, vec![]))
            }
            None => {
                let (n, s) = self.recover_expr(
                    "expected-pattern-atom",
                    "expected pattern atom, got end of input",
                );
                Ok((n, s, vec![]))
            }
        }
    }
}
