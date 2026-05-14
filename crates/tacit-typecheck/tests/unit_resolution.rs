use std::collections::BTreeMap;

use tacit_canonical::ast::Node;
use tacit_canonical::hash_node;
use tacit_typecheck::{
    check_unit, check_unit_with_sidecar, check_units_in_memory, DefinitionEnv,
    DefinitionVisibility, ProvidedDefinition,
};
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

fn fake_hash(ch: char) -> String {
    ch.to_string().repeat(64)
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

fn ref_def(ref_hash: &str) -> Node {
    Node::Def {
        sig: Box::new(int_to_int_sig()),
        body: Box::new(Node::Ref {
            hash: ref_hash.into(),
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
fn definition_body_signature_mismatch_is_reported() {
    let def = Node::Def {
        sig: Box::new(int_to_int_sig()),
        body: Box::new(Node::Int { value: "1".into() }),
    };
    let def_hash = hash(&def);
    let unit = Node::Unit {
        imports: vec![],
        exports: vec![Node::Export {
            visibility: "public".into(),
            hash: def_hash,
        }],
        defs: vec![def],
    };

    let diags = check_unit(&unit, &BTreeMap::new()).expect_err("definition signature mismatch");
    assert!(diags
        .iter()
        .any(|d| { d.kind == "signature-mismatch" && d.message.contains("definition body") }));
}

#[test]
fn missing_import_is_reported_for_unresolved_hash() {
    let missing = fake_hash('0');
    let def = apply_import_def(&missing);
    let def_hash = hash(&def);
    let unit = Node::Unit {
        imports: vec![Node::Import {
            hash: missing,
            sig: Box::new(int_to_int_sig()),
        }],
        exports: vec![Node::Export {
            visibility: "public".into(),
            hash: def_hash,
        }],
        defs: vec![def],
    };

    let diags = check_unit(&unit, &BTreeMap::new()).expect_err("missing import");
    assert!(diags.iter().any(|d| d.kind == "missing-import"));
}

#[test]
fn unit_diagnostics_include_unambiguous_sidecar_alias() {
    let missing = fake_hash('0');
    let def = apply_import_def(&missing);
    let def_hash = hash(&def);
    let unit = Node::Unit {
        imports: vec![Node::Import {
            hash: missing.clone(),
            sig: Box::new(int_to_int_sig()),
        }],
        exports: vec![Node::Export {
            visibility: "public".into(),
            hash: def_hash,
        }],
        defs: vec![def],
    };
    let mut import_aliases = BTreeMap::new();
    import_aliases.insert(missing, "missing_dep".to_string());
    let sidecar = SidecarNode {
        import_aliases: Some(import_aliases),
        ..Default::default()
    };

    let diags = check_unit_with_sidecar(&unit, &BTreeMap::new(), Some(&sidecar))
        .expect_err("missing import");
    let diag = diags
        .iter()
        .find(|d| d.kind == "missing-import")
        .expect("missing-import diagnostic");
    assert!(diag.message.contains("missing_dep"), "{diag:?}");
    assert!(diag.message.contains("blake3:"), "{diag:?}");
}

#[test]
fn unit_diagnostics_ignore_ambiguous_sidecar_alias() {
    let missing = fake_hash('0');
    let def = apply_import_def(&missing);
    let def_hash = hash(&def);
    let unit = Node::Unit {
        imports: vec![Node::Import {
            hash: missing.clone(),
            sig: Box::new(int_to_int_sig()),
        }],
        exports: vec![Node::Export {
            visibility: "public".into(),
            hash: def_hash,
        }],
        defs: vec![def],
    };
    let mut import_aliases = BTreeMap::new();
    import_aliases.insert(missing, "ambiguous".to_string());
    let mut definition_aliases = BTreeMap::new();
    definition_aliases.insert(fake_hash('1'), "ambiguous".to_string());
    let sidecar = SidecarNode {
        import_aliases: Some(import_aliases),
        definition_aliases: Some(definition_aliases),
        ..Default::default()
    };

    let diags = check_unit_with_sidecar(&unit, &BTreeMap::new(), Some(&sidecar))
        .expect_err("missing import");
    let diag = diags
        .iter()
        .find(|d| d.kind == "missing-import")
        .expect("missing-import diagnostic");
    assert!(!diag.message.contains("ambiguous"), "{diag:?}");
    assert!(diag.message.contains("blake3:"), "{diag:?}");
}

#[test]
fn hash_mismatch_is_reported_for_wrong_provider_object() {
    let requested_hash = fake_hash('a');
    let provider_def = identity_def();
    let consumer_def = apply_import_def(&requested_hash);
    let consumer_hash = hash(&consumer_def);
    let unit = Node::Unit {
        imports: vec![Node::Import {
            hash: requested_hash.clone(),
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
        requested_hash,
        ProvidedDefinition::new(provider_def, DefinitionVisibility::Public, false),
    );

    let diags = check_unit(&unit, &env).expect_err("hash mismatch");
    assert!(diags.iter().any(|d| d.kind == "hash-mismatch"));
}

#[test]
fn external_package_export_visibility_violation_is_reported() {
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
        ProvidedDefinition::new(provider_def, DefinitionVisibility::Package, false),
    );

    let diags = check_unit(&unit, &env).expect_err("visibility violation");
    assert!(diags
        .iter()
        .any(|d| d.kind == "visibility-violation" && d.message.contains("package")));
}

#[test]
fn private_provider_visibility_violation_is_reported() {
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
        ProvidedDefinition::new(provider_def, DefinitionVisibility::Private, true),
    );

    let diags = check_unit(&unit, &env).expect_err("private visibility violation");
    assert!(diags
        .iter()
        .any(|d| d.kind == "visibility-violation" && d.message.contains("private")));
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

    check_units_in_memory(&[provider_unit.clone(), consumer_unit.clone()])
        .expect("same-package import checks");
    check_units_in_memory(&[consumer_unit, provider_unit])
        .expect("same-package import checks independent of unit order");
}

#[test]
fn duplicate_import_diagnostic_names_unit_artifact() {
    let import_hash = fake_hash('0');
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
fn duplicate_export_diagnostic_names_unit_artifact() {
    let def = identity_def();
    let def_hash = hash(&def);
    let unit = Node::Unit {
        imports: vec![],
        exports: vec![
            Node::Export {
                visibility: "public".into(),
                hash: def_hash.clone(),
            },
            Node::Export {
                visibility: "package".into(),
                hash: def_hash,
            },
        ],
        defs: vec![def],
    };

    let diags = check_unit(&unit, &BTreeMap::new()).expect_err("duplicate export");
    assert!(diags
        .iter()
        .any(|d| d.kind == "duplicate-export" && d.message.starts_with("unit exports")));
}

#[test]
fn dangling_export_is_reported() {
    let unit = Node::Unit {
        imports: vec![],
        exports: vec![Node::Export {
            visibility: "public".into(),
            hash: fake_hash('d'),
        }],
        defs: vec![identity_def()],
    };

    let diags = check_unit(&unit, &BTreeMap::new()).expect_err("dangling export");
    assert!(diags.iter().any(|d| d.kind == "dangling-export"));
}

#[test]
fn cyclic_dependency_is_reported_for_provider_graph_cycle() {
    let hash_a = fake_hash('a');
    let hash_b = fake_hash('b');
    let unit = Node::Unit {
        imports: vec![],
        exports: vec![],
        defs: vec![identity_def()],
    };
    let mut env = DefinitionEnv::new();
    env.insert(
        hash_a.clone(),
        ProvidedDefinition::new(ref_def(&hash_b), DefinitionVisibility::Public, false),
    );
    env.insert(
        hash_b,
        ProvidedDefinition::new(ref_def(&hash_a), DefinitionVisibility::Public, false),
    );

    let diags = check_unit(&unit, &env).expect_err("cyclic dependency");
    assert!(diags.iter().any(|d| d.kind == "cyclic-dependency"));
}

#[test]
fn module_binding_group_is_not_a_unit_artifact() {
    let node = Node::Module {
        bindings: vec![Node::Int { value: "1".into() }],
    };

    let diags = check_unit(&node, &BTreeMap::new()).expect_err("not a unit artifact");
    assert!(diags.iter().any(|d| d.kind == "invalid-unit-artifact"));
}

#[test]
fn empty_unit_is_not_a_unit_artifact() {
    let node = Node::Unit {
        imports: vec![],
        exports: vec![],
        defs: vec![],
    };

    let diags = check_unit(&node, &BTreeMap::new()).expect_err("empty unit artifact");
    assert!(diags.iter().any(|d| d.kind == "invalid-unit-artifact"));
}

#[test]
fn unit_artifact_rejects_module_binding_in_defs() {
    let node = Node::Unit {
        imports: vec![],
        exports: vec![],
        defs: vec![Node::Module {
            bindings: vec![Node::Int { value: "1".into() }],
        }],
    };

    let diags = check_unit(&node, &BTreeMap::new()).expect_err("malformed unit artifact");
    assert!(diags.iter().any(|d| d.kind == "invalid-unit-artifact"));
}

#[test]
fn unit_artifact_rejects_private_export_entry() {
    let def = identity_def();
    let def_hash = hash(&def);
    let node = Node::Unit {
        imports: vec![],
        exports: vec![Node::Export {
            visibility: "private".into(),
            hash: def_hash,
        }],
        defs: vec![def],
    };

    let diags = check_unit(&node, &BTreeMap::new()).expect_err("malformed export visibility");
    assert!(diags.iter().any(|d| d.kind == "invalid-unit-artifact"));
}
