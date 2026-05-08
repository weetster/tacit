use tacit_typecheck::infer_module;
use tacit_typecheck::ty::EffAtom;
use tacit_views::authoring::parse_authoring;

fn infer_authoring(src: &str) -> tacit_typecheck::TypedModule {
    let (ast, _) =
        parse_authoring(src.as_bytes()).unwrap_or_else(|e| panic!("parse failed for {src:?}: {e}"));
    infer_module(&ast).unwrap_or_else(|diags| {
        let msgs: Vec<String> = diags
            .iter()
            .map(|d| format!("{}: {}", d.kind, d.message))
            .collect();
        panic!("typecheck failed for {src:?}:\n{}", msgs.join("\n"));
    })
}

#[test]
fn map_accepts_pure_callback_and_adds_mut() {
    let typed = infer_authoring(
        "let xs = @i64-alloc 1 in
         let ys = @i64-alloc 1 in
         @map xs 1 (lambda x. @add x 1) ys",
    );
    assert!(typed.effects.atoms.contains(&EffAtom::Alloc));
    assert!(typed.effects.atoms.contains(&EffAtom::Mut));
}

#[test]
fn fold_accepts_effectful_final_callback() {
    let typed = infer_authoring(
        "let xs = @i64-alloc 1 in
         let _ = @i64-set xs 0 1 in
         @fold xs 1 0 (lambda acc. lambda x.
           let _ = @write 1 \"x\" 1 in
           @add acc x)",
    );
    assert!(typed.effects.atoms.contains(&EffAtom::Alloc));
    assert!(typed.effects.atoms.contains(&EffAtom::Mut));
    assert!(typed.effects.atoms.contains(&EffAtom::IO));
}

#[test]
fn for_each_accepts_effectful_callback() {
    let typed = infer_authoring(
        "let xs = @i64-alloc 1 in
         @for-each xs 1 (lambda x.
           let _ = @write 1 \"x\" 1 in
           x)",
    );
    assert!(typed.effects.atoms.contains(&EffAtom::Alloc));
    assert!(typed.effects.atoms.contains(&EffAtom::IO));
}
