//! Bundle G UTF-8 codepoint primitive type/effect fixtures (ADR 0069).

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
fn utf8_decode_program_is_pure_aside_from_buf_alloc() {
    let typed = infer_authoring(
        "let buf = @buf-alloc 4 in
         @utf8-decode buf 0",
    );
    assert_eq!(typed.ty, Ty::Int);
    assert!(typed.effects.atoms.contains(&EffAtom::Alloc));
    assert!(!typed.effects.atoms.contains(&EffAtom::Mut));
    assert!(!typed.effects.atoms.contains(&EffAtom::IO));
}

#[test]
fn utf8_encode_program_carries_mut() {
    let typed = infer_authoring(
        "let buf = @buf-alloc 4 in
         @utf8-encode buf 0 65",
    );
    assert_eq!(typed.ty, Ty::Int);
    assert!(typed.effects.atoms.contains(&EffAtom::Alloc));
    assert!(typed.effects.atoms.contains(&EffAtom::Mut));
    assert!(!typed.effects.atoms.contains(&EffAtom::IO));
}

#[test]
fn utf8_len_program_is_pure() {
    let typed = infer_authoring("@utf8-len 65");
    assert_eq!(typed.ty, Ty::Int);
    assert!(typed.effects.atoms.is_empty());
}

#[test]
fn utf8_decode_then_unpack_via_div_and_mod() {
    // The intended use site: decode, split packed result into (cp, byte_len)
    // via @div / @mod, recombine. Type stays Int throughout, no IO effect.
    let typed = infer_authoring(
        "let buf = @buf-alloc 4 in
         let _ = @buf-set buf 0 65 in
         let packed = @utf8-decode buf 0 in
         let cp = @div packed 8 in
         let len = @mod packed 8 in
         @add cp len",
    );
    assert_eq!(typed.ty, Ty::Int);
    assert!(typed.effects.atoms.contains(&EffAtom::Alloc));
    assert!(typed.effects.atoms.contains(&EffAtom::Mut));
    assert!(!typed.effects.atoms.contains(&EffAtom::IO));
}

#[test]
fn utf8_encode_composes_with_buf_get() {
    let typed = infer_authoring(
        "let out = @buf-alloc 4 in
         let n = @utf8-encode out 0 233 in
         @add n (@buf-get out 0)",
    );
    assert_eq!(typed.ty, Ty::Int);
    assert!(typed.effects.atoms.contains(&EffAtom::Alloc));
    assert!(typed.effects.atoms.contains(&EffAtom::Mut));
}
