//! Verify each `examples/smoke/*.tac` file parses through the canonical
//! parser + produces an AST whose pre-codegen analysis (closed-lambda
//! check, hole check) succeeds. This runs without LLVM and confirms
//! the smoke programs are syntactically and structurally well-formed
//! before the LLVM-version pin is chosen.

use std::path::PathBuf;

use tacit_canonical::parse as parse_canonical;
use tacit_codegen::analysis::{check_closed, check_no_holes};

fn smoke_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("smoke")
}

fn parse_and_check(name: &str) {
    let path = smoke_dir().join(format!("{}.tac", name));
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {}", path.display(), e));
    let node =
        parse_canonical(&bytes).unwrap_or_else(|e| panic!("parse {}: {}", path.display(), e));
    check_no_holes(&node).unwrap_or_else(|e| panic!("hole check {}: {}", name, e));
    // Top-level depth = 0; any lambda inside extends depth itself.
    check_closed(&node, 0).unwrap_or_else(|e| panic!("closed check {}: {}", name, e));
}

#[test]
fn return_zero_parses() {
    parse_and_check("return-zero");
}

#[test]
fn return_computed_parses() {
    parse_and_check("return-computed");
}

#[test]
fn hello_parses() {
    parse_and_check("hello");
}

#[test]
fn if_branch_parses() {
    parse_and_check("if-branch");
}

#[test]
fn factorial_parses() {
    parse_and_check("factorial");
}

#[test]
fn even_odd_parses() {
    parse_and_check("even-odd");
}

#[test]
fn exit_nonzero_parses() {
    parse_and_check("exit-nonzero");
}
