//! ADR 0085 Stage 7 typed mutable memory primitive type/effect fixtures.

use tacit_typecheck::ty::{EffAtom, FixedIntTy, IntSign};
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

fn infer_authoring_err(src: &str) -> Vec<tacit_typecheck::error::Diagnostic> {
    let (ast, _) = tacit_views::authoring::parse_authoring(src.as_bytes())
        .unwrap_or_else(|e| panic!("parse failed for {src:?}: {e}"));
    match infer_module(&ast) {
        Ok(_) => panic!("expected typecheck error for {src:?}"),
        Err(diags) => diags,
    }
}

#[test]
fn u32vec_round_trip_typechecks() {
    let typed = infer_authoring(
        "let regs = @u32vec-alloc 8 in
         let _    = @u32vec-set regs 0 100 in
         let _    = @u32vec-set regs 7 200 in
         @u32vec-get regs 7",
    );
    assert_eq!(
        typed.ty,
        Ty::FixedInt(FixedIntTy::new(IntSign::Unsigned, 32))
    );
    assert!(typed.effects.atoms.contains(&EffAtom::Alloc));
    assert!(typed.effects.atoms.contains(&EffAtom::Mut));
}

#[test]
fn u8vec_len_is_pure() {
    let typed = infer_authoring(
        "let buf = @u8vec-alloc 32 in
         @u8vec-len buf",
    );
    assert_eq!(typed.ty, Ty::Int);
    assert!(typed.effects.atoms.contains(&EffAtom::Alloc));
    assert!(!typed.effects.atoms.contains(&EffAtom::Mut));
}

#[test]
fn u8vec_byte_bus_load_typechecks() {
    let typed = infer_authoring(
        "let ram = @u8vec-alloc 16 in
         let _   = @u8vec-store-u32-le ram 0 305419896 in
         @u8vec-load-u32-le ram 0",
    );
    assert_eq!(
        typed.ty,
        Ty::FixedInt(FixedIntTy::new(IntSign::Unsigned, 32))
    );
    assert!(typed.effects.atoms.contains(&EffAtom::Alloc));
    assert!(typed.effects.atoms.contains(&EffAtom::Mut));
}

#[test]
fn u8vec_slice_returns_u8vec() {
    let typed = infer_authoring(
        "let v = @u8vec-alloc 16 in
         let s = @u8vec-slice v 2 4 in
         @u8vec-len s",
    );
    assert_eq!(typed.ty, Ty::Int);
}

#[test]
fn u8vec_eq_returns_bool() {
    let typed = infer_authoring(
        "let a = @u8vec-alloc 4 in
         let b = @u8vec-alloc 4 in
         @u8vec-eq a 0 b 0 4",
    );
    assert_eq!(typed.ty, Ty::Bool);
}

#[test]
fn neg_capture_vec_handle_in_closure_rejected() {
    let diags = infer_authoring_err("let regs = @u32vec-alloc 4 in lambda x. @u32vec-get regs x");
    assert!(
        diags.iter().any(|d| d.kind == "invalid-capture"),
        "expected invalid-capture, got: {:?}",
        diags.iter().map(|d| &d.kind).collect::<Vec<_>>()
    );
}

#[test]
fn neg_wrong_vec_type_rejected() {
    let diags = infer_authoring_err(
        "let bytes = @u8vec-alloc 8 in
         @u32vec-get bytes 0",
    );
    assert!(
        diags.iter().any(|d| d.kind == "type-mismatch"),
        "expected type-mismatch, got: {:?}",
        diags.iter().map(|d| &d.kind).collect::<Vec<_>>()
    );
}
