//! Bundle C ordering primitive type/effect fixtures.

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
fn sort_i64_program_typechecks() {
    let typed = infer_authoring(
        "let xs = @i64-alloc 3 in
         let _ = @i64-set xs 0 3 in
         let _ = @i64-set xs 1 1 in
         let _ = @i64-set xs 2 2 in
         let _ = @sort-i64 xs 3 in
         @i64-get xs 0",
    );
    assert_eq!(typed.ty, Ty::Int);
    assert!(typed.effects.atoms.contains(&EffAtom::Alloc));
    assert!(typed.effects.atoms.contains(&EffAtom::Mut));
}

#[test]
fn sort_ranges_by_bytes_program_typechecks() {
    let typed = infer_authoring(
        "let text = @buf-alloc 4 in
         let rows = @i64-alloc 4 in
         let _ = @sort-ranges-by-bytes text rows 2 in
         @range-start rows 0",
    );
    assert_eq!(typed.ty, Ty::Int);
    assert!(typed.effects.atoms.contains(&EffAtom::Alloc));
    assert!(typed.effects.atoms.contains(&EffAtom::Mut));
}

#[test]
fn stable_sort_pairs_i64_program_typechecks() {
    let typed = infer_authoring(
        "let keys = @i64-alloc 2 in
         let vals = @i64-alloc 2 in
         let _ = @stable-sort-pairs-i64 keys vals 2 in
         @i64-get vals 0",
    );
    assert_eq!(typed.ty, Ty::Int);
    assert!(typed.effects.atoms.contains(&EffAtom::Alloc));
    assert!(typed.effects.atoms.contains(&EffAtom::Mut));
}
