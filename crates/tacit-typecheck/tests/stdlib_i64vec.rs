//! ADR 0061 I64Vec primitive type/effect fixtures.

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
fn i64vec_get_set_program_typechecks() {
    let typed = infer_authoring(
        "let xs = @i64-alloc 3 in
         let _ = @i64-set xs 0 7 in
         let _ = @i64-set xs 1 -2 in
         let _ = @i64-set xs 2 10 in
         @add (@i64-get xs 0) (@i64-get xs 2)",
    );
    assert_eq!(typed.ty, Ty::Int);
    assert!(typed.effects.atoms.contains(&EffAtom::Alloc));
    assert!(typed.effects.atoms.contains(&EffAtom::Mut));
    assert_eq!(typed.effects.atoms.len(), 2);
}

#[test]
fn i64vec_copy_program_typechecks() {
    let typed = infer_authoring(
        "let src = @i64-alloc 2 in
         let _ = @i64-set src 0 11 in
         let _ = @i64-set src 1 22 in
         let dst = @i64-alloc 2 in
         let _ = @i64-copy dst 0 src 0 2 in
         @i64-get dst 1",
    );
    assert_eq!(typed.ty, Ty::Int);
    assert!(typed.effects.atoms.contains(&EffAtom::Alloc));
    assert!(typed.effects.atoms.contains(&EffAtom::Mut));
}
