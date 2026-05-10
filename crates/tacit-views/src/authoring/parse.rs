//! Authoring-view parser: BPE-compact text → (Node, SidecarNode).
//!
//! Tracks a binding stack for DeBruijn index computation.
//! Projection rules: plans/candidates/authoring-bpe-compact.md § "Direction 1".

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use tacit_canonical::ast::Node;
use tacit_canonical::hash_node;

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
        top_aliases: BTreeSet::new(),
        import_aliases: BTreeMap::new(),
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
    /// Logical-module local aliases that should lower to hash refs after parsing.
    top_aliases: BTreeSet<String>,
    /// Logical-module import aliases already bound to external definition hashes.
    import_aliases: BTreeMap<String, String>,
    /// Holes emitted during recovery; callers may inspect these.
    pub holes: Vec<HoleDiag>,
}

#[derive(Clone)]
struct ModuleDefDraft {
    alias: String,
    visibility: Option<String>,
    sig: Node,
    body: Node,
}

struct ResolvedModuleDef {
    alias: String,
    visibility: Option<String>,
    hash: String,
    def: Node,
}

const MODULE_REF_PLACEHOLDER_PREFIX: &str = "__tacit_module_ref__";

fn module_ref_placeholder(alias: &str) -> String {
    format!("{}{}", MODULE_REF_PLACEHOLDER_PREFIX, alias)
}

fn module_ref_placeholder_alias(name: &str) -> Option<&str> {
    name.strip_prefix(MODULE_REF_PLACEHOLDER_PREFIX)
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

    fn consume_ident_keyword(&mut self, keyword: &str) -> bool {
        match self.peek() {
            Some(Token::Ident(name)) if name == keyword => {
                self.advance();
                true
            }
            _ => false,
        }
    }

    fn consume_ident_exact(&mut self, keyword: &str) -> Result<(), ParseError> {
        if self.consume_ident_keyword(keyword) {
            Ok(())
        } else {
            err(format!("expected '{}'", keyword))
        }
    }

    fn consume_hash(&mut self, what: &str) -> Result<String, ParseError> {
        match self.peek().cloned() {
            Some(Token::Hash(hash)) => {
                self.advance();
                Ok(hash)
            }
            Some(t) => err(format!("expected {} but got {:?}", what, t)),
            None => err(format!("expected {} but got end of input", what)),
        }
    }

    fn scan_module_decls(
        &mut self,
    ) -> Result<(BTreeSet<String>, BTreeMap<String, String>), ParseError> {
        let mut def_aliases = BTreeSet::new();
        let mut import_aliases = BTreeMap::new();
        let mut seen_value_aliases = BTreeSet::new();

        while !matches!(self.peek(), Some(Token::RBrace) | None) {
            if self.consume_ident_keyword("import") {
                let alias = self.consume_ident("import alias")?;
                if !seen_value_aliases.insert(alias.clone()) {
                    return err(format!("duplicate module alias '{}'", alias));
                }
                while !matches!(self.peek(), Some(Token::Hash(_))) {
                    if matches!(self.peek(), Some(Token::Semicolon | Token::RBrace) | None) {
                        return err("import declaration is missing blake3 hash");
                    }
                    self.advance();
                }
                let hash = self.consume_hash("definition hash")?;
                import_aliases.insert(alias, hash);
                self.skip_to_decl_end()?;
            } else if self.consume_ident_keyword("private") {
                let alias = self.consume_ident("definition alias")?;
                if !seen_value_aliases.insert(alias.clone()) {
                    return err(format!("duplicate module alias '{}'", alias));
                }
                def_aliases.insert(alias);
                self.skip_to_decl_end()?;
            } else if self.consume_ident_keyword("export") {
                let visibility = self.consume_ident("export visibility")?;
                if visibility != "public" && visibility != "package" {
                    return err("export visibility must be public or package");
                }
                let alias = self.consume_ident("definition alias")?;
                if !seen_value_aliases.insert(alias.clone()) {
                    return err(format!("duplicate module alias '{}'", alias));
                }
                def_aliases.insert(alias);
                self.skip_to_decl_end()?;
            } else {
                return err(format!(
                    "expected import, export, private, or '}}' in module but got {:?}",
                    self.peek()
                ));
            }

            if matches!(self.peek(), Some(Token::Semicolon)) {
                self.advance();
            }
        }

        Ok((def_aliases, import_aliases))
    }

    fn skip_to_decl_end(&mut self) -> Result<(), ParseError> {
        let mut depth = 0i32;
        loop {
            match self.peek() {
                Some(Token::LBrace) | Some(Token::LParen) => {
                    depth += 1;
                    self.advance();
                }
                Some(Token::RParen) if depth > 0 => {
                    depth -= 1;
                    self.advance();
                }
                Some(Token::RBrace) if depth > 0 => {
                    depth -= 1;
                    self.advance();
                }
                Some(Token::RBrace) | Some(Token::Semicolon) if depth == 0 => return Ok(()),
                None => return err("unexpected end of input in module declaration"),
                _ => self.advance(),
            }
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
                    | Token::Hash(_)
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
        if matches!(self.peek(), Some(Token::Ident(_))) {
            return self.parse_logical_module_after_keyword();
        }
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

    fn parse_logical_module_after_keyword(&mut self) -> Result<(Node, SidecarNode), ParseError> {
        let module_alias = self.consume_ident("module alias")?;
        self.consume(&Token::LBrace, "'{'")?;
        let body_start = self.pos;

        let (all_def_aliases, import_aliases) = self.scan_module_decls()?;
        self.pos = body_start;
        let old_top_aliases = std::mem::replace(&mut self.top_aliases, all_def_aliases);
        let old_import_aliases = std::mem::replace(&mut self.import_aliases, import_aliases);

        let mut imports = Vec::new();
        let mut import_alias_map = BTreeMap::new();
        let mut drafts = Vec::new();

        while !matches!(self.peek(), Some(Token::RBrace) | None) {
            if self.consume_ident_keyword("import") {
                let alias = self.consume_ident("import alias")?;
                self.consume(&Token::Colon, "':'")?;
                let sig_type = self.parse_type_expr()?;
                self.consume_ident_exact("from")?;
                let hash = self.consume_hash("definition hash")?;
                let sig = Node::Sig {
                    type_: Box::new(sig_type),
                    eval_eff: Box::new(Node::EffSet { atoms: vec![] }),
                };
                imports.push(Node::Import {
                    hash: hash.clone(),
                    sig: Box::new(sig),
                });
                import_alias_map.insert(hash, alias);
            } else if self.consume_ident_keyword("private") {
                let draft = self.parse_module_def_decl(None)?;
                drafts.push(draft);
            } else if self.consume_ident_keyword("export") {
                let visibility = self.consume_ident("export visibility")?;
                if visibility != "public" && visibility != "package" {
                    return err("export visibility must be public or package");
                }
                let draft = self.parse_module_def_decl(Some(visibility))?;
                drafts.push(draft);
            } else {
                return err(format!(
                    "expected import, export, private, or '}}' in module but got {:?}",
                    self.peek()
                ));
            }

            if matches!(self.peek(), Some(Token::Semicolon)) {
                self.advance();
            } else if !matches!(self.peek(), Some(Token::RBrace)) {
                return err("expected ';' or '}' after module declaration");
            }
        }
        self.consume(&Token::RBrace, "'}'")?;

        self.top_aliases = old_top_aliases;
        self.import_aliases = old_import_aliases;

        let resolved = resolve_module_defs(drafts)?;
        let mut defs = Vec::new();
        let mut exports = Vec::new();
        let mut definition_aliases = BTreeMap::new();
        let mut export_aliases = BTreeMap::new();

        for resolved_def in resolved {
            definition_aliases.insert(resolved_def.hash.clone(), resolved_def.alias.clone());
            if let Some(visibility) = resolved_def.visibility {
                exports.push(Node::Export {
                    visibility,
                    hash: resolved_def.hash.clone(),
                });
                export_aliases.insert(resolved_def.hash.clone(), resolved_def.alias.clone());
            }
            defs.push(resolved_def.def);
        }

        let sc = SidecarNode {
            module_alias: Some(module_alias),
            definition_aliases: Some(definition_aliases),
            import_aliases: Some(import_alias_map),
            export_aliases: Some(export_aliases),
            ..Default::default()
        };

        Ok((
            Node::Unit {
                imports,
                exports,
                defs,
            },
            sc,
        ))
    }

    fn parse_module_def_decl(
        &mut self,
        visibility: Option<String>,
    ) -> Result<ModuleDefDraft, ParseError> {
        let alias = self.consume_ident("definition alias")?;
        self.consume(&Token::Colon, "':'")?;
        let sig_type = self.parse_type_expr()?;
        self.consume(&Token::Eq, "'='")?;
        let (body, _body_sc) = self.parse_expr()?;
        Ok(ModuleDefDraft {
            alias,
            visibility,
            sig: Node::Sig {
                type_: Box::new(sig_type),
                eval_eff: Box::new(Node::EffSet { atoms: vec![] }),
            },
            body,
        })
    }

    fn parse_type_expr(&mut self) -> Result<Node, ParseError> {
        self.parse_fn_type()
    }

    fn parse_fn_type(&mut self) -> Result<Node, ParseError> {
        let left = self.parse_type_atom()?;
        if matches!(self.peek(), Some(Token::Arrow)) {
            self.advance();
            let ret = self.parse_fn_type()?;
            let eff = if matches!(self.peek(), Some(Token::Slash)) {
                self.advance();
                self.parse_effect_set()?
            } else {
                Node::EffSet { atoms: vec![] }
            };
            Ok(Node::FnTy {
                arg: Box::new(left),
                ret: Box::new(ret),
                eff: Box::new(eff),
            })
        } else {
            Ok(left)
        }
    }

    fn parse_effect_set(&mut self) -> Result<Node, ParseError> {
        self.consume(&Token::LBrace, "'{'")?;
        let mut atoms = Vec::new();
        if !matches!(self.peek(), Some(Token::RBrace)) {
            loop {
                atoms.push(self.consume_ident("effect atom")?);
                if matches!(self.peek(), Some(Token::Comma)) {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        self.consume(&Token::RBrace, "'}'")?;
        atoms.sort();
        Ok(Node::EffSet { atoms })
    }

    fn parse_type_atom(&mut self) -> Result<Node, ParseError> {
        match self.peek().cloned() {
            Some(Token::Ident(name)) => {
                self.advance();
                Ok(Node::Sym { name })
            }
            Some(Token::LParen) => {
                self.advance();
                let ty = self.parse_type_expr()?;
                self.consume(&Token::RParen, "')'")?;
                Ok(ty)
            }
            Some(Token::LBrace) => self.parse_record_type(),
            Some(t) => err(format!("expected type expression but got {:?}", t)),
            None => err("expected type expression but got end of input"),
        }
    }

    fn parse_record_type(&mut self) -> Result<Node, ParseError> {
        self.consume(&Token::LBrace, "'{'")?;
        let mut fields = Vec::new();
        if !matches!(self.peek(), Some(Token::RBrace)) {
            loop {
                let name = self.consume_ident("record type field")?;
                self.consume(&Token::Colon, "':'")?;
                let ty = self.parse_type_expr()?;
                fields.push((name, ty));
                if matches!(self.peek(), Some(Token::Comma)) {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        self.consume(&Token::RBrace, "'}'")?;
        fields.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
        Ok(Node::Record { fields })
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
                } else if let Some(hash) = self.import_aliases.get(&name) {
                    Ok((
                        Node::Ref { hash: hash.clone() },
                        SidecarNode::default(),
                        false,
                    ))
                } else if self.top_aliases.contains(&name) {
                    Ok((
                        Node::Sym {
                            name: module_ref_placeholder(&name),
                        },
                        SidecarNode::default(),
                        false,
                    ))
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
            Some(Token::Hash(hash)) => {
                self.advance();
                Ok((Node::Ref { hash }, SidecarNode::default(), false))
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

fn resolve_module_defs(drafts: Vec<ModuleDefDraft>) -> Result<Vec<ResolvedModuleDef>, ParseError> {
    let draft_map: BTreeMap<String, ModuleDefDraft> = drafts
        .into_iter()
        .map(|draft| (draft.alias.clone(), draft))
        .collect();
    let mut marks = BTreeMap::new();
    let mut resolved = BTreeMap::new();
    let mut order = Vec::new();

    for alias in draft_map.keys() {
        resolve_module_def(alias, &draft_map, &mut marks, &mut resolved, &mut order)?;
    }

    Ok(order
        .into_iter()
        .map(|alias| resolved.remove(&alias).expect("resolved in order"))
        .collect())
}

fn resolve_module_def(
    alias: &str,
    drafts: &BTreeMap<String, ModuleDefDraft>,
    marks: &mut BTreeMap<String, bool>,
    resolved: &mut BTreeMap<String, ResolvedModuleDef>,
    order: &mut Vec<String>,
) -> Result<String, ParseError> {
    if let Some(done) = marks.get(alias).copied() {
        if done {
            return Ok(resolved
                .get(alias)
                .map(|def| def.hash.clone())
                .unwrap_or_default());
        }
        return err(format!("cyclic module dependency involving '{}'", alias));
    }

    let draft = drafts
        .get(alias)
        .ok_or_else(|| ParseError::Structural(format!("unknown module alias '{}'", alias)))?
        .clone();
    marks.insert(alias.to_string(), false);

    let deps = module_local_deps(&draft.body, drafts);
    let mut dep_hashes = BTreeMap::new();
    for dep in deps {
        let dep_hash = resolve_module_def(&dep, drafts, marks, resolved, order)?;
        dep_hashes.insert(dep, dep_hash);
    }

    let body = replace_module_ref_placeholders(&draft.body, &dep_hashes)?;
    let def = Node::Def {
        sig: Box::new(draft.sig),
        body: Box::new(body),
    };
    let hash = hash_node(&def)
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();
    let resolved_def = ResolvedModuleDef {
        alias: draft.alias.clone(),
        visibility: draft.visibility,
        hash: hash.clone(),
        def,
    };
    resolved.insert(alias.to_string(), resolved_def);
    order.push(alias.to_string());
    marks.insert(alias.to_string(), true);
    Ok(hash)
}

fn module_local_deps(node: &Node, drafts: &BTreeMap<String, ModuleDefDraft>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    collect_module_local_deps(node, drafts, &mut out);
    out
}

fn collect_module_local_deps(
    node: &Node,
    drafts: &BTreeMap<String, ModuleDefDraft>,
    out: &mut BTreeSet<String>,
) {
    match node {
        Node::Sym { name } => {
            if let Some(alias) = module_ref_placeholder_alias(name) {
                if drafts.contains_key(alias) {
                    out.insert(alias.to_string());
                }
            }
        }
        _ => for_each_module_ref_child(node, |child| collect_module_local_deps(child, drafts, out)),
    }
}

fn replace_module_ref_placeholders(
    node: &Node,
    dep_hashes: &BTreeMap<String, String>,
) -> Result<Node, ParseError> {
    match node {
        Node::Sym { name } => {
            if let Some(alias) = module_ref_placeholder_alias(name) {
                let Some(hash) = dep_hashes.get(alias) else {
                    return err(format!("unresolved module alias '{}'", alias));
                };
                Ok(Node::Ref { hash: hash.clone() })
            } else {
                Ok(node.clone())
            }
        }
        Node::Lam { body } => Ok(Node::Lam {
            body: Box::new(replace_module_ref_placeholders(body, dep_hashes)?),
        }),
        Node::App { fn_, arg } => Ok(Node::App {
            fn_: Box::new(replace_module_ref_placeholders(fn_, dep_hashes)?),
            arg: Box::new(replace_module_ref_placeholders(arg, dep_hashes)?),
        }),
        Node::Let { rhs, body } => Ok(Node::Let {
            rhs: Box::new(replace_module_ref_placeholders(rhs, dep_hashes)?),
            body: Box::new(replace_module_ref_placeholders(body, dep_hashes)?),
        }),
        Node::Rec { bindings, body } => Ok(Node::Rec {
            bindings: bindings
                .iter()
                .map(|binding| replace_module_ref_placeholders(binding, dep_hashes))
                .collect::<Result<Vec<_>, _>>()?,
            body: Box::new(replace_module_ref_placeholders(body, dep_hashes)?),
        }),
        Node::If { cond, then, else_ } => Ok(Node::If {
            cond: Box::new(replace_module_ref_placeholders(cond, dep_hashes)?),
            then: Box::new(replace_module_ref_placeholders(then, dep_hashes)?),
            else_: Box::new(replace_module_ref_placeholders(else_, dep_hashes)?),
        }),
        Node::Match { scrutinee, arms } => Ok(Node::Match {
            scrutinee: Box::new(replace_module_ref_placeholders(scrutinee, dep_hashes)?),
            arms: arms
                .iter()
                .map(|arm| replace_module_ref_placeholders(arm, dep_hashes))
                .collect::<Result<Vec<_>, _>>()?,
        }),
        Node::Arm { pattern, body } => Ok(Node::Arm {
            pattern: pattern.clone(),
            body: Box::new(replace_module_ref_placeholders(body, dep_hashes)?),
        }),
        Node::Record { fields } => Ok(Node::Record {
            fields: fields
                .iter()
                .map(|(name, value)| {
                    Ok((
                        name.clone(),
                        replace_module_ref_placeholders(value, dep_hashes)?,
                    ))
                })
                .collect::<Result<Vec<_>, ParseError>>()?,
        }),
        Node::Proj { record, field } => Ok(Node::Proj {
            record: Box::new(replace_module_ref_placeholders(record, dep_hashes)?),
            field: field.clone(),
        }),
        Node::Ctor { name, args } => Ok(Node::Ctor {
            name: name.clone(),
            args: args
                .iter()
                .map(|arg| replace_module_ref_placeholders(arg, dep_hashes))
                .collect::<Result<Vec<_>, _>>()?,
        }),
        Node::Ann { expr, type_ } => Ok(Node::Ann {
            expr: Box::new(replace_module_ref_placeholders(expr, dep_hashes)?),
            type_: type_.clone(),
        }),
        Node::Def { sig, body } => Ok(Node::Def {
            sig: sig.clone(),
            body: Box::new(replace_module_ref_placeholders(body, dep_hashes)?),
        }),
        _ => Ok(node.clone()),
    }
}

fn for_each_module_ref_child(node: &Node, mut f: impl FnMut(&Node)) {
    match node {
        Node::Lam { body } => f(body),
        Node::App { fn_, arg } => {
            f(fn_);
            f(arg);
        }
        Node::Let { rhs, body } => {
            f(rhs);
            f(body);
        }
        Node::Rec { bindings, body } => {
            for binding in bindings {
                f(binding);
            }
            f(body);
        }
        Node::If { cond, then, else_ } => {
            f(cond);
            f(then);
            f(else_);
        }
        Node::Match { scrutinee, arms } => {
            f(scrutinee);
            for arm in arms {
                f(arm);
            }
        }
        Node::Arm { body, .. } => f(body),
        Node::Record { fields } => {
            for (_, value) in fields {
                f(value);
            }
        }
        Node::Proj { record, .. } => f(record),
        Node::Ctor { args, .. } => {
            for arg in args {
                f(arg);
            }
        }
        Node::Ann { expr, .. } => f(expr),
        _ => {}
    }
}
