//! Bundle F ASCII case + classification primitive type/effect fixtures (ADR 0068).

use tacit_typecheck::ty::EffAtom;
use tacit_typecheck::{infer_module, Ty};

fn infer_authoring(src: &str) -> tacit_typecheck::TypedModule {
    let (ast, _) = tacit_views::authoring::parse_authoring(src.as_bytes())
        .unwrap_or_else(|e| panic!("parse failed for {src:?}: {e}"));
    infer_module(&ast).unwrap_or_else(|diags| {
        let msgs: Vec<_> = diags
            .iter()
            .map(|d| format!("{}: {}", d.kind, d.message))
            .collect();
        panic!("typecheck failed for {src:?}:\n{}", msgs.join("\n"));
    })
}

#[test]
fn ascii_tolower_program_is_pure() {
    let typed = infer_authoring("@ascii-tolower 65");
    assert_eq!(typed.ty, Ty::Int);
    assert!(typed.effects.atoms.is_empty());
}

#[test]
fn ascii_toupper_program_is_pure() {
    let typed = infer_authoring("@ascii-toupper 97");
    assert_eq!(typed.ty, Ty::Int);
    assert!(typed.effects.atoms.is_empty());
}

#[test]
fn ascii_is_alpha_program_is_pure() {
    let typed = infer_authoring("@ascii-is-alpha 90");
    assert_eq!(typed.ty, Ty::Int);
    assert!(typed.effects.atoms.is_empty());
}

#[test]
fn ascii_is_digit_program_is_pure() {
    let typed = infer_authoring("@ascii-is-digit 48");
    assert_eq!(typed.ty, Ty::Int);
    assert!(typed.effects.atoms.is_empty());
}

#[test]
fn ascii_is_space_program_is_pure() {
    let typed = infer_authoring("@ascii-is-space 32");
    assert_eq!(typed.ty, Ty::Int);
    assert!(typed.effects.atoms.is_empty());
}

#[test]
fn ascii_class_composes_with_buf_get_without_io() {
    // The intended use site: classify a byte read from a buffer.
    // Effects come only from the Alloc / Mut on buf-alloc / buf-set.
    let typed = infer_authoring(
        "let buf = @buf-alloc 1 in
         let _ = @buf-set buf 0 97 in
         @ascii-is-alpha (@buf-get buf 0)",
    );
    assert_eq!(typed.ty, Ty::Int);
    assert!(typed.effects.atoms.contains(&EffAtom::Alloc));
    assert!(typed.effects.atoms.contains(&EffAtom::Mut));
    assert!(!typed.effects.atoms.contains(&EffAtom::IO));
}

#[test]
fn ascii_tolower_composes_with_buf_set_round_trip() {
    // Typing must accept @ascii-toupper on the result of @buf-get and feed
    // its output back into @buf-set (matches the ADR's primer example).
    let typed = infer_authoring(
        "let buf = @buf-alloc 1 in
         let _ = @buf-set buf 0 97 in
         let _ = @buf-set buf 0 (@ascii-toupper (@buf-get buf 0)) in
         @buf-get buf 0",
    );
    assert_eq!(typed.ty, Ty::Int);
    assert!(typed.effects.atoms.contains(&EffAtom::Alloc));
    assert!(typed.effects.atoms.contains(&EffAtom::Mut));
    assert!(!typed.effects.atoms.contains(&EffAtom::IO));
}
