//! BLAKE3 content hashing over canonical text (ADR 0009).
//!
//! ```text
//! hash(node) = BLAKE3(canonical_text(node))
//! ```
//!
//! Children are fully inlined; the Merkle property falls out because
//! identical subtrees produce identical canonical bytes.

use crate::ast::Node;
use crate::emit::emit;

pub fn hash_bytes(data: &[u8]) -> [u8; 32] {
    *blake3::hash(data).as_bytes()
}

pub fn hash_node(node: &Node) -> [u8; 32] {
    hash_bytes(&emit(node))
}
