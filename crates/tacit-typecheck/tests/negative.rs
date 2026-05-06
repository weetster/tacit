//! Negative type-check tests: one case per error kind from ADR 0041.
//!
//! Each test constructs a minimal AST that triggers exactly one error kind
//! and asserts the diagnostic is produced with the correct `kind` field.

use tacit_canonical::ast::Node;
use tacit_typecheck::infer_module;
use tacit_typecheck::sidecar::check_against_tacd;
use tacit_typecheck::ty::Subst;

// ── helpers ──────────────────────────────────────────────────────────────────

fn int(v: i64) -> Node {
    Node::Int {
        value: v.to_string(),
    }
}

fn str_node(s: &str) -> Node {
    Node::Str {
        value: s.to_string(),
    }
}

fn sym(name: &str) -> Node {
    Node::Sym {
        name: name.to_string(),
    }
}

fn app(f: Node, a: Node) -> Node {
    Node::App {
        fn_: Box::new(f),
        arg: Box::new(a),
    }
}

fn ann(expr: Node, type_: Node) -> Node {
    Node::Ann {
        expr: Box::new(expr),
        type_: Box::new(type_),
    }
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

fn expect_authoring_error(src: &str, kind: &str) {
    let (ast, _) = tacit_views::authoring::parse_authoring(src.as_bytes())
        .unwrap_or_else(|e| panic!("parse failed for {src:?}: {e}"));
    let diags = infer_module(&ast).expect_err("expected typecheck error");
    expect_error(&diags, kind);
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
    let ast = app(app(sym("add"), cond), int(5)); // add Bool Int → mismatch
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
    let ast = Node::Module {
        bindings: vec![int(42)],
    };
    let mut subst = Subst::default();
    let mut diags = Vec::new();
    tacit_typecheck::infer::infer(&[], &ast, &mut subst, &[], &mut diags);
    assert!(
        diags.iter().any(|d| d.kind == "module-missing-annotation"),
        "expected module-missing-annotation warning, got: {:?}",
        diags
            .iter()
            .map(|d| (&d.kind, &d.severity))
            .collect::<Vec<_>>()
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

/// Parser recovery (ADR 0040): malformed authoring text produces a Hole node
/// rather than a hard parse error; the Hole flows through typecheck as an error.
#[test]
fn neg_parser_recovery_hole_flows_through_typecheck() {
    // `lambda x. => x` — FatArrow in expression position triggers recover_expr.
    // The parser should succeed (no ParseError), producing a Hole in the body.
    let src = b"lambda x. => x";
    let (ast, _sidecar) =
        tacit_views::authoring::parse_authoring(src).expect("parser should recover, not hard-fail");
    // The AST should contain a hole somewhere (the body).
    let canonical = String::from_utf8(tacit_canonical::emit(&ast)).unwrap();
    assert!(
        canonical.contains("hole"),
        "expected hole in AST, got: {}",
        canonical
    );
    // Typechecking the holed program should yield a Hole-related diagnostic.
    let result = infer_module(&ast);
    let diags = result.unwrap_err();
    assert!(
        !diags.is_empty(),
        "expected at least one diagnostic from hole"
    );
}

// ── sidecar type mismatch ──────────────────────────────────────────────────────

/// check_against_tacd reports a type-mismatch when the inferred type
/// does not match the sidecar type_hint.
#[test]
fn neg_sidecar_type_mismatch() {
    use tacit_views::sidecar::{Sidecar, SidecarNode};

    // Expression is `"hello"` (Str), but sidecar expects Int.
    let ast = str_node("hello");
    let display = SidecarNode {
        type_hint: Some("Int".to_string()),
        ..Default::default()
    };
    let sidecar = Sidecar::new(b"(str \"hello\")", display);
    let result = check_against_tacd(&ast, &sidecar);
    let diags = result.unwrap_err();
    expect_error(&diags, "type-mismatch");
}

// ── effect-violation ──────────────────────────────────────────────────────────

/// Annotating a lambda as pure while its body calls @write produces effect-violation.
#[test]
fn neg_effect_violation_pure_annotation_on_io_body() {
    use tacit_canonical::ast::Node;

    // ann (lam n. let _ = @write 1 "x" 1 in n)
    //     (fn-ty Int Int (eff-set))   ← declared pure
    let fn_ty_node = Node::FnTy {
        arg: Box::new(Node::Sym {
            name: "Int".to_string(),
        }),
        ret: Box::new(Node::Sym {
            name: "Int".to_string(),
        }),
        eff: Box::new(Node::EffSet { atoms: vec![] }),
    };
    let write_call = app(app(app(sym("write"), int(1)), str_node("x")), int(1));
    // lam n. let _ = write(...) in n   (var 1 = n after let shifts context)
    let body = Node::Let {
        rhs: Box::new(write_call),
        body: Box::new(Node::Var { index: 1 }),
    };
    let annotated = Node::Ann {
        expr: Box::new(Node::Lam {
            body: Box::new(body),
        }),
        type_: Box::new(fn_ty_node),
    };

    let mut subst = tacit_typecheck::ty::Subst::default();
    let mut diags = Vec::new();
    tacit_typecheck::infer::infer(&[], &annotated, &mut subst, &[], &mut diags);
    expect_error(&diags, "effect-violation");
}

// ── sidecar effect mismatch ───────────────────────────────────────────────────

/// check_against_tacd reports an effect-violation when the inferred
/// eval-effect does not match the sidecar's effect_hint.
#[test]
fn neg_sidecar_effect_mismatch() {
    use tacit_views::sidecar::{Sidecar, SidecarNode};

    // Expression is pure (int 0), but sidecar expects IO.
    let ast = int(0);
    let display = SidecarNode {
        type_hint: Some("Int".to_string()),
        effect_hint: Some(vec!["IO".to_string()]),
        ..Default::default()
    };
    let sidecar = Sidecar::new(b"(int 0)", display);
    let result = check_against_tacd(&ast, &sidecar);
    let diags = result.unwrap_err();
    expect_error(&diags, "effect-violation");
}

// ── unbound-effect-variable ──────────────────────────────────────────────────

/// A fn-ty with eff-var(0) but no enclosing forall produces an error.
#[test]
fn neg_unbound_effect_variable() {
    use tacit_canonical::ast::Node;

    let fn_ty_node = Node::FnTy {
        arg: Box::new(Node::Sym {
            name: "Int".to_string(),
        }),
        ret: Box::new(Node::Sym {
            name: "Int".to_string(),
        }),
        eff: Box::new(Node::EffVar { index: 0 }),
    };
    let annotated = ann(int(0), fn_ty_node);
    let mut subst = tacit_typecheck::ty::Subst::default();
    let mut diags = Vec::new();
    tacit_typecheck::infer::infer(&[], &annotated, &mut subst, &[], &mut diags);
    assert!(
        diags.iter().any(|d| d.kind == "unbound-effect-variable"),
        "expected unbound-effect-variable, got: {:?}",
        diags.iter().map(|d| &d.kind).collect::<Vec<_>>()
    );
}

// ── I64Vec / Buf mixups ──────────────────────────────────────────────────────

#[test]
fn neg_i64_get_rejects_byte_buffer() {
    expect_authoring_error("let buf = @buf-alloc 8 in @i64-get buf 0", "type-mismatch");
}

#[test]
fn neg_buf_get_rejects_i64_vector() {
    expect_authoring_error("let xs = @i64-alloc 1 in @buf-get xs 0", "type-mismatch");
}

#[test]
fn neg_read_rejects_i64_vector() {
    expect_authoring_error("let xs = @i64-alloc 1 in @read 0 xs 1", "type-mismatch");
}

#[test]
fn neg_write_rejects_i64_vector() {
    expect_authoring_error("let xs = @i64-alloc 1 in @write 1 xs 1", "type-mismatch");
}

#[test]
fn neg_parse_i64_rejects_i64_vector() {
    expect_authoring_error(
        "let xs = @i64-alloc 1 in @parse-i64 xs 0 1",
        "type-mismatch",
    );
}

#[test]
fn neg_fmt_i64_rejects_i64_vector() {
    expect_authoring_error("let xs = @i64-alloc 1 in @fmt-i64 xs 0 42", "type-mismatch");
}

#[test]
fn neg_line_index_rejects_byte_buffer_table() {
    expect_authoring_error(
        "let text = @buf-alloc 1 in let table = @buf-alloc 2 in @line-index text 1 table",
        "type-mismatch",
    );
}

#[test]
fn neg_token_index_rejects_byte_buffer_table() {
    expect_authoring_error(
        "let text = @buf-alloc 1 in let table = @buf-alloc 2 in @token-index text 0 1 32 table",
        "type-mismatch",
    );
}

#[test]
fn neg_token_index_any_rejects_byte_buffer_table() {
    expect_authoring_error(
        "let text = @buf-alloc 1 in let table = @buf-alloc 2 in @token-index-any text 0 1 \" \" 1 table",
        "type-mismatch",
    );
}

#[test]
fn neg_line_index_rejects_i64_vector_text() {
    expect_authoring_error(
        "let text = @i64-alloc 1 in let table = @i64-alloc 2 in @line-index text 1 table",
        "type-mismatch",
    );
}

#[test]
fn neg_token_index_any_rejects_i64_vector_text() {
    expect_authoring_error(
        "let text = @i64-alloc 1 in let table = @i64-alloc 2 in @token-index-any text 0 1 \" \" 1 table",
        "type-mismatch",
    );
}

#[test]
fn neg_token_index_any_rejects_i64_vector_delims() {
    expect_authoring_error(
        "let text = @buf-alloc 1 in let delims = @i64-alloc 1 in let table = @i64-alloc 2 in @token-index-any text 0 1 delims 1 table",
        "type-mismatch",
    );
}

#[test]
fn neg_range_start_rejects_byte_buffer() {
    expect_authoring_error(
        "let table = @buf-alloc 2 in @range-start table 0",
        "type-mismatch",
    );
}

#[test]
fn neg_range_len_rejects_byte_buffer() {
    expect_authoring_error(
        "let table = @buf-alloc 2 in @range-len table 0",
        "type-mismatch",
    );
}

#[test]
fn neg_sort_i64_rejects_byte_buffer() {
    expect_authoring_error("let xs = @buf-alloc 2 in @sort-i64 xs 2", "type-mismatch");
}

#[test]
fn neg_sort_ranges_by_bytes_rejects_i64_text() {
    expect_authoring_error(
        "let text = @i64-alloc 2 in let rows = @i64-alloc 4 in @sort-ranges-by-bytes text rows 2",
        "type-mismatch",
    );
}

#[test]
fn neg_stable_sort_pairs_i64_rejects_byte_buffer_values() {
    expect_authoring_error(
        "let keys = @i64-alloc 2 in let values = @buf-alloc 16 in @stable-sort-pairs-i64 keys values 2",
        "type-mismatch",
    );
}

#[test]
fn neg_lower_bound_i64_rejects_byte_buffer() {
    expect_authoring_error(
        "let xs = @buf-alloc 8 in @lower-bound-i64 xs 1 4",
        "type-mismatch",
    );
}

#[test]
fn neg_count_equal_ranges_rejects_i64_text() {
    expect_authoring_error(
        "let text = @i64-alloc 2 in let rows = @i64-alloc 4 in let out = @i64-alloc 6 in @count-equal-ranges text rows 2 out",
        "type-mismatch",
    );
}

#[test]
fn neg_count_equal_ranges_rejects_byte_buffer_out() {
    expect_authoring_error(
        "let text = @buf-alloc 4 in let rows = @i64-alloc 4 in let out = @buf-alloc 24 in @count-equal-ranges text rows 2 out",
        "type-mismatch",
    );
}

#[test]
fn neg_dedup_adjacent_ranges_rejects_byte_buffer_table() {
    expect_authoring_error(
        "let text = @buf-alloc 4 in let rows = @buf-alloc 8 in let out = @i64-alloc 4 in @dedup-adjacent-ranges text rows 2 out",
        "type-mismatch",
    );
}

// ── Bundle E (ADR 0067) ──────────────────────────────────────────────────────

#[test]
fn neg_stdin_slurp_rejects_i64_vector() {
    expect_authoring_error(
        "let xs = @i64-alloc 1 in @stdin-slurp xs 8",
        "type-mismatch",
    );
}

#[test]
fn neg_write_range_rejects_i64_vector() {
    expect_authoring_error(
        "let xs = @i64-alloc 1 in @write-range 1 xs 0 1",
        "type-mismatch",
    );
}

#[test]
fn neg_buf_rev_rejects_i64_vector() {
    expect_authoring_error("let xs = @i64-alloc 1 in @buf-rev xs 0 1", "type-mismatch");
}

// ── Bundle F (ADR 0068) ──────────────────────────────────────────────────────

#[test]
fn neg_ascii_tolower_rejects_buf_argument() {
    expect_authoring_error(
        "let buf = @buf-alloc 1 in @ascii-tolower buf",
        "type-mismatch",
    );
}

#[test]
fn neg_ascii_toupper_rejects_buf_argument() {
    expect_authoring_error(
        "let buf = @buf-alloc 1 in @ascii-toupper buf",
        "type-mismatch",
    );
}

#[test]
fn neg_ascii_is_alpha_rejects_i64_vector_argument() {
    expect_authoring_error(
        "let xs = @i64-alloc 1 in @ascii-is-alpha xs",
        "type-mismatch",
    );
}

#[test]
fn neg_ascii_is_digit_rejects_i64_vector_argument() {
    expect_authoring_error(
        "let xs = @i64-alloc 1 in @ascii-is-digit xs",
        "type-mismatch",
    );
}

#[test]
fn neg_ascii_is_space_rejects_buf_argument() {
    expect_authoring_error(
        "let buf = @buf-alloc 1 in @ascii-is-space buf",
        "type-mismatch",
    );
}

// ── Bundle G (ADR 0069) ──────────────────────────────────────────────────────

#[test]
fn neg_utf8_decode_rejects_i64_vector_buffer() {
    expect_authoring_error(
        "let xs = @i64-alloc 4 in @utf8-decode xs 0",
        "type-mismatch",
    );
}

#[test]
fn neg_utf8_encode_rejects_i64_vector_buffer() {
    expect_authoring_error(
        "let xs = @i64-alloc 4 in @utf8-encode xs 0 65",
        "type-mismatch",
    );
}

#[test]
fn neg_utf8_len_rejects_buf_argument() {
    expect_authoring_error("let buf = @buf-alloc 1 in @utf8-len buf", "type-mismatch");
}
