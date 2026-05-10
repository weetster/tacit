use std::collections::BTreeMap;

use tacit_canonical::ast::Node;
use tacit_canonical::hash_node;
use tacit_typecheck::{
    check_unit, check_units_in_memory, DefinitionEnv, DefinitionVisibility, ProvidedDefinition,
};

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

fn bool_to_int_sig() -> Node {
    Node::Sig {
        type_: Box::new(Node::FnTy {
            arg: Box::new(sym("Bool")),
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

fn hash(node: &Node) -> String {
    hash_node(node)
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}

fn apply_import_def(import_hash: &str) -> Node {
    Node::Def {
        sig: Box::new(int_to_int_sig()),
        body: Box::new(Node::Lam {
            body: Box::new(Node::App {
                fn_: Box::new(Node::Ref {
                    hash: import_hash.into(),
                }),
                arg: Box::new(Node::Var { index: 0 }),
            }),
        }),
    }
}

#[test]
fn imported_hash_signature_checks() {
    let provider_def = identity_def();
    let provider_hash = hash(&provider_def);
    let consumer_def = apply_import_def(&provider_hash);
    let consumer_hash = hash(&consumer_def);

    let unit = Node::Unit {
        imports: vec![Node::Import {
            hash: provider_hash.clone(),
            sig: Box::new(int_to_int_sig()),
        }],
        exports: vec![Node::Export {
            visibility: "public".into(),
            hash: consumer_hash,
        }],
        defs: vec![consumer_def],
    };

    let mut env = DefinitionEnv::new();
    env.insert(
        provider_hash,
        ProvidedDefinition::new(provider_def, DefinitionVisibility::Public, false),
    );

    let typed = check_unit(&unit, &env).expect("unit checks");
    assert_eq!(typed.definition_types.len(), 1);
}

#[test]
fn import_signature_mismatch_is_reported() {
    let provider_def = identity_def();
    let provider_hash = hash(&provider_def);
    let consumer_def = apply_import_def(&provider_hash);
    let consumer_hash = hash(&consumer_def);

    let unit = Node::Unit {
        imports: vec![Node::Import {
            hash: provider_hash.clone(),
            sig: Box::new(bool_to_int_sig()),
        }],
        exports: vec![Node::Export {
            visibility: "public".into(),
            hash: consumer_hash,
        }],
        defs: vec![consumer_def],
    };

    let mut env = DefinitionEnv::new();
    env.insert(
        provider_hash,
        ProvidedDefinition::new(provider_def, DefinitionVisibility::Public, false),
    );

    let diags = check_unit(&unit, &env).expect_err("signature mismatch");
    assert!(diags.iter().any(|d| d.kind == "signature-mismatch"));
}

#[test]
fn private_local_definition_can_be_referenced_by_hash() {
    let helper = identity_def();
    let helper_hash = hash(&helper);
    let public = apply_import_def(&helper_hash);
    let public_hash = hash(&public);

    let unit = Node::Unit {
        imports: vec![],
        exports: vec![Node::Export {
            visibility: "public".into(),
            hash: public_hash,
        }],
        defs: vec![public, helper],
    };

    check_unit(&unit, &BTreeMap::new()).expect("local private ref checks");
}

#[test]
fn package_exports_resolve_across_units_in_memory() {
    let provider = identity_def();
    let provider_hash = hash(&provider);
    let provider_unit = Node::Unit {
        imports: vec![],
        exports: vec![Node::Export {
            visibility: "package".into(),
            hash: provider_hash.clone(),
        }],
        defs: vec![provider],
    };

    let consumer = apply_import_def(&provider_hash);
    let consumer_hash = hash(&consumer);
    let consumer_unit = Node::Unit {
        imports: vec![Node::Import {
            hash: provider_hash,
            sig: Box::new(int_to_int_sig()),
        }],
        exports: vec![Node::Export {
            visibility: "public".into(),
            hash: consumer_hash,
        }],
        defs: vec![consumer],
    };

    check_units_in_memory(&[provider_unit, consumer_unit]).expect("same-package import checks");
}

#[test]
fn duplicate_import_diagnostic_names_unit_artifact() {
    let import_hash = "0".repeat(64);
    let unit = Node::Unit {
        imports: vec![
            Node::Import {
                hash: import_hash.clone(),
                sig: Box::new(int_to_int_sig()),
            },
            Node::Import {
                hash: import_hash,
                sig: Box::new(int_to_int_sig()),
            },
        ],
        exports: vec![],
        defs: vec![identity_def()],
    };

    let diags = check_unit(&unit, &BTreeMap::new()).expect_err("duplicate import");
    assert!(diags
        .iter()
        .any(|d| d.kind == "duplicate-import" && d.message.starts_with("unit imports")));
}

#[test]
fn module_binding_group_is_not_a_unit_artifact() {
    let node = Node::Module {
        bindings: vec![Node::Int { value: "1".into() }],
    };

    let diags = check_unit(&node, &BTreeMap::new()).expect_err("not a unit artifact");
    assert!(diags.iter().any(|d| d.kind == "invalid-unit-artifact"));
}
