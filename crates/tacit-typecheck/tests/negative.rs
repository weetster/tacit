//! Negative type-check tests: one case per error kind from ADR 0041.
//!
//! Each test constructs a minimal AST that triggers exactly one error kind
//! and asserts the diagnostic is produced with the correct `kind` field.

use tacit_canonical::ast::Node;
use tacit_typecheck::infer_module;
use tacit_typecheck::sidecar::{check_against_sidecar, TypeSidecar};
use tacit_typecheck::ty::Subst;

// ── helpers ──────────────────────────────────────────────────────────────────

fn int(v: i64) -> Node {
    Node::Int { value: v.to_string() }
}

fn str_node(s: &str) -> Node {
    Node::Str { value: s.to_string() }
}

fn sym(name: &str) -> Node {
    Node::Sym { name: name.to_string() }
}

fn app(f: Node, a: Node) -> Node {
    Node::App { fn_: Box::new(f), arg: Box::new(a) }
}

fn ann(expr: Node, type_: Node) -> Node {
    Node::Ann { expr: Box::new(expr), type_: Box::new(type_) }
}

fn has_error(diags: &[tacit_typecheck::Diagnostic], kind: &str) -> bool {
    diags.iter().any(|d| d.kind == kind)
}

fn expect_error(diags: &[tacit_typecheck::Diagnostic], kind: &str) {
    assert!(
        has_error(diags, kind),
        "expected error kind '{}', got: {:?}",
        kind,
        diags.iter().map(|d| &d.kind).collect::<Vec<_>>()
    );
}

// ── type-mismatch ─────────────────────────────────────────────────────────────

/// `ann` check: annotating an Int expression with Str type.
#[test]
fn neg_type_mismatch() {
    // (ann 42 Str) — Int expression annotated as Str
    let ast = ann(int(42), sym("Str"));
    let result = infer_module(&ast);
    let diags = result.unwrap_err();
    expect_error(&diags, "type-mismatch");
}

// ── operator-overload-failure ────────────────────────────────────────────────

/// Applying @add to a Bool and an Int — operand types disagree (ADR 0042).
#[test]
fn neg_operator_overload_failure() {
    // (@add (@eq 1 2) 5) — first operand is Bool, second is Int
    let cond = app(app(sym("eq"), int(1)), int(2)); // Bool
    let ast = app(app(sym("add"), cond), int(5));   // add Bool Int → mismatch
    let result = infer_module(&ast);
    let diags = result.unwrap_err();
    expect_error(&diags, "operator-overload-failure");
}

// ── unbound-type-variable ────────────────────────────────────────────────────

/// A `ty-var` node with no enclosing `forall`.
#[test]
fn neg_unbound_type_variable() {
    // (ann 1 (ty-var 0)) — ty-var 0 with no forall in scope
    let ty_node = Node::TyVar { index: 0 };
    let ast = ann(int(1), ty_node);
    // type_from_node is called by infer for Ann nodes — should emit unbound-type-variable.
    // However infer_module filters by severity=="error"; unbound-type-variable is an error.
    let result = infer_module(&ast);
    // Ann with unknown type doesn't produce a type-mismatch (Unknown is compatible).
    // The unbound-type-variable diagnostic IS present among all diags.
    // Check directly via infer() to get all diagnostics.
    let mut subst = Subst::default();
    let mut diags = Vec::new();
    tacit_typecheck::infer::infer(&[], &ast, &mut subst, &[], &mut diags);
    expect_error(&diags, "unbound-type-variable");
    let _ = result; // may be Ok or Err depending on whether Unknown suppresses mismatch
}

// ── type-arity-mismatch ──────────────────────────────────────────────────────

/// Applying a base type (Int) as a type constructor in an annotation.
#[test]
fn neg_type_arity_mismatch() {
    // (ann 1 (app (sym Int) (sym Bool))) — Int takes 0 args, given 1
    let ty_node = Node::App {
        fn_: Box::new(sym("Int")),
        arg: Box::new(sym("Bool")),
    };
    let ast = ann(int(1), ty_node);
    let mut subst = Subst::default();
    let mut diags = Vec::new();
    tacit_typecheck::infer::infer(&[], &ast, &mut subst, &[], &mut diags);
    expect_error(&diags, "type-arity-mismatch");
}

// ── unresolved-type ───────────────────────────────────────────────────────────

/// An unknown type name in an annotation.
#[test]
fn neg_unresolved_type() {
    // (ann 1 (sym Foo)) — Foo is not a known type
    let ast = ann(int(1), sym("Foo"));
    let mut subst = Subst::default();
    let mut diags = Vec::new();
    tacit_typecheck::infer::infer(&[], &ast, &mut subst, &[], &mut diags);
    expect_error(&diags, "unresolved-type");
}

// ── module-missing-annotation ─────────────────────────────────────────────────

/// A module binding without an explicit type+effect annotation.
/// This produces a warning (not an error), so we inspect all diagnostics.
#[test]
fn neg_module_missing_annotation() {
    // (module [42]) — one unannotated binding
    let ast = Node::Module { bindings: vec![int(42)] };
    let mut subst = Subst::default();
    let mut diags = Vec::new();
    tacit_typecheck::infer::infer(&[], &ast, &mut subst, &[], &mut diags);
    assert!(
        diags.iter().any(|d| d.kind == "module-missing-annotation"),
        "expected module-missing-annotation warning, got: {:?}",
        diags.iter().map(|d| (&d.kind, &d.severity)).collect::<Vec<_>>()
    );
}

// ── hole-diagnostic ───────────────────────────────────────────────────────────

/// A Hole node propagates the diag-id as the error kind.
#[test]
fn neg_hole_diagnostic() {
    // (hole "parse-error" (str "unexpected token")) — hole in expression position
    let ast = Node::Hole {
        diag_id: "parse-error".to_string(),
        payload: Box::new(str_node("unexpected token")),
    };
    let result = infer_module(&ast);
    let diags = result.unwrap_err();
    expect_error(&diags, "parse-error");
}

// ── sidecar type mismatch ──────────────────────────────────────────────────────

/// check_against_sidecar reports a type-mismatch when the inferred type
/// does not match the sidecar expectation.
#[test]
fn neg_sidecar_type_mismatch() {
    use std::collections::BTreeMap;
    use tacit_typecheck::sidecar::TypeEntry;

    // Expression is `"hello"` (Str), but sidecar expects Int.
    let ast = str_node("hello");
    let mut types = BTreeMap::new();
    types.insert(
        "main".to_string(),
        TypeEntry {
            type_str: "Int".to_string(),
            effects: vec![],
        },
    );
    let sidecar = TypeSidecar { types };
    let result = check_against_sidecar(&ast, &sidecar);
    let diags = result.unwrap_err();
    expect_error(&diags, "type-mismatch");
}
