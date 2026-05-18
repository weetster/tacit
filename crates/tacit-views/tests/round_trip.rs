//! Round-trip property tests: canonical → authoring → canonical.
//!
//! For each .canonical test vector we:
//!   1. Parse the canonical bytes into a Node.
//!   2. Emit the authoring view (synthetic names, no sidecar).
//!   3. Parse the authoring text back.
//!   4. Re-emit canonical bytes and assert byte-identity.
//!
//! Skipped vectors:
//!   16 — field symbols with dashes (`a-`, `a-b`) and lone `_` as field name;
//!         not representable as authoring-view identifiers.
//!   08a/08b, 20a–20f — hole nodes are lossy through the authoring view
//!         (all holes render as `_` and parse back as expected-expr holes).
//!   12, 19a, 19b, 26 — open terms (free variable at top level); the authoring
//!         view can only represent closed, well-scoped programs.
//!   29–31 — Phase 2 type nodes; authoring syntax is Stage 5 work.

use std::fs;
use std::path::PathBuf;

use tacit_canonical::{emit, parse};
use tacit_views::authoring::{emit_authoring, parse_authoring};

fn vectors_dir() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("plans")
        .join("test-vectors")
}

const SKIP: &[&str] = &[
    // Field names with dashes / lone `_` — not valid authoring idents.
    "16-symbol-regex-edges.canonical",
    // Holes are lossy through the authoring view (all render as `_`).
    "08a-hole-standalone.canonical",
    "08b-hole-embedded.canonical",
    "20a-unexpected-token.canonical",
    "20b-unclosed-paren.canonical",
    "20c-expected-expr.canonical",
    "20d-expected-pattern.canonical",
    "20e-unbound-name.canonical",
    "20f-arity-mismatch.canonical",
    // Open terms — free var at top level, no binder in scope.
    "12-proj-nested.canonical",
    "19a-match-wild-first.canonical",
    "19b-match-zero-first.canonical",
    "26-ctor-mixed-args.canonical",
    // Phase 2 type nodes (fn-ty, forall, eff-set, ty-var, eff-var): authoring syntax
    // is Stage 5 work; no round-trip until then.
    "29-ann-generic-id.canonical",
    "30-ann-io-fn.canonical",
    "31-ann-eff-poly.canonical",
];

#[test]
fn authoring_round_trip_all_canonical_vectors() {
    let dir = vectors_dir();
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {:?}: {}", dir, e))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("canonical"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no *.canonical fixtures found");

    let mut failures: Vec<String> = Vec::new();

    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy();
        if SKIP.contains(&name.as_ref()) {
            continue;
        }

        let canonical_bytes = fs::read(path).unwrap();
        let node = match parse(&canonical_bytes) {
            Ok(n) => n,
            Err(e) => {
                failures.push(format!("{}: canonical parse failed: {}", name, e));
                continue;
            }
        };
        let canonical_original = emit(&node);
        let authoring_text = emit_authoring(&node, None);

        match parse_authoring(authoring_text.as_bytes()) {
            Ok((node2, _sidecar)) => {
                let canonical_roundtripped = emit(&node2);
                if canonical_original != canonical_roundtripped {
                    failures.push(format!(
                        "{}: round-trip mismatch\n  authoring: {}\n  expected:  {}\n  got:       {}",
                        name,
                        authoring_text,
                        String::from_utf8_lossy(&canonical_original),
                        String::from_utf8_lossy(&canonical_roundtripped),
                    ));
                }
            }
            Err(e) => {
                failures.push(format!(
                    "{}: authoring parse error: {}\n  authoring text: {}",
                    name, e, authoring_text,
                ));
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "{} round-trip failure(s):\n{}",
            failures.len(),
            failures.join("\n---\n")
        );
    }
}

// -------------------------------------------------------------------------
// Canonical → authoring emission snapshots
// -------------------------------------------------------------------------

fn check_emit(canonical: &str, expected_authoring: &str) {
    let node =
        parse(canonical.as_bytes()).unwrap_or_else(|e| panic!("parse {:?}: {}", canonical, e));
    let got = emit_authoring(&node, None);
    assert_eq!(
        got, expected_authoring,
        "authoring emit mismatch\n  canonical: {}\n  expected:  {}\n  got:       {}",
        canonical, expected_authoring, got
    );
}

#[test]
fn emit_identity_lambda() {
    check_emit("(lam (var 0))", "lambda v0. v0");
}

#[test]
fn emit_let_cascade() {
    check_emit(
        "(let (int 1) (let (int 2) (var 1)))",
        "let v0 = 1 in let v1 = 2 in v0",
    );
}

#[test]
fn emit_mutual_rec() {
    check_emit(
        "(rec (lam (app (var 2) (var 0))) (lam (app (var 1) (var 0))) (app (var 0) (int 10)))",
        "rec {B0 = lambda v0. B1 v0; B1 = lambda v0. B0 v0} in B0 10",
    );
}

#[test]
fn emit_record_sorted() {
    check_emit(
        "(record A (int 4) aa (int 2) ab (int 3) b (int 1))",
        "{A:4, aa:2, ab:3, b:1}",
    );
}

#[test]
fn emit_projection_inside_lam() {
    // Wrap in lambda so var 0 is in scope.
    check_emit("(lam (proj (proj (var 0) x) y))", "lambda v0. v0.x.y");
}

#[test]
fn emit_nullary_ctor() {
    check_emit("(ctor Nil)", "Nil");
}

#[test]
fn emit_ctor_with_args_inside_lam() {
    check_emit(
        "(lam (ctor Triple (var 0) (int 42) (ctor Nil)))",
        "lambda v0. Triple v0 42 Nil",
    );
}

#[test]
fn emit_pattern_match() {
    check_emit(
        "(lam (match (var 0) (arm (pat-ctor Pair pat-var pat-var) (app (var 2) (var 1)))))",
        "lambda v0. match v0 with | Pair p0 p1 => v0 p0",
    );
}

#[test]
fn emit_ann_standalone() {
    check_emit(
        "(ann (int 5) (record x (sym Int) y (sym Int)))",
        "(5:{x:@Int, y:@Int})",
    );
}

#[test]
fn emit_hole() {
    check_emit(r#"(hole unbound-name (str "foo"))"#, "_");
}

#[test]
fn emit_string_escapes() {
    check_emit(
        r#"(str "hello \"world\"\n\tback\\slash\r")"#,
        r#""hello \"world\"\n\tback\\slash\r""#,
    );
}

#[test]
fn emit_sym() {
    check_emit("(sym write)", "@write");
}

#[test]
fn emit_single_rec() {
    check_emit("(rec (var 0) (var 0))", "rec {B0 = B0} in B0");
}

#[test]
fn emit_lam_in_let_rhs_no_parens() {
    // rhs extends to `in`; lambda should NOT be wrapped in parens.
    check_emit(
        "(let (lam (var 0)) (app (var 0) (int 5)))",
        "let v0 = lambda v1. v1 in v0 5",
    );
}

// -------------------------------------------------------------------------
// Authoring → canonical parse tests
// -------------------------------------------------------------------------

fn check_parse(authoring: &str, expected_canonical: &str) {
    let (node, _sc) = parse_authoring(authoring.as_bytes())
        .unwrap_or_else(|e| panic!("parse {:?}: {}", authoring, e));
    let got = String::from_utf8(emit(&node)).unwrap();
    assert_eq!(
        got, expected_canonical,
        "parse mismatch\n  input:    {}\n  expected: {}\n  got:      {}",
        authoring, expected_canonical, got
    );
}

#[test]
fn parse_identity_lambda() {
    check_parse("lambda x. x", "(lam (var 0))");
}

#[test]
fn parse_let_simple() {
    check_parse("let x = 1 in x", "(let (int 1) (var 0))");
}

#[test]
fn parse_app_left_assoc() {
    check_parse(
        "lambda f. lambda x. lambda y. f x y",
        "(lam (lam (lam (app (app (var 2) (var 1)) (var 0)))))",
    );
}

#[test]
fn parse_rec_mutual() {
    check_parse(
        "rec {f = lambda x. g x; g = lambda x. f x} in f 10",
        "(rec (lam (app (var 2) (var 0))) (lam (app (var 1) (var 0))) (app (var 0) (int 10)))",
    );
}

#[test]
fn parse_if_expr() {
    check_parse(
        "lambda n. if n then n else 0",
        "(lam (if (var 0) (var 0) (int 0)))",
    );
}

#[test]
fn parse_match_arm() {
    // Stack: xs=0 after lam. In Cons arm: h is first pat-var (index 1), t last (index 0).
    check_parse(
        "lambda xs. match xs with | Nil => 0 | Cons h t => h",
        "(lam (match (var 0) (arm (pat-ctor Nil) (int 0)) (arm (pat-ctor Cons pat-var pat-var) (var 1))))",
    );
}

#[test]
fn parse_record_literal() {
    // Fields canonicalize to alphabetical order: x before y.
    check_parse("{y: 2, x: 1}", "(record x (int 1) y (int 2))");
}

#[test]
fn parse_ann_standalone() {
    check_parse("(5:@Int)", "(ann (int 5) (sym Int))");
}

#[test]
fn parse_sym() {
    check_parse("@write 1 2", "(app (app (sym write) (int 1)) (int 2))");
}

#[test]
fn parse_nested_app_parens() {
    // f, g, x need to be in scope.
    check_parse(
        "lambda f. lambda g. lambda x. f (g x)",
        "(lam (lam (lam (app (var 2) (app (var 1) (var 0))))))",
    );
}

#[test]
fn parse_projection() {
    check_parse("lambda r. r.x.y", "(lam (proj (proj (var 0) x) y))");
}

#[test]
fn parse_let_type_ann() {
    check_parse(
        "let x:@Int = 5 in x",
        "(let (ann (int 5) (sym Int)) (var 0))",
    );
}

#[test]
fn parse_ctor_in_match() {
    check_parse(
        "lambda n. match n with | Zero => 1 | Succ m => m",
        "(lam (match (var 0) (arm (pat-ctor Zero) (int 1)) (arm (pat-ctor Succ pat-var) (var 0))))",
    );
}

// -------------------------------------------------------------------------
// Sidecar round-trip: authoring → canonical + sidecar → authoring (same text)
// -------------------------------------------------------------------------

#[test]
fn sidecar_preserves_names() {
    let authoring = "let id = lambda x. x in id 5";
    let (node, sidecar) = parse_authoring(authoring.as_bytes()).unwrap();
    let roundtripped = emit_authoring(&node, Some(&sidecar));
    assert_eq!(
        roundtripped, authoring,
        "sidecar names not preserved\n  original:     {}\n  roundtripped: {}",
        authoring, roundtripped
    );
}

#[test]
fn sidecar_rec_names_preserved() {
    let authoring = "rec {even = lambda n. odd n; odd = lambda n. even n} in even 10";
    let (node, sidecar) = parse_authoring(authoring.as_bytes()).unwrap();
    let roundtripped = emit_authoring(&node, Some(&sidecar));
    assert_eq!(
        roundtripped, authoring,
        "rec sidecar names not preserved\n  original:     {}\n  roundtripped: {}",
        authoring, roundtripped
    );
}

#[test]
fn sidecar_names_preserved_inside_app_arg() {
    // A binder inside an application argument should keep its name through emit.
    let authoring = "let f = lambda x. x in f (lambda y. y)";
    let (node, sidecar) = parse_authoring(authoring.as_bytes()).unwrap();
    let roundtripped = emit_authoring(&node, Some(&sidecar));
    assert_eq!(
        roundtripped, authoring,
        "binder name inside app arg not preserved\n  original:     {}\n  roundtripped: {}",
        authoring, roundtripped
    );
}

#[test]
fn parse_rec_typed_binding() {
    // First-pass scan must stop at `=`, not skip past it to `;`/`}`.
    check_parse(
        "rec {x:@Int = 1} in x",
        "(rec (ann (int 1) (sym Int)) (var 0))",
    );
}

#[test]
fn parse_module_one_binding() {
    check_parse("module {f = lambda x. x}", "(module (lam (var 0)))");
}

#[test]
fn parse_module_two_bindings() {
    check_parse("module {a = 1; b = 2}", "(module (int 1) (int 2))");
}

#[test]
fn parse_pat_int_match() {
    check_parse(
        "match 5 with | 5 => 1 | _ => 0",
        "(match (int 5) (arm (pat-int 5) (int 1)) (arm pat-wild (int 0)))",
    );
}

#[test]
fn parse_buf_alloc_sym() {
    // `@buf-alloc` should lex as At + Ident("buf-alloc") (ADR 0038).
    check_parse(
        "let buf = @buf-alloc 256 in @read 0 buf 256",
        "(let (app (sym buf-alloc) (int 256)) (app (app (app (sym read) (int 0)) (var 0)) (int 256)))",
    );
}

#[test]
fn parse_unit_state_decl_and_state_field_arg() {
    let (node, _) = parse_authoring(
        b"unit Demo {state Self = {ram: u8vec, pc: Int}; export public set_pc : Int -> Int / {Mut} = lambda x. @state-store pc x}",
    )
    .unwrap();
    let canonical = String::from_utf8(tacit_canonical::emit(&node)).unwrap();
    assert!(
        canonical.contains("(state Self (record pc (sym Int) ram (sym u8vec)))"),
        "{canonical}"
    );
    assert!(
        canonical.contains("(app (app (sym state-store) (sym pc)) (var 0))"),
        "{canonical}"
    );
}

#[test]
fn state_field_arg_round_trips_without_at_prefix() {
    let authoring =
        "unit Demo {state Self = {ram: u8vec}; export public len : Int -> Int = lambda x. let v = @state-load ram in @u8vec-len v}";
    let (node, sidecar) = parse_authoring(authoring.as_bytes()).unwrap();
    let roundtripped = emit_authoring(&node, Some(&sidecar));
    assert!(
        roundtripped.contains("@state-load ram"),
        "roundtrip should render state field as a bare field name: {roundtripped}"
    );
}

#[test]
fn parse_hole_recovery_unknown_token() {
    // An unexpected token in expression position should produce a Hole node, not a hard error.
    let result = parse_authoring("lambda x. => x".as_bytes());
    assert!(result.is_ok(), "expected Ok with Hole, got {:?}", result);
    let (node, _sc) = result.unwrap();
    // The body should be an App(Hole, Var(0)).
    let canonical = String::from_utf8(tacit_canonical::emit(&node)).unwrap();
    assert!(
        canonical.contains("hole"),
        "expected hole in canonical: {}",
        canonical
    );
}
