//! Second canonicalizer for the Tacit-Lite canonical text format.
//!
//! Reference: plans/canonical-text-format.md and decisions/0005-0012.

pub mod ast;
pub mod emit;
pub mod hashing;
pub mod lex;
pub mod parse;

pub use emit::emit;
pub use hashing::{hash_bytes, hash_node};
pub use lex::LexError;
pub use parse::{parse, ParseError};
