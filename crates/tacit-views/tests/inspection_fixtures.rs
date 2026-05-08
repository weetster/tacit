//! Regression fixtures for the inspection-view renderer.
//!
//! § 6 worked examples from plans/inspection-view.md are the Stage 3 exit gate.
//! L0 renderings are locked; changes require a new ADR.
//! L1/L2 renderings are regression fixtures established by this implementation.

use tacit_canonical::ast::Node;
use tacit_canonical::parse;
use tacit_views::inspection::{emit_inspection, InspectFlags};
use tacit_views::SidecarNode;

fn l0() -> InspectFlags {
    InspectFlags {
        debruijn: false,
        hashes: false,
        ..Default::default()
    }
}
fn l1() -> InspectFlags {
    InspectFlags {
        debruijn: true,
        hashes: false,
        ..Default::default()
    }
}
fn types_effects() -> InspectFlags {
    InspectFlags {
        types: true,
        effects: true,
        ..Default::default()
    }
}
fn sidecar_from_json(json: &str) -> SidecarNode {
    serde_json::from_str(json).expect("valid sidecar JSON")
}

// ---------------------------------------------------------------------------
// § 6.1 — Identity-of-5
// Canonical: (let (lam (var 0)) (app (var 0) (int 5)))
// ---------------------------------------------------------------------------

fn fixture_6_1_node() -> Node {
    parse(b"(let (lam (var 0)) (app (var 0) (int 5)))").unwrap()
}

fn fixture_6_1_sidecar() -> SidecarNode {
    sidecar_from_json(r#"{"binder":"id","children":[{"binder":"x"},{}]}"#)
}

#[test]
fn section_6_1_l0() {
    let node = fixture_6_1_node();
    let sc = fixture_6_1_sidecar();
    let out = emit_inspection(&node, Some(&sc), &l0());
    assert_eq!(
        out, "let id = lambda x. x in\n  id 5",
        "§6.1 L0 must match spec exactly"
    );
}

#[test]
fn section_6_1_l1() {
    let node = fixture_6_1_node();
    let sc = fixture_6_1_sidecar();
    let out = emit_inspection(&node, Some(&sc), &l1());
    // "lambda x. x" → x is var 0; "id 5" → id is var 0.
    // Annotations append to the assembled line, so `in` stays outside the
    // comment (matching inspection-view.md § 6.1 L0+L1).
    assert_eq!(
        out, "let id = lambda x. x in  # x \u{2261} var 0\n  id 5  # id \u{2261} var 0",
        "§6.1 L1 regression"
    );
}

#[test]
fn section_6_1_l0_no_sidecar() {
    let node = fixture_6_1_node();
    let out = emit_inspection(&node, None, &l0());
    // Without sidecar: synthetic names v0 (let), v0 for lam param (same depth counter)
    // let binder = v0 (lam_let_depth=0), lam binder = v1 (lam_let_depth=1 after let increments)
    // Actually: let increments depth to 1, then lam sees depth=1 → v1
    // var 0 in lam body → lam param = v1
    // var 0 in body (app position) → let binder = v0
    assert!(out.starts_with("let v0 = lambda v1."), "synthetic names");
}

// ---------------------------------------------------------------------------
// § 6.2 — Mutual recursion
// Canonical (from inspection-view.md):
//   (rec (lam (if (var 0) (app (var 2) (ctor sub (var 0) (int 1))) (int 1)))
//        (lam (if (var 0) (app (var 1) (ctor sub (var 0) (int 1))) (int 0)))
//        (app (var 0) (int 10)))
// ---------------------------------------------------------------------------

fn fixture_6_2_node() -> Node {
    parse(b"(rec (lam (if (var 0) (app (var 2) (ctor sub (var 0) (int 1))) (int 1))) (lam (if (var 0) (app (var 1) (ctor sub (var 0) (int 1))) (int 0))) (app (var 0) (int 10)))").unwrap()
}

fn fixture_6_2_sidecar() -> SidecarNode {
    sidecar_from_json(
        r#"{
        "binders": ["even", "odd"],
        "children": [
            {"binder": "n"},
            {"binder": "n"},
            {}
        ]
    }"#,
    )
}

#[test]
fn section_6_2_l0() {
    let node = fixture_6_2_node();
    let sc = fixture_6_2_sidecar();
    let out = emit_inspection(&node, Some(&sc), &l0());
    let expected = "\
rec
  | even =
      lambda n.
        if n
        then odd (sub n 1)
        else 1
  | odd =
      lambda n.
        if n
        then even (sub n 1)
        else 0
in
  even 10";
    assert_eq!(out, expected, "§6.2 L0 must match spec exactly");
}

// ---------------------------------------------------------------------------
// § 6.3 — Hole in a program
// Canonical: (let (lam (hole expected-expr (str "missing body after lambda")))
//                 (app (var 0) (int 5)))
// ---------------------------------------------------------------------------

fn fixture_6_3_node() -> Node {
    parse(b"(let (lam (hole expected-expr (str \"missing body after lambda\"))) (app (var 0) (int 5)))").unwrap()
}

fn fixture_6_3_sidecar() -> SidecarNode {
    sidecar_from_json(r#"{"binder":"id","children":[{"binder":"x"},{}]}"#)
}

#[test]
fn section_6_3_l0() {
    let node = fixture_6_3_node();
    let sc = fixture_6_3_sidecar();
    let out = emit_inspection(&node, Some(&sc), &l0());
    let expected = "let id = lambda x. \u{27E8}hole:expected-expr \"missing body after lambda\"\u{27E9} in\n  id 5";
    assert_eq!(out, expected, "§6.3 L0 must match spec exactly");
}

// ---------------------------------------------------------------------------
// Additional unit tests for individual kinds.
// ---------------------------------------------------------------------------

#[test]
fn int_literal() {
    let node = parse(b"(int 42)").unwrap();
    assert_eq!(emit_inspection(&node, None, &l0()), "42");
}

#[test]
fn str_literal() {
    let node = parse(b"(str \"hello\\nworld\")").unwrap();
    assert_eq!(emit_inspection(&node, None, &l0()), "\"hello\\nworld\"");
}

#[test]
fn sym_at_prefix() {
    let node = parse(b"(sym write)").unwrap();
    assert_eq!(emit_inspection(&node, None, &l0()), "@write");
}

#[test]
fn pat_wild() {
    let node = parse(b"pat-wild").unwrap();
    assert_eq!(emit_inspection(&node, None, &l0()), "_");
}

#[test]
fn ctor_nullary() {
    let node = parse(b"(ctor Nil)").unwrap();
    assert_eq!(emit_inspection(&node, None, &l0()), "Nil");
}

#[test]
fn ctor_applied_inline() {
    let node = parse(b"(ctor Cons (int 1) (int 2))").unwrap();
    assert_eq!(emit_inspection(&node, None, &l0()), "Cons 1 2");
}

#[test]
fn record_empty() {
    let node = parse(b"(record)").unwrap();
    assert_eq!(emit_inspection(&node, None, &l0()), "{}");
}

#[test]
fn record_single_field_inline() {
    let node = parse(b"(record fst (int 1))").unwrap();
    assert_eq!(emit_inspection(&node, None, &l0()), "{ fst: 1 }");
}

#[test]
fn if_always_breaks() {
    let node = parse(b"(if (int 1) (int 2) (int 3))").unwrap();
    let out = emit_inspection(&node, None, &l0());
    assert_eq!(out, "if 1\nthen 2\nelse 3");
}

#[test]
fn match_arms() {
    let node = parse(b"(match (var 0) (arm pat-wild (int 0)))").unwrap();
    // var 0 is out of scope → ?var0
    let out = emit_inspection(&node, None, &l0());
    assert!(
        out.starts_with("match ?var0\n| _ => 0"),
        "match rendering: {:?}",
        out
    );
}

#[test]
fn hole_rendering() {
    let node = parse(b"(hole type-error (str \"expected int\"))").unwrap();
    let out = emit_inspection(&node, None, &l0());
    assert_eq!(out, "\u{27E8}hole:type-error \"expected int\"\u{27E9}");
}

#[test]
fn let_chain() {
    // let a = 1 in let b = 2 in b
    let node = parse(b"(let (int 1) (let (int 2) (var 0)))").unwrap();
    let out = emit_inspection(&node, None, &l0());
    // Expect the chain to not grow indentation
    let expected = "let v0 = 1 in\nlet v1 = 2 in\n  v1";
    assert_eq!(out, expected, "let chain: {:?}", out);
}

#[test]
fn ctor_arg_sidecar_indexing() {
    // canonical (ctor Cons …) children are [name-sym, arg₀, arg₁],
    // so an arg's sidecar lives at child(k+1), not child(k).
    let node = parse(b"(ctor Cons (int 1) (int 2))").unwrap();
    let sc: SidecarNode =
        serde_json::from_str(r#"{"children":[{},{"comment":"first"},{"comment":"second"}]}"#)
            .unwrap();
    let out = emit_inspection(&node, Some(&sc), &l0());
    let expected = "Cons\n  # first\n  1\n  # second\n  2";
    assert_eq!(out, expected, "ctor arg sidecar indexing: {:?}", out);
}

#[test]
fn ctor_propagates_var_annot_through_let() {
    // (let (lam (ctor Pair (var 0) (var 0))) (var 0))
    // L1 annotation must follow `in` on the header line, not embed inside it.
    let node = parse(b"(let (lam (ctor Pair (var 0) (var 0))) (var 0))").unwrap();
    let sc = sidecar_from_json(r#"{"binder":"p","children":[{"binder":"x"},{}]}"#);
    let out = emit_inspection(&node, Some(&sc), &l1());
    let expected = "let p = lambda x. Pair x x in  # x \u{2261} var 0, x \u{2261} var 0\n  p  # p \u{2261} var 0";
    assert_eq!(out, expected, "ctor annot propagation: {:?}", out);
}

#[test]
fn phase_4_record_type_annotation_renders_structurally() {
    let node = parse(
        b"(ann (lam (proj (var 0) x)) (fn-ty (record x (sym Int) y (sym Int)) (sym Int) (eff-set)))",
    )
    .unwrap();
    let sc = sidecar_from_json(r#"{"children":[{"binder":"p"},{}]}"#);
    let out = emit_inspection(&node, Some(&sc), &types_effects());
    assert_eq!(out, "(lambda p. p.x : {x: Int, y: Int} -> Int)");
}

#[test]
fn phase_4_closure_capture_overlay_is_flag_gated() {
    let (node, sc) =
        tacit_views::authoring::parse_authoring(b"let n = 1 in lambda x. @add x n").unwrap();
    let out = emit_inspection(&node, Some(&sc), &types_effects());
    assert_eq!(out, "let n = 1 in\n  lambda x [captures n]. @add x n");
}

#[test]
fn phase_4_combinator_application_uses_labeled_inspection_block() {
    let (node, sc) = tacit_views::authoring::parse_authoring(
        b"let xs = @i64-alloc 1 in let ys = @i64-alloc 1 in @map xs 1 (lambda x. @add x 1) ys",
    )
    .unwrap();
    let out = emit_inspection(&node, Some(&sc), &types_effects());
    let expected = "\
let xs = @i64-alloc 1 in
let ys = @i64-alloc 1 in
  @map
    input xs
    count 1
    callback lambda x. @add x 1
    output ys";
    assert_eq!(out, expected);
}
