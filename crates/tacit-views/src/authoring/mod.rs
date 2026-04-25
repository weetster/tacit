//! Authoring view (BPE-compact) parser and emitter.
//!
//! Reference: plans/candidates/authoring-bpe-compact.md.

pub mod emit;
pub mod lex;
pub mod parse;

pub use emit::emit_authoring;
pub use parse::{parse_authoring, ParseError};
