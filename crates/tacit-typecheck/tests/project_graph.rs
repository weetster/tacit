use tacit_canonical::ast::Node;
use tacit_canonical::{emit, hash_node};
use tacit_typecheck::{check_project, load_project, ProjectLoadError};
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

fn hash(node: &Node) -> String {
    hash_node(node)
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect()
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
