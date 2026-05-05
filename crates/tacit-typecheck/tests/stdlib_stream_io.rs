//! Bundle E stream IO and buf-rev primitive type/effect fixtures (ADR 0067).

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
fn stdin_slurp_program_typechecks_with_io_and_mut() {
    let typed = infer_authoring(
        "let buf = @buf-alloc 64 in
         @stdin-slurp buf 64",
    );
    assert_eq!(typed.ty, Ty::Int);
    assert!(typed.effects.atoms.contains(&EffAtom::IO));
    assert!(typed.effects.atoms.contains(&EffAtom::Mut));
    assert!(typed.effects.atoms.contains(&EffAtom::Alloc));
}

#[test]
fn write_range_program_typechecks_with_io() {
    let typed = infer_authoring(
        "let buf = @buf-alloc 8 in
         let _ = @buf-set buf 0 72 in
         let _ = @buf-set buf 1 105 in
         @write-range 1 buf 0 2",
    );
    assert_eq!(typed.ty, Ty::Int);
    assert!(typed.effects.atoms.contains(&EffAtom::IO));
    assert!(typed.effects.atoms.contains(&EffAtom::Mut));
    assert!(typed.effects.atoms.contains(&EffAtom::Alloc));
}

#[test]
fn buf_rev_program_typechecks_with_mut_only() {
    let typed = infer_authoring(
        "let buf = @buf-alloc 4 in
         let _ = @buf-set buf 0 65 in
         let _ = @buf-set buf 1 66 in
         let _ = @buf-rev buf 0 2 in
         @buf-get buf 0",
    );
    assert_eq!(typed.ty, Ty::Int);
    assert!(typed.effects.atoms.contains(&EffAtom::Mut));
    assert!(typed.effects.atoms.contains(&EffAtom::Alloc));
    assert!(!typed.effects.atoms.contains(&EffAtom::IO));
}
