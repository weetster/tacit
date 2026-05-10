use tacit_canonical::emit;
use tacit_canonical::hash_node;
use tacit_views::authoring::{emit_authoring, parse_authoring};
use tacit_views::inspection::{emit_inspection, InspectFlags};

#[test]
fn parses_authoring_unit_with_import_and_export() {
    let import_hash = "0".repeat(64);
    let src = format!(
        "unit Math {{ import increment : Int -> Int from blake3:{}; export public add_one : Int -> Int = lambda x. increment x }}",
        import_hash
    );
    let (node, sidecar) = parse_authoring(src.as_bytes()).expect("parse unit");
    let canonical = String::from_utf8(emit(&node)).unwrap();
    assert!(canonical.starts_with("(unit (imports"));
    assert!(canonical
        .contains("(ref \"0000000000000000000000000000000000000000000000000000000000000000\")"));
    assert_eq!(sidecar.unit_alias.as_deref(), Some("Math"));
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
    let src = "unit Math { export public use_id : Int -> Int = lambda x. id x; private id : Int -> Int = lambda y. y }";
    let (node, sidecar) = parse_authoring(src.as_bytes()).expect("parse unit");
    let rendered = emit_authoring(&node, Some(&sidecar));
    assert!(rendered.contains("private id : Int -> Int = lambda"));
    assert!(rendered.contains("export public use_id : Int -> Int = lambda"));
}

#[test]
fn inspection_shows_unit_boundaries() {
    let src = "unit Math { export public id : Int -> Int = lambda x. x }";
    let (node, sidecar) = parse_authoring(src.as_bytes()).expect("parse unit");
    let out = emit_inspection(
        &node,
        Some(&sidecar),
        &InspectFlags {
            types: true,
            effects: true,
            ..Default::default()
        },
    );
    assert!(out.starts_with("unit Math\nimports"));
    assert!(out.contains("exports\n  public id : Int -> Int = blake3:"));
    assert!(out.contains("definitions\n  id = lambda"));
}

#[test]
fn emitted_authoring_refs_use_sidecar_alias() {
    let src = "unit Math { private id : Int -> Int = lambda y. y; export public use_id : Int -> Int = lambda x. id x }";
    let (node, sidecar) = parse_authoring(src.as_bytes()).expect("parse unit");
    let text = emit_authoring(&node, Some(&sidecar));
    assert!(text.contains("id v0"), "{text}");
    assert!(!text.contains("__tacit_unit_ref__"));
    let _ = hash_node(&node);
}

#[test]
fn parses_function_call_effect_in_boundary_type() {
    let src = "unit Effects { export public f : Int -> Int / {IO} = lambda x. x }";
    let (node, sidecar) = parse_authoring(src.as_bytes()).expect("parse unit");
    let text = emit_authoring(&node, Some(&sidecar));
    assert!(text.contains("Int -> Int / {IO}"), "{text}");
}

#[test]
fn rejects_empty_module_binding_group() {
    let err = parse_authoring(b"module {}").expect_err("empty module binding group is invalid");
    assert!(err
        .to_string()
        .contains("module requires at least one binding"));
}

#[test]
fn rejects_named_module_binding_group() {
    let err = parse_authoring(b"module Math { f = lambda x. x }")
        .expect_err("named grouping must use unit syntax");
    assert!(err.to_string().contains("expected '{'"));
}

#[test]
fn rejects_empty_logical_unit() {
    let err = parse_authoring(b"unit Empty {}").expect_err("empty unit is invalid");
    assert!(err
        .to_string()
        .contains("unit requires at least one definition"));
}
