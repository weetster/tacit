use std::collections::{BTreeMap, BTreeSet};

use tacit_canonical::ast::Node;
use tacit_canonical::{emit, hash_node};
use tacit_typecheck::{
    check_project, load_project, materialize_project_derived, project_entry_expression,
    DefinitionVisibility, ProjectDefinition, ProjectEntryError, ProjectGraph, ProjectLoadError,
    ProjectUnit,
};
use tacit_views::sidecar::{Sidecar, SidecarNode};

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

fn int_sig() -> Node {
    Node::Sig {
        type_: Box::new(sym("Int")),
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

fn const_int_def(value: &str) -> Node {
    Node::Def {
        sig: Box::new(int_sig()),
        body: Box::new(Node::Int {
            value: value.into(),
        }),
    }
}

fn add_import_const_def(import_hash: &str, value: &str) -> Node {
    Node::Def {
        sig: Box::new(int_sig()),
        body: Box::new(Node::App {
            fn_: Box::new(Node::App {
                fn_: Box::new(sym("add")),
                arg: Box::new(Node::Ref {
                    hash: import_hash.into(),
                }),
            }),
            arg: Box::new(Node::Int {
                value: value.into(),
            }),
        }),
    }
}

fn hash(node: &Node) -> String {
    hash_node(node)
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect()
}

fn fake_hash(ch: char) -> String {
    ch.to_string().repeat(64)
}

fn provider_unit(visibility: Option<&str>) -> (Node, String) {
    let def = identity_def();
    let def_hash = hash(&def);
    let exports = visibility
        .map(|visibility| {
            vec![Node::Export {
                visibility: visibility.into(),
                hash: def_hash.clone(),
            }]
        })
        .unwrap_or_default();
    (
        Node::Unit {
            imports: vec![],
            exports,
            defs: vec![def],
        },
        def_hash,
    )
}

fn consumer_unit(import_hash: &str) -> Node {
    let def = apply_import_def(import_hash);
    let def_hash = hash(&def);
    Node::Unit {
        imports: vec![Node::Import {
            hash: import_hash.into(),
            sig: Box::new(int_to_int_sig()),
        }],
        exports: vec![Node::Export {
            visibility: "public".into(),
            hash: def_hash,
        }],
        defs: vec![def],
    }
}

fn write_unit(root: &std::path::Path, rel: &str, unit: &Node) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, emit(unit)).unwrap();
}

fn write_sidecar(root: &std::path::Path, rel: &str, unit: &Node, display: SidecarNode) {
    let path = root.join(rel).with_extension("tacd");
    let canonical = emit(unit);
    Sidecar::new(&canonical, display).write(&path).unwrap();
}

#[test]
fn project_check_is_independent_of_file_names() {
    let (provider, provider_hash) = provider_unit(Some("package"));
    let consumer = consumer_unit(&provider_hash);

    let first = tempfile::tempdir().unwrap();
    write_unit(first.path(), "src/a-provider.tac", &provider);
    write_unit(first.path(), "src/b-consumer.tac", &consumer);
    let first_graph = load_project(first.path()).expect("load first graph");
    check_project(&first_graph).expect("first project checks");

    let second = tempfile::tempdir().unwrap();
    write_unit(second.path(), "src/z-provider.tac", &provider);
    write_unit(second.path(), "src/y-consumer.tac", &consumer);
    let second_graph = load_project(second.path()).expect("load second graph");
    check_project(&second_graph).expect("second project checks");

    assert_eq!(first_graph.graph_hash, second_graph.graph_hash);
    assert_eq!(
        first_graph
            .units
            .iter()
            .map(|unit| unit.hash.as_str())
            .collect::<Vec<_>>(),
        second_graph
            .units
            .iter()
            .map(|unit| unit.hash.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn project_loader_ignores_missing_and_stale_sidecars() {
    let (unit, _) = provider_unit(Some("public"));
    let dir = tempfile::tempdir().unwrap();
    write_unit(dir.path(), "src/lib.tac", &unit);

    let stale = Sidecar::new(
        b"(int 0)",
        SidecarNode {
            unit_alias: Some("Stale".into()),
            ..Default::default()
        },
    );
    stale.write(&dir.path().join("src/lib.tacd")).unwrap();

    let graph = load_project(dir.path()).expect("load project with stale sidecar");
    assert!(graph.units[0].sidecar.is_none());
    check_project(&graph).expect("stale sidecar is non-semantic");
}

#[test]
fn project_entry_expands_refs_and_materializes_derived_layout() {
    let provider_def = const_int_def("40");
    let provider_hash = hash(&provider_def);
    let provider_unit = Node::Unit {
        imports: vec![],
        exports: vec![Node::Export {
            visibility: "package".into(),
            hash: provider_hash.clone(),
        }],
        defs: vec![provider_def],
    };

    let main_def = add_import_const_def(&provider_hash, "2");
    let main_hash = hash(&main_def);
    let main_unit = Node::Unit {
        imports: vec![Node::Import {
            hash: provider_hash,
            sig: Box::new(int_sig()),
        }],
        exports: vec![Node::Export {
            visibility: "public".into(),
            hash: main_hash.clone(),
        }],
        defs: vec![main_def],
    };
    let mut export_aliases = BTreeMap::new();
    export_aliases.insert(main_hash.clone(), "main".to_string());

    let dir = tempfile::tempdir().unwrap();
    write_unit(dir.path(), "src/provider.tac", &provider_unit);
    write_unit(dir.path(), "src/main.tac", &main_unit);
    write_sidecar(
        dir.path(),
        "src/main.tac",
        &main_unit,
        SidecarNode {
            export_aliases: Some(export_aliases),
            ..Default::default()
        },
    );

    let graph = load_project(dir.path()).expect("load project");
    check_project(&graph).expect("project checks");
    let entry = project_entry_expression(&graph, Some("main")).expect("entry expression");
    assert_eq!(entry.hash, main_hash);
    let entry_text = String::from_utf8(emit(&entry.expression)).unwrap();
    assert!(entry_text.contains("(int 40)"), "{entry_text}");
    assert!(!entry_text.contains("(ref "), "{entry_text}");

    let derived = materialize_project_derived(&graph).expect("write derived layout");
    assert!(derived.join("index/project-graph.json").exists());
    assert!(derived.join("units").read_dir().unwrap().next().is_some());
    assert!(derived.join("defs").read_dir().unwrap().next().is_some());
}

#[test]
fn unresolved_project_import_reports_missing_import() {
    let missing = fake_hash('0');
    let def = Node::Def {
        sig: Box::new(int_sig()),
        body: Box::new(Node::Ref {
            hash: missing.clone(),
        }),
    };
    let def_hash = hash(&def);
    let unit = Node::Unit {
        imports: vec![Node::Import {
            hash: missing,
            sig: Box::new(int_sig()),
        }],
        exports: vec![Node::Export {
            visibility: "public".into(),
            hash: def_hash,
        }],
        defs: vec![def],
    };
    let dir = tempfile::tempdir().unwrap();
    write_unit(dir.path(), "src/main.tac", &unit);

    let graph = load_project(dir.path()).expect("load project");
    let diags = check_project(&graph).expect_err("missing import");
    assert!(diags.iter().any(|diag| diag.kind == "missing-import"));
}

#[test]
fn ambiguous_project_entry_alias_is_rejected() {
    let left_def = const_int_def("1");
    let left_hash = hash(&left_def);
    let left_unit = Node::Unit {
        imports: vec![],
        exports: vec![Node::Export {
            visibility: "public".into(),
            hash: left_hash.clone(),
        }],
        defs: vec![left_def],
    };
    let right_def = const_int_def("2");
    let right_hash = hash(&right_def);
    let right_unit = Node::Unit {
        imports: vec![],
        exports: vec![Node::Export {
            visibility: "public".into(),
            hash: right_hash.clone(),
        }],
        defs: vec![right_def],
    };

    let dir = tempfile::tempdir().unwrap();
    write_unit(dir.path(), "src/left.tac", &left_unit);
    write_unit(dir.path(), "src/right.tac", &right_unit);

    for (rel, unit, hash) in [
        ("src/left.tac", &left_unit, &left_hash),
        ("src/right.tac", &right_unit, &right_hash),
    ] {
        let mut export_aliases = BTreeMap::new();
        export_aliases.insert(hash.clone(), "main".to_string());
        write_sidecar(
            dir.path(),
            rel,
            unit,
            SidecarNode {
                export_aliases: Some(export_aliases),
                ..Default::default()
            },
        );
    }

    let graph = load_project(dir.path()).expect("load project");
    let err = project_entry_expression(&graph, Some("main")).expect_err("ambiguous alias");
    assert!(matches!(err, ProjectEntryError::AmbiguousEntryAlias { .. }));
}

#[test]
fn project_entry_expansion_detects_dependency_cycle() {
    let hash_a = fake_hash('a');
    let hash_b = fake_hash('b');
    let def_a = Node::Def {
        sig: Box::new(int_sig()),
        body: Box::new(Node::Ref {
            hash: hash_b.clone(),
        }),
    };
    let def_b = Node::Def {
        sig: Box::new(int_sig()),
        body: Box::new(Node::Ref {
            hash: hash_a.clone(),
        }),
    };

    let mut definitions = BTreeMap::new();
    definitions.insert(
        hash_a.clone(),
        ProjectDefinition {
            hash: hash_a.clone(),
            def: def_a,
            visibility: DefinitionVisibility::Public,
            unit_hashes: BTreeSet::from([fake_hash('1')]),
        },
    );
    definitions.insert(
        hash_b,
        ProjectDefinition {
            hash: fake_hash('b'),
            def: def_b,
            visibility: DefinitionVisibility::Private,
            unit_hashes: BTreeSet::from([fake_hash('1')]),
        },
    );

    let dir = tempfile::tempdir().unwrap();
    let graph = ProjectGraph {
        root: dir.path().to_path_buf(),
        source_base: dir.path().to_path_buf(),
        graph_hash: fake_hash('9'),
        units: vec![ProjectUnit {
            hash: fake_hash('1'),
            node: Node::Unit {
                imports: vec![],
                exports: vec![],
                defs: vec![],
            },
            sidecar: None,
            source_paths: vec![],
            definition_hashes: vec![hash_a.clone()],
            public_exports: BTreeSet::from([hash_a.clone()]),
            package_exports: BTreeSet::new(),
        }],
        definitions,
    };

    let err = project_entry_expression(&graph, Some(&hash_a)).expect_err("cycle");
    assert!(matches!(err, ProjectEntryError::CyclicDependency(_)));
}

#[test]
fn private_definition_import_reports_visibility_violation() {
    let (provider, provider_hash) = provider_unit(None);
    let consumer = consumer_unit(&provider_hash);
    let dir = tempfile::tempdir().unwrap();
    write_unit(dir.path(), "src/provider.tac", &provider);
    write_unit(dir.path(), "src/consumer.tac", &consumer);

    let graph = load_project(dir.path()).expect("load project");
    let diags = check_project(&graph).expect_err("private import should fail");
    assert!(diags
        .iter()
        .any(|diag| { diag.kind == "visibility-violation" && diag.message.contains("private") }));
}

#[test]
fn project_rejects_non_unit_artifacts() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/main.tac"), b"(int 1)").unwrap();

    let err = load_project(dir.path()).expect_err("non-unit project source");
    assert!(matches!(err, ProjectLoadError::NonUnitArtifact { .. }));
}
