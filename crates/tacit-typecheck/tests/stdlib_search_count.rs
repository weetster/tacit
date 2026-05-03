//! Bundle D search and adjacent range grouping primitive type/effect fixtures.

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
fn lower_bound_i64_program_typechecks() {
    let typed = infer_authoring(
        "let xs = @i64-alloc 3 in
         let _ = @i64-set xs 0 1 in
         let _ = @i64-set xs 1 4 in
         let _ = @i64-set xs 2 9 in
         @lower-bound-i64 xs 3 5",
    );
    assert_eq!(typed.ty, Ty::Int);
    assert!(typed.effects.atoms.contains(&EffAtom::Alloc));
    assert!(typed.effects.atoms.contains(&EffAtom::Mut));
}

#[test]
fn count_equal_ranges_program_typechecks() {
    let typed = infer_authoring(
        "let text = @buf-alloc 4 in
         let rows = @i64-alloc 4 in
         let out = @i64-alloc 6 in
         let count = @count-equal-ranges text rows 2 out in
         @add count (@i64-get out 2)",
    );
    assert_eq!(typed.ty, Ty::Int);
    assert!(typed.effects.atoms.contains(&EffAtom::Alloc));
    assert!(typed.effects.atoms.contains(&EffAtom::Mut));
}

#[test]
fn dedup_adjacent_ranges_program_typechecks() {
    let typed = infer_authoring(
        "let text = @buf-alloc 4 in
         let rows = @i64-alloc 4 in
         let out = @i64-alloc 4 in
         let count = @dedup-adjacent-ranges text rows 2 out in
         @add count (@range-len out 0)",
    );
    assert_eq!(typed.ty, Ty::Int);
    assert!(typed.effects.atoms.contains(&EffAtom::Alloc));
    assert!(typed.effects.atoms.contains(&EffAtom::Mut));
}
