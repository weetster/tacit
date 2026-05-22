use std::collections::BTreeMap;

use tacit_canonical::ast::Node;
use tacit_canonical::emit;
use tacit_canonical::hash_node;
use tacit_views::authoring::{emit_authoring, parse_authoring};
use tacit_views::inspection::{emit_inspection, InspectFlags};
use tacit_views::sidecar::SidecarNode;

fn sym(name: &str) -> Node {
    Node::Sym { name: name.into() }
}

fn int_to_int_sig() -> Node {
    Node::Sig {
        type_: Box::new(Node::FnTy {
            arg: Box::new(sym("Int")),
            ret: Box::new(sym("Int")),
            eff: Box::new(Node::EffSet { atoms: vec![] }),
        }),
        eval_eff: Box::new(Node::EffSet { atoms: vec![] }),
    }
}

fn identity_def() -> Node {
    Node::Def {
        sig: Box::new(int_to_int_sig()),
        body: Box::new(Node::Lam {
            body: Box::new(Node::Var { index: 0 }),
        }),
    }
}

fn apply_ref_def(hash: &str) -> Node {
    Node::Def {
        sig: Box::new(int_to_int_sig()),
        body: Box::new(Node::Lam {
            body: Box::new(Node::App {
                fn_: Box::new(Node::Ref { hash: hash.into() }),
                arg: Box::new(Node::Var { index: 0 }),
            }),
        }),
    }
}

fn hash_hex(node: &Node) -> String {
    hash_node(node)
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}

fn duplicate_alias_unit() -> (Node, SidecarNode, String, String) {
    let helper = identity_def();
    let helper_hash = hash_hex(&helper);
    let public = apply_ref_def(&helper_hash);
    let public_hash = hash_hex(&public);
    let mut definition_aliases = BTreeMap::new();
    definition_aliases.insert(helper_hash.clone(), "same".to_string());
    definition_aliases.insert(public_hash.clone(), "same".to_string());
    let sidecar = SidecarNode {
        unit_alias: Some("Stale".to_string()),
        definition_aliases: Some(definition_aliases),
        ..Default::default()
    };
    let unit = Node::Unit {
        imports: vec![],
        exports: vec![Node::Export {
            visibility: "public".into(),
            hash: public_hash.clone(),
        }],
        defs: vec![helper, public],
    };
    (unit, sidecar, helper_hash, public_hash)
}

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
fn parses_authoring_unit_with_effectful_import_type() {
    let import_hash = "1".repeat(64);
    let src = format!(
        "unit Math {{ import hi_byte : Int -> Int / {{Div}} from blake3:{}; export public pass : Int -> Int = lambda x. x }}",
        import_hash
    );
    let (node, sidecar) = parse_authoring(src.as_bytes()).expect("parse effectful import");
    let canonical = String::from_utf8(emit(&node)).unwrap();
    assert!(canonical.contains("(eff-set Div)"), "{canonical}");
    let authoring = emit_authoring(&node, Some(&sidecar));
    assert!(
        authoring.contains("import hi_byte : Int -> Int / {Div} from blake3:"),
        "{authoring}"
    );
    assert_eq!(
        sidecar
            .import_aliases
            .as_ref()
            .and_then(|m| m.get(&import_hash))
            .map(String::as_str),
        Some("hi_byte")
    );
}

#[test]
fn parses_order_independent_private_local_reference() {
    let src = "unit Math { export public use_id : Int -> Int = lambda x. id x; private id : Int -> Int = lambda y. y }";
    let (node, sidecar) = parse_authoring(src.as_bytes()).expect("parse unit");
    let rendered = emit_authoring(&node, Some(&sidecar));
    assert!(rendered.starts_with("unit Math {\n  "), "{rendered}");
    assert!(rendered.ends_with("\n}"), "{rendered}");
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
fn stale_duplicate_sidecar_aliases_fall_back_in_authoring() {
    let (unit, sidecar, helper_hash, public_hash) = duplicate_alias_unit();
    let text = emit_authoring(&unit, Some(&sidecar));

    assert!(!text.contains("same :"), "{text}");
    assert!(
        text.contains(&format!("private def_{}", &helper_hash[..8])),
        "{text}"
    );
    assert!(
        text.contains(&format!("export public def_{}", &public_hash[..8])),
        "{text}"
    );
    assert!(text.contains(&format!("blake3:{}", helper_hash)), "{text}");

    let (round_tripped, _) = parse_authoring(text.as_bytes()).expect("synthetic aliases parse");
    assert_eq!(emit(&round_tripped), emit(&unit));
}

#[test]
fn stale_duplicate_sidecar_aliases_fall_back_in_inspection() {
    let (unit, sidecar, helper_hash, public_hash) = duplicate_alias_unit();
    let text = emit_inspection(&unit, Some(&sidecar), &InspectFlags::default());

    assert!(!text.contains("same :"), "{text}");
    assert!(
        text.contains(&format!(
            "private\n  def_{} : Int -> Int",
            &helper_hash[..8]
        )),
        "{text}"
    );
    assert!(
        text.contains(&format!("public export_{} : Int -> Int", &public_hash[..8])),
        "{text}"
    );
    assert!(
        text.contains(&format!("blake3:{}...", &helper_hash[..8])),
        "{text}"
    );
}

#[test]
fn parses_function_call_effect_in_boundary_type() {
    let src = "unit Effects { export public f : Int -> Int / {IO} = lambda x. x }";
    let (node, sidecar) = parse_authoring(src.as_bytes()).expect("parse unit");
    let text = emit_authoring(&node, Some(&sidecar));
    assert!(text.contains("Int -> Int / {IO}"), "{text}");
}

#[test]
fn parses_and_renders_host_imports() {
    let src = "unit Host { import host log_byte : u8 -> Int / {IO} from capability \"tacit.host.log\" operation \"write-byte\"; export public call_log : u8 -> Int / {IO} = lambda x. log_byte x }";
    let (node, sidecar) = parse_authoring(src.as_bytes()).expect("parse host import unit");
    let canonical = String::from_utf8(emit(&node)).unwrap();
    assert!(canonical.contains("(host-imp \"tacit.host.log\" \"write-byte\""));
    let authoring = emit_authoring(&node, Some(&sidecar));
    assert!(authoring.contains("import host log_byte : u8 -> Int / {IO}"));
    let inspection = emit_inspection(
        &node,
        Some(&sidecar),
        &InspectFlags {
            types: true,
            effects: true,
            ..Default::default()
        },
    );
    assert!(inspection.contains("host log_byte : u8 -> Int / {IO}"));
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

#[test]
fn unit_keyword_in_expression_position_uses_unit_diagnostic() {
    let src = b"lambda x. unit Math { export public id : Int -> Int = lambda y. y }";
    let (node, _sidecar) = parse_authoring(src).expect("parser should recover with a hole");
    let canonical = String::from_utf8(emit(&node)).unwrap();
    assert!(
        canonical.contains("(hole invalid-unit-artifact"),
        "expected unit-specific hole diagnostic, got: {canonical}"
    );
    assert!(
        !canonical.contains("module-binding-error"),
        "unit expression recovery should not use the module diagnostic: {canonical}"
    );
}
