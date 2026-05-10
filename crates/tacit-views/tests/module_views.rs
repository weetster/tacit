use tacit_canonical::emit;
use tacit_canonical::hash_node;
use tacit_views::authoring::{emit_authoring, parse_authoring};
use tacit_views::inspection::{emit_inspection, InspectFlags};

#[test]
fn parses_authoring_module_with_import_and_export() {
    let import_hash = "0".repeat(64);
    let src = format!(
        "module Math {{ import increment : Int -> Int from blake3:{}; export public add_one : Int -> Int = lambda x. increment x }}",
        import_hash
    );
    let (node, sidecar) = parse_authoring(src.as_bytes()).expect("parse module");
    let canonical = String::from_utf8(emit(&node)).unwrap();
    assert!(canonical.starts_with("(unit (imports"));
    assert!(canonical
        .contains("(ref \"0000000000000000000000000000000000000000000000000000000000000000\")"));
    assert_eq!(sidecar.module_alias.as_deref(), Some("Math"));
    assert_eq!(
        sidecar
            .import_aliases
            .as_ref()
            .and_then(|m| m.get(&import_hash))
            .map(String::as_str),
        Some("increment")
    );
}

#[test]
fn parses_order_independent_private_local_reference() {
    let src = "module Math { export public use_id : Int -> Int = lambda x. id x; private id : Int -> Int = lambda y. y }";
    let (node, sidecar) = parse_authoring(src.as_bytes()).expect("parse module");
    let rendered = emit_authoring(&node, Some(&sidecar));
    assert!(rendered.contains("private id : Int -> Int = lambda"));
    assert!(rendered.contains("export public use_id : Int -> Int = lambda"));
}

#[test]
fn inspection_shows_module_boundaries() {
    let src = "module Math { export public id : Int -> Int = lambda x. x }";
    let (node, sidecar) = parse_authoring(src.as_bytes()).expect("parse module");
    let out = emit_inspection(
        &node,
        Some(&sidecar),
        &InspectFlags {
            types: true,
            effects: true,
            ..Default::default()
        },
    );
    assert!(out.starts_with("module Math\nimports"));
    assert!(out.contains("exports\n  public id : Int -> Int = blake3:"));
    assert!(out.contains("definitions\n  id = lambda"));
}

#[test]
fn emitted_authoring_refs_use_sidecar_alias() {
    let src = "module Math { private id : Int -> Int = lambda y. y; export public use_id : Int -> Int = lambda x. id x }";
    let (node, sidecar) = parse_authoring(src.as_bytes()).expect("parse module");
    let text = emit_authoring(&node, Some(&sidecar));
    assert!(text.contains("id v0"), "{text}");
    assert!(!text.contains("__tacit_module_ref__"));
    let _ = hash_node(&node);
}

#[test]
fn parses_function_call_effect_in_boundary_type() {
    let src = "module Effects { export public f : Int -> Int / {IO} = lambda x. x }";
    let (node, sidecar) = parse_authoring(src.as_bytes()).expect("parse module");
    let text = emit_authoring(&node, Some(&sidecar));
    assert!(text.contains("Int -> Int / {IO}"), "{text}");
}
