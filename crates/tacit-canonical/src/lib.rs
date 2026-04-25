//! Canonical text format for Tacit-Lite: AST, parser, emitter, and BLAKE3 hasher.
//!
//! Reference: plans/canonical-text-format.md and decisions/0005-0013.

pub mod ast;
pub mod emit;
pub mod hashing;
pub mod lex;
pub mod parse;

pub use emit::emit;
pub use hashing::{hash_bytes, hash_node};
pub use lex::LexError;
pub use parse::{parse, ParseError};
