//! ADR 0062 text-indexing primitive type/effect fixtures.

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
fn line_index_program_typechecks() {
    let typed = infer_authoring(
        "let text = @buf-alloc 4 in
         let _ = @buf-set text 0 65 in
         let _ = @buf-set text 1 10 in
         let _ = @buf-set text 2 66 in
         let _ = @buf-set text 3 10 in
         let rows = @i64-alloc 8 in
         let count = @line-index text 4 rows in
         @add count (@range-len rows 0)",
    );
    assert_eq!(typed.ty, Ty::Int);
    assert!(typed.effects.atoms.contains(&EffAtom::Alloc));
    assert!(typed.effects.atoms.contains(&EffAtom::Mut));
    assert_eq!(typed.effects.atoms.len(), 2);
}

#[test]
fn token_index_program_typechecks() {
    let typed = infer_authoring(
        "let text = @buf-alloc 5 in
         let _ = @buf-set text 0 32 in
         let _ = @buf-set text 1 65 in
         let _ = @buf-set text 2 32 in
         let _ = @buf-set text 3 66 in
         let _ = @buf-set text 4 32 in
         let rows = @i64-alloc 10 in
         let count = @token-index text 0 5 32 rows in
         @add count (@range-start rows 1)",
    );
    assert_eq!(typed.ty, Ty::Int);
    assert!(typed.effects.atoms.contains(&EffAtom::Alloc));
    assert!(typed.effects.atoms.contains(&EffAtom::Mut));
    assert_eq!(typed.effects.atoms.len(), 2);
}

#[test]
fn token_index_any_program_typechecks_with_string_delims() {
    let typed = infer_authoring(
        "let text = @buf-alloc 5 in
         let _ = @buf-set text 0 65 in
         let _ = @buf-set text 1 44 in
         let _ = @buf-set text 2 66 in
         let _ = @buf-set text 3 59 in
         let _ = @buf-set text 4 67 in
         let rows = @i64-alloc 10 in
         let count = @token-index-any text 0 5 \",;\" 2 rows in
         @add count (@range-len rows 1)",
    );
    assert_eq!(typed.ty, Ty::Int);
    assert!(typed.effects.atoms.contains(&EffAtom::Alloc));
    assert!(typed.effects.atoms.contains(&EffAtom::Mut));
    assert_eq!(typed.effects.atoms.len(), 2);
}

#[test]
fn token_index_any_program_typechecks_with_buffer_delims() {
    let typed = infer_authoring(
        "let text = @buf-alloc 5 in
         let _ = @buf-set text 0 65 in
         let _ = @buf-set text 1 32 in
         let _ = @buf-set text 2 66 in
         let _ = @buf-set text 3 10 in
         let _ = @buf-set text 4 67 in
         let delims = @buf-alloc 2 in
         let _ = @buf-set delims 0 32 in
         let _ = @buf-set delims 1 10 in
         let rows = @i64-alloc 10 in
         let count = @token-index-any text 0 5 delims 2 rows in
         @add count (@range-start rows 2)",
    );
    assert_eq!(typed.ty, Ty::Int);
    assert!(typed.effects.atoms.contains(&EffAtom::Alloc));
    assert!(typed.effects.atoms.contains(&EffAtom::Mut));
    assert_eq!(typed.effects.atoms.len(), 2);
}

#[test]
fn range_accessors_are_pure_after_allocation() {
    let typed = infer_authoring("let rows = @i64-alloc 2 in @range-start rows 0");
    assert_eq!(typed.ty, Ty::Int);
    assert!(typed.effects.atoms.contains(&EffAtom::Alloc));
    assert!(!typed.effects.atoms.contains(&EffAtom::Mut));
    assert_eq!(typed.effects.atoms.len(), 1);
}
