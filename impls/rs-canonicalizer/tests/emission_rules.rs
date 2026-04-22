//! Parse-and-normalize tests called out in plans/test-vectors/README.md.
//!
//! Some spec rules can't be checked purely through round-trip on fixture
//! bytes — they require constructing a non-canonical AST (or non-canonical
//! input) and confirming the canonicalizer normalizes it.

use tacit_canon::ast::Node;
use tacit_canon::{emit, parse};

#[test]
fn i1_signed_zero_normalizes() {
    // Input `(int -0)` is non-canonical but semantically a zero integer.
    // ADR 0010 I1: must emit as `(int 0)`.
    assert_eq!(emit(&parse(b"(int -0)").unwrap()), b"(int 0)");
    // Constructed AST with "0" also emits without sign.
    assert_eq!(emit(&Node::Int { value: "0".into() }), b"(int 0)");
    assert_eq!(emit(&Node::int_from_decimal("-0")), b"(int 0)");
}

#[test]
fn s1_named_escape_preferred_over_hex() {
    assert_eq!(emit(&Node::Str { value: "\"".into() }), b"(str \"\\\"\")");
    assert_eq!(emit(&Node::Str { value: "\\".into() }), b"(str \"\\\\\")");
    assert_eq!(emit(&Node::Str { value: "\t".into() }), b"(str \"\\t\")");
    assert_eq!(emit(&Node::Str { value: "\n".into() }), b"(str \"\\n\")");
    assert_eq!(emit(&Node::Str { value: "\r".into() }), b"(str \"\\r\")");
}

#[test]
fn s2_non_ascii_emits_as_hex() {
    let raw = "(str \"A\u{1f600}\")".as_bytes().to_vec();
    assert_eq!(emit(&parse(&raw).unwrap()), b"(str \"A\\u{1f600}\")");
}

#[test]
fn s3_nul_uses_single_zero() {
    assert_eq!(emit(&Node::Str { value: "\0".into() }), b"(str \"\\u{0}\")");
}

#[test]
fn s3_shortest_lowercase_hex() {
    assert_eq!(emit(&Node::Str { value: "\u{7f}".into() }), b"(str \"\\u{7f}\")");
    assert_eq!(emit(&Node::Str { value: "\u{ffff}".into() }), b"(str \"\\u{ffff}\")");
    assert_eq!(emit(&Node::Str { value: "\u{1f600}".into() }), b"(str \"\\u{1f600}\")");
    assert_eq!(emit(&Node::Str { value: "\u{10ffff}".into() }), b"(str \"\\u{10ffff}\")");
}

#[test]
fn i2_bignum_roundtrip() {
    let fifty_nines: String = "9".repeat(50);
    let src = format!("(int {})", fifty_nines).into_bytes();
    assert_eq!(emit(&parse(&src).unwrap()), src);
    assert_eq!(parse(&src).unwrap(), Node::Int { value: fifty_nines });
}
