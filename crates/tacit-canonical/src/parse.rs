//! Parser for canonical text. Bytes → typed AST.
//!
//! Two passes:
//!   1. Tokens → nested s-expression items (SList / SAtom).
//!   2. Items → typed AST, validating tag, arity, and child shape.
//!
//! Rejections surface as ParseError (structural) or LexError (lexical).

use std::fmt;

use crate::ast::Node;
use crate::lex::{lex, LexError, Token};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Structural(String),
    Lex(LexError),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Structural(m) => write!(f, "{}", m),
            ParseError::Lex(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for ParseError {}

impl From<LexError> for ParseError {
    fn from(e: LexError) -> Self {
        ParseError::Lex(e)
    }
}

fn err<T, S: Into<String>>(msg: S) -> Result<T, ParseError> {
    Err(ParseError::Structural(msg.into()))
}

#[derive(Debug, Clone)]
enum SItem {
    Atom(Token),
    List(Vec<SItem>),
}

pub fn parse(data: &[u8]) -> Result<Node, ParseError> {
    let tokens = lex(data)?;
    let (item, pos) = read_item(&tokens, 0)?;
    if pos != tokens.len() {
        return err(format!("trailing tokens after position {}", pos));
    }
    build(&item)
}

fn read_item(tokens: &[Token], mut pos: usize) -> Result<(SItem, usize), ParseError> {
    if pos >= tokens.len() {
        return err("unexpected end of input");
    }
    match &tokens[pos] {
        Token::LParen => {
            pos += 1;
            let mut items = Vec::new();
            while pos < tokens.len() && tokens[pos] != Token::RParen {
                let (child, np) = read_item(tokens, pos)?;
                items.push(child);
                pos = np;
            }
            if pos >= tokens.len() {
                return err("unclosed '('");
            }
            Ok((SItem::List(items), pos + 1))
        }
        Token::RParen => err(format!("unexpected ')' at token {}", pos)),
        other => Ok((SItem::Atom(other.clone()), pos + 1)),
    }
}

fn build(item: &SItem) -> Result<Node, ParseError> {
    match item {
        SItem::Atom(tok) => match tok {
            Token::Sym(name) => match name.as_str() {
                "pat-wild" => Ok(Node::PatWild),
                "pat-var" => Ok(Node::PatVar),
                _ => err(format!("bare symbol {:?} is not an expression", name)),
            },
            _ => err(format!("bare atom {:?} is not an expression", tok)),
        },
        SItem::List(items) => {
            if items.is_empty() {
                return err("empty list");
            }
            let head = &items[0];
            let tag = match head {
                SItem::Atom(Token::Sym(s)) => s.as_str(),
                _ => return err("list head must be a tag symbol"),
            };
            build_form(tag, &items[1..])
        }
    }
}

fn check_arity(tag: &str, args: &[SItem], expected: usize) -> Result<(), ParseError> {
    if args.len() != expected {
        err(format!(
            "{} expects arity {}, got {}",
            tag,
            expected,
            args.len()
        ))
    } else {
        Ok(())
    }
}

fn bare_sym(item: &SItem, what: &str) -> Result<String, ParseError> {
    if let SItem::Atom(Token::Sym(s)) = item {
        Ok(s.clone())
    } else {
        err(format!("{} must be a bare symbol", what))
    }
}

fn bare_int(item: &SItem, what: &str) -> Result<String, ParseError> {
    if let SItem::Atom(Token::Int(s)) = item {
        Ok(s.clone())
    } else {
        err(format!("{} must be a decimal integer atom", what))
    }
}

fn bare_str(item: &SItem, what: &str) -> Result<String, ParseError> {
    if let SItem::Atom(Token::Str(s)) = item {
        Ok(s.clone())
    } else {
        err(format!("{} must be a string atom", what))
    }
}

fn build_form(tag: &str, args: &[SItem]) -> Result<Node, ParseError> {
    match tag {
        "lam" => {
            check_arity(tag, args, 1)?;
            Ok(Node::Lam {
                body: Box::new(build(&args[0])?),
            })
        }
        "app" => {
            check_arity(tag, args, 2)?;
            Ok(Node::App {
                fn_: Box::new(build(&args[0])?),
                arg: Box::new(build(&args[1])?),
            })
        }
        "let" => {
            check_arity(tag, args, 2)?;
            Ok(Node::Let {
                rhs: Box::new(build(&args[0])?),
                body: Box::new(build(&args[1])?),
            })
        }
        "rec" => {
            if args.len() < 2 {
                return err("rec requires at least 1 binding + body (ADR 0011)");
            }
            let mut bindings = Vec::with_capacity(args.len() - 1);
            for a in &args[..args.len() - 1] {
                bindings.push(build(a)?);
            }
            let body = build(&args[args.len() - 1])?;
            Ok(Node::Rec {
                bindings,
                body: Box::new(body),
            })
        }
        "module" => {
            if args.is_empty() {
                return err("module requires at least 1 binding (ADR 0011)");
            }
            let mut bindings = Vec::with_capacity(args.len());
            for a in args {
                bindings.push(build(a)?);
            }
            Ok(Node::Module { bindings })
        }
        "if" => {
            check_arity(tag, args, 3)?;
            Ok(Node::If {
                cond: Box::new(build(&args[0])?),
                then: Box::new(build(&args[1])?),
                else_: Box::new(build(&args[2])?),
            })
        }
        "match" => {
            if args.len() < 2 {
                return err("match requires scrutinee + at least 1 arm (ADR 0011)");
            }
            let scrutinee = build(&args[0])?;
            let mut arms = Vec::with_capacity(args.len() - 1);
            for a in &args[1..] {
                let arm = build(a)?;
                if !matches!(arm, Node::Arm { .. }) {
                    return err("match child must be an arm");
                }
                arms.push(arm);
            }
            Ok(Node::Match {
                scrutinee: Box::new(scrutinee),
                arms,
            })
        }
        "arm" => {
            check_arity(tag, args, 2)?;
            Ok(Node::Arm {
                pattern: Box::new(build(&args[0])?),
                body: Box::new(build(&args[1])?),
            })
        }
        "record" => {
            if !args.len().is_multiple_of(2) {
                return err("record requires an even number of children (sym/val pairs)");
            }
            let mut fields = Vec::with_capacity(args.len() / 2);
            let mut i = 0;
            while i < args.len() {
                let k = bare_sym(&args[i], "record field")?;
                let v = build(&args[i + 1])?;
                fields.push((k, v));
                i += 2;
            }
            Ok(Node::Record { fields })
        }
        "proj" => {
            check_arity(tag, args, 2)?;
            Ok(Node::Proj {
                record: Box::new(build(&args[0])?),
                field: bare_sym(&args[1], "proj field")?,
            })
        }
        "ctor" => {
            if args.is_empty() {
                return err("ctor requires a name symbol");
            }
            let name = bare_sym(&args[0], "ctor name")?;
            let mut sub = Vec::with_capacity(args.len() - 1);
            for a in &args[1..] {
                sub.push(build(a)?);
            }
            Ok(Node::Ctor { name, args: sub })
        }
        "ann" => {
            check_arity(tag, args, 2)?;
            Ok(Node::Ann {
                expr: Box::new(build(&args[0])?),
                type_: Box::new(build(&args[1])?),
            })
        }
        "var" => {
            check_arity(tag, args, 1)?;
            let text = bare_int(&args[0], "var index")?;
            if text.starts_with('-') {
                return err("var index must be >= 0");
            }
            let idx: u64 = text.parse().map_err(|e: std::num::ParseIntError| {
                ParseError::Structural(format!("invalid var index: {}", e))
            })?;
            Ok(Node::Var { index: idx })
        }
        "int" => {
            check_arity(tag, args, 1)?;
            let text = bare_int(&args[0], "int literal")?;
            Ok(Node::int_from_decimal(&text))
        }
        "str" => {
            check_arity(tag, args, 1)?;
            Ok(Node::Str {
                value: bare_str(&args[0], "str literal")?,
            })
        }
        "sym" => {
            check_arity(tag, args, 1)?;
            Ok(Node::Sym {
                name: bare_sym(&args[0], "sym atom")?,
            })
        }
        "hole" => {
            check_arity(tag, args, 2)?;
            let diag = bare_sym(&args[0], "hole diag-id")?;
            let payload = build(&args[1])?;
            if !matches!(payload, Node::Str { .. }) {
                return err("hole payload must be a (str ...) node");
            }
            Ok(Node::Hole {
                diag_id: diag,
                payload: Box::new(payload),
            })
        }
        "pat-ctor" => {
            if args.is_empty() {
                return err("pat-ctor requires a name symbol");
            }
            let name = bare_sym(&args[0], "pat-ctor name")?;
            let mut sub = Vec::with_capacity(args.len() - 1);
            for a in &args[1..] {
                sub.push(build(a)?);
            }
            Ok(Node::PatCtor {
                name,
                sub_patterns: sub,
            })
        }
        "pat-int" => {
            check_arity(tag, args, 1)?;
            let text = bare_int(&args[0], "pat-int literal")?;
            Ok(Node::PatInt {
                value: if text == "-0" {
                    "0".to_string()
                } else {
                    text
                },
            })
        }
        "fn-ty" => {
            check_arity(tag, args, 3)?;
            Ok(Node::FnTy {
                arg: Box::new(build(&args[0])?),
                ret: Box::new(build(&args[1])?),
                eff: Box::new(build(&args[2])?),
            })
        }
        "ty-var" => {
            check_arity(tag, args, 1)?;
            let text = bare_int(&args[0], "ty-var index")?;
            if text.starts_with('-') {
                return err("ty-var index must be >= 0");
            }
            let idx: u64 = text.parse().map_err(|e: std::num::ParseIntError| {
                ParseError::Structural(format!("invalid ty-var index: {}", e))
            })?;
            Ok(Node::TyVar { index: idx })
        }
        "forall" => {
            check_arity(tag, args, 3)?;
            let ty_text = bare_int(&args[0], "forall ty-count")?;
            let eff_text = bare_int(&args[1], "forall eff-count")?;
            let ty_count: u32 = ty_text.parse().map_err(|e: std::num::ParseIntError| {
                ParseError::Structural(format!("invalid forall ty-count: {}", e))
            })?;
            let eff_count: u32 = eff_text.parse().map_err(|e: std::num::ParseIntError| {
                ParseError::Structural(format!("invalid forall eff-count: {}", e))
            })?;
            Ok(Node::Forall {
                ty_count,
                eff_count,
                body: Box::new(build(&args[2])?),
            })
        }
        "eff-set" => {
            // Children are bare symbol atoms (effect atom names), sorted required.
            let mut atoms = Vec::with_capacity(args.len());
            for a in args {
                atoms.push(bare_sym(a, "eff-set atom")?);
            }
            Ok(Node::EffSet { atoms })
        }
        "eff-var" => {
            check_arity(tag, args, 1)?;
            let text = bare_int(&args[0], "eff-var index")?;
            if text.starts_with('-') {
                return err("eff-var index must be >= 0");
            }
            let idx: u64 = text.parse().map_err(|e: std::num::ParseIntError| {
                ParseError::Structural(format!("invalid eff-var index: {}", e))
            })?;
            Ok(Node::EffVar { index: idx })
        }
        _ => err(format!("unknown tag {:?}", tag)),
    }
}
