use tacit_canonical::ast::Node;
use tacit_canonical::{emit, hash_node};
use tacit_typecheck::{
    check_package, load_package, load_project, lock_package, package_entry_expression,
    package_hash_for_project,
};

fn sym(name: &str) -> Node {
    Node::Sym { name: name.into() }
}

fn int_sig() -> Node {
    Node::Sig {
        type_: Box::new(sym("Int")),
        eval_eff: Box::new(Node::EffSet { atoms: vec![] }),
    }
}

fn bool_sig() -> Node {
    Node::Sig {
        type_: Box::new(sym("Bool")),
        eval_eff: Box::new(Node::EffSet { atoms: vec![] }),
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

fn const_bool_def(value: bool) -> Node {
    Node::Def {
        sig: Box::new(bool_sig()),
        body: Box::new(Node::Ctor {
            name: if value { "True" } else { "False" }.into(),
            args: vec![],
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

fn write_unit(root: &std::path::Path, rel: &str, unit: &Node) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, emit(unit)).unwrap();
}

fn write_provider_package(root: &std::path::Path, value: &str) -> (String, String) {
    let def = const_int_def(value);
    let def_hash = hash(&def);
    let unit = Node::Unit {
        imports: vec![],
        exports: vec![Node::Export {
            visibility: "public".into(),
            hash: def_hash.clone(),
        }],
        defs: vec![def],
    };
    let unit_hash = hash(&unit);
    write_unit(root, "src/lib.tac", &unit);
    (def_hash, unit_hash)
}

fn consumer_unit(import_hash: &str) -> (Node, String) {
    let main = add_import_const_def(import_hash, "2");
    let main_hash = hash(&main);
    let unit = Node::Unit {
        imports: vec![Node::Import {
            hash: import_hash.into(),
            sig: Box::new(int_sig()),
        }],
        exports: vec![Node::Export {
            visibility: "public".into(),
            hash: main_hash.clone(),
        }],
        defs: vec![main],
    };
    (unit, main_hash)
}

fn write_consumer_package(root: &std::path::Path, import_hash: &str, manifest: &str) -> String {
    let (unit, main_hash) = consumer_unit(import_hash);
    write_unit(root, "src/main.tac", &unit);
    std::fs::write(root.join("tacit.toml"), manifest).unwrap();
    main_hash
}

#[test]
fn path_dependency_locks_checks_and_rebuilds_deterministically() {
    let workspace = tempfile::tempdir().unwrap();
    let dep_root = workspace.path().join("dep");
    let app_root = workspace.path().join("app");
    std::fs::create_dir_all(&dep_root).unwrap();
    std::fs::create_dir_all(&app_root).unwrap();

    let (dep_export_hash, _) = write_provider_package(&dep_root, "40");
    write_consumer_package(
        &app_root,
        &dep_export_hash,
        r#"[dependencies]
util = { path = "../dep" }
"#,
    );

    let missing_lock = load_package(&app_root).expect_err("check requires a lockfile");
    assert!(missing_lock.iter().any(|d| d.kind == "lockfile-drift"));

    let dep_graph = load_project(&dep_root).expect("load dependency project");
    let dep_package_hash = package_hash_for_project(&dep_graph);

    let package = lock_package(&app_root).expect("write lockfile");
    check_package(&package).expect("package checks with path dependency");
    assert_eq!(package.dependencies.len(), 1);
    assert_eq!(package.dependencies[0].hash, dep_package_hash);

    let lock_path = app_root.join("tacit.lock");
    let first_lock = std::fs::read(&lock_path).unwrap();
    let second = lock_package(&app_root).expect("rewrite lockfile");
    check_package(&second).expect("package still checks");
    let second_lock = std::fs::read(&lock_path).unwrap();
    assert_eq!(first_lock, second_lock);

    assert!(app_root
        .join(".tacit/cache/packages")
        .join(&dep_package_hash)
        .join("package.json")
        .exists());
}

#[test]
fn hash_dependency_resolves_from_cache_and_verifies_objects() {
    let workspace = tempfile::tempdir().unwrap();
    let dep_root = workspace.path().join("dep");
    let app_root = workspace.path().join("app");
    std::fs::create_dir_all(&dep_root).unwrap();
    std::fs::create_dir_all(&app_root).unwrap();

    let (dep_export_hash, dep_unit_hash) = write_provider_package(&dep_root, "40");
    write_consumer_package(
        &app_root,
        &dep_export_hash,
        r#"[dependencies]
util = { path = "../dep" }
"#,
    );
    let dep_package_hash = package_hash_for_project(&load_project(&dep_root).unwrap());
    lock_package(&app_root).expect("path dependency materializes cache");

    write_consumer_package(
        &app_root,
        &dep_export_hash,
        &format!(
            "[dependencies]\nutil = {{ hash = \"blake3:{}\" }}\n",
            dep_package_hash
        ),
    );
    let package = lock_package(&app_root).expect("hash dependency lock");
    check_package(&package).expect("hash dependency checks from cache");
    load_package(&app_root).expect("hash dependency verifies from cache");

    let def_object = app_root
        .join(".tacit/cache/objects/defs")
        .join(format!("{}.tac", dep_export_hash));
    std::fs::remove_file(&def_object).unwrap();
    let missing = load_package(&app_root).expect_err("missing cached def object");
    assert!(missing.iter().any(|d| d.kind == "cache-missing-object"));

    write_consumer_package(
        &app_root,
        &dep_export_hash,
        r#"[dependencies]
util = { path = "../dep" }
"#,
    );
    lock_package(&app_root).expect("restore cache from path dependency");
    write_consumer_package(
        &app_root,
        &dep_export_hash,
        &format!(
            "[dependencies]\nutil = {{ hash = \"blake3:{}\" }}\n",
            dep_package_hash
        ),
    );
    lock_package(&app_root).expect("restore hash dependency lock");
    let unit_object = app_root
        .join(".tacit/cache/objects/units")
        .join(format!("{}.tac", dep_unit_hash));
    std::fs::write(&unit_object, b"(int 0)").unwrap();
    let corrupt = load_package(&app_root).expect_err("tampered cached unit object");
    assert!(corrupt.iter().any(|d| d.kind == "cache-corruption"));
    assert!(app_root.join(".tacit/cache/trash").exists());
}

#[test]
fn package_entry_uses_bin_alias_and_expands_dependency_refs() {
    let workspace = tempfile::tempdir().unwrap();
    let dep_root = workspace.path().join("dep");
    let app_root = workspace.path().join("app");
    std::fs::create_dir_all(&dep_root).unwrap();
    std::fs::create_dir_all(&app_root).unwrap();

    let (dep_export_hash, _) = write_provider_package(&dep_root, "40");
    let (consumer, main_hash) = consumer_unit(&dep_export_hash);
    write_unit(&app_root, "src/main.tac", &consumer);
    std::fs::write(
        app_root.join("tacit.toml"),
        format!(
            r#"[dependencies]
util = {{ path = "../dep" }}

[exports]
app = "blake3:{}"

[bin]
main = "app"
"#,
            main_hash
        ),
    )
    .unwrap();

    let package = lock_package(&app_root).expect("lock package with bin alias");
    check_package(&package).expect("package checks");
    let entry = package_entry_expression(&package, Some("main")).expect("resolve bin entry");
    assert_eq!(entry.hash, main_hash);
    let entry_text = String::from_utf8(emit(&entry.expression)).unwrap();
    assert!(entry_text.contains("(int 40)"), "{entry_text}");
    assert!(!entry_text.contains("(ref "), "{entry_text}");
}

#[test]
fn path_dependency_drift_is_reported_against_lockfile() {
    let workspace = tempfile::tempdir().unwrap();
    let dep_root = workspace.path().join("dep");
    let app_root = workspace.path().join("app");
    std::fs::create_dir_all(&dep_root).unwrap();
    std::fs::create_dir_all(&app_root).unwrap();

    let (old_export_hash, _) = write_provider_package(&dep_root, "40");
    write_consumer_package(
        &app_root,
        &old_export_hash,
        r#"[dependencies]
util = { path = "../dep" }
"#,
    );
    lock_package(&app_root).expect("initial lock");

    write_provider_package(&dep_root, "41");
    let drift = load_package(&app_root).expect_err("path dependency hash changed");
    assert!(drift.iter().any(|d| d.kind == "lockfile-drift"));
}

#[test]
fn manifest_schema_errors_use_reserved_diagnostic_kinds() {
    let dir = tempfile::tempdir().unwrap();
    let (export_hash, _) = write_provider_package(dir.path(), "1");

    std::fs::write(
        dir.path().join("tacit.toml"),
        format!(
            "[dependencies]\nutil = {{ hash = \"blake3:{}\", path = \"../util\" }}\n",
            export_hash
        ),
    )
    .unwrap();
    let ambiguous = load_package(dir.path()).expect_err("ambiguous dependency source");
    assert!(ambiguous
        .iter()
        .any(|d| d.kind == "manifest-ambiguous-source"));

    std::fs::write(dir.path().join("tacit.toml"), "[dependencies]\nutil = {}\n").unwrap();
    let missing = load_package(dir.path()).expect_err("missing dependency source");
    assert!(missing.iter().any(|d| d.kind == "manifest-missing-source"));

    std::fs::write(dir.path().join("tacit.toml"), "[tool]\nname = \"x\"\n").unwrap();
    let unknown = load_package(dir.path()).expect_err("unknown manifest field");
    assert!(unknown.iter().any(|d| d.kind == "manifest-unknown-field"));
}

#[test]
fn manifest_tests_parse_effect_policy_and_reject_div() {
    let dir = tempfile::tempdir().unwrap();
    let test_def = const_bool_def(true);
    let test_hash = hash(&test_def);
    let unit = Node::Unit {
        imports: vec![],
        exports: vec![],
        defs: vec![test_def],
    };
    write_unit(dir.path(), "src/tests.tac", &unit);
    std::fs::write(
        dir.path().join("tacit.toml"),
        format!(
            r#"[[tests]]
name = "effectful"
target = "blake3:{}"
effects = ["Alloc", "IO", "Mut"]
"#,
            test_hash
        ),
    )
    .unwrap();

    let package = load_package(dir.path()).expect("manifest tests parse");
    assert_eq!(package.manifest.tests.len(), 1);
    let atoms: Vec<_> = package.manifest.tests[0]
        .effects
        .atoms
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(atoms, vec!["Alloc", "IO", "Mut"]);

    std::fs::write(
        dir.path().join("tacit.toml"),
        format!(
            r#"[[tests]]
name = "div"
target = "blake3:{}"
effects = ["Div"]
"#,
            test_hash
        ),
    )
    .unwrap();
    let div = load_package(dir.path()).expect_err("Div is not manifest-allowed");
    assert!(div.iter().any(|d| d.kind == "manifest-parse"));
}

#[test]
fn duplicate_manifest_tests_use_reserved_diagnostics() {
    let dir = tempfile::tempdir().unwrap();
    let test_def = const_bool_def(true);
    let test_hash = hash(&test_def);
    let unit = Node::Unit {
        imports: vec![],
        exports: vec![],
        defs: vec![test_def],
    };
    write_unit(dir.path(), "src/tests.tac", &unit);
    std::fs::write(
        dir.path().join("tacit.toml"),
        format!(
            r#"[[tests]]
name = "same"
target = "blake3:{0}"

[[tests]]
name = "same"
target = "blake3:{0}"
"#,
            test_hash
        ),
    )
    .unwrap();

    let diags = load_package(dir.path()).expect_err("duplicate tests are invalid");
    assert!(diags.iter().any(|d| d.kind == "duplicate-test-alias"));
    assert!(diags.iter().any(|d| d.kind == "duplicate-test-target"));
}
