use std::collections::BTreeMap;
use std::process::Command;

use tacit_canonical::ast::Node;
use tacit_canonical::{emit, hash_node};
use tacit_views::sidecar::{Sidecar, SidecarNode};

fn tacit_bin() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_tacit"))
}

fn tacit(args: &[&str], dir: &std::path::Path) -> std::process::Output {
    Command::new(tacit_bin())
        .args(args)
        .current_dir(dir)
        .output()
        .expect("failed to spawn tacit binary")
}

/// Round-trip: write .taca → canonicalize → render --authoring → canonicalize again.
/// The two canonical hashes must match (hash stability).
#[test]
fn canonicalize_render_hash_stability() {
    let dir = tempfile::tempdir().expect("tempdir");
    let d = dir.path();

    // Write a small authoring-view program.
    let taca = d.join("prog.taca");
    std::fs::write(&taca, b"let n = 42 in n").unwrap();

    // Step 1: canonicalize .taca → .tac + .tacd
    let out1 = tacit(&["canonicalize", "prog.taca"], d);
    assert!(
        out1.status.success(),
        "canonicalize step 1 failed: {}",
        String::from_utf8_lossy(&out1.stderr)
    );
    assert!(d.join("prog.tac").exists(), ".tac not written");
    assert!(d.join("prog.tacd").exists(), ".tacd not written");

    // Step 2: render --as authoring → prog.taca2
    let out2 = tacit(
        &[
            "render",
            "prog.tac",
            "--as",
            "authoring",
            "-o",
            "prog2.taca",
        ],
        d,
    );
    assert!(
        out2.status.success(),
        "render step 2 failed: {}",
        String::from_utf8_lossy(&out2.stderr)
    );
    assert!(d.join("prog2.taca").exists(), "rendered .taca not written");

    // Step 3: canonicalize the re-rendered authoring view
    let out3 = tacit(&["canonicalize", "prog2.taca", "-o", "prog2.tac"], d);
    assert!(
        out3.status.success(),
        "canonicalize step 3 failed: {}",
        String::from_utf8_lossy(&out3.stderr)
    );

    // Assert canonical bytes are identical.
    let bytes1 = std::fs::read(d.join("prog.tac")).unwrap();
    let bytes2 = std::fs::read(d.join("prog2.tac")).unwrap();
    assert_eq!(
        bytes1,
        bytes2,
        "canonical bytes differ after round-trip: {:?} vs {:?}",
        String::from_utf8_lossy(&bytes1),
        String::from_utf8_lossy(&bytes2),
    );
}

/// canonicalize refuses to overwrite an existing .tac without --force.
#[test]
fn canonicalize_refuses_overwrite_without_force() {
    let dir = tempfile::tempdir().expect("tempdir");
    let d = dir.path();

    std::fs::write(d.join("prog.taca"), b"let n = 1 in n").unwrap();
    std::fs::write(d.join("prog.tac"), b"existing content").unwrap();

    let out = tacit(&["canonicalize", "prog.taca"], d);
    assert!(!out.status.success(), "expected failure without --force");

    // --force succeeds.
    let out2 = tacit(&["canonicalize", "--force", "prog.taca"], d);
    assert!(
        out2.status.success(),
        "expected success with --force: {}",
        String::from_utf8_lossy(&out2.stderr)
    );
}

/// render --as authoring writes to stdout when no -o is given.
#[test]
fn render_authoring_to_stdout() {
    let dir = tempfile::tempdir().expect("tempdir");
    let d = dir.path();

    std::fs::write(d.join("prog.taca"), b"let x = 7 in x").unwrap();
    let out1 = tacit(&["canonicalize", "prog.taca"], d);
    assert!(out1.status.success());

    let out2 = tacit(&["render", "prog.tac"], d);
    assert!(
        out2.status.success(),
        "{}",
        String::from_utf8_lossy(&out2.stderr)
    );
    let rendered = String::from_utf8_lossy(&out2.stdout);
    assert!(!rendered.is_empty(), "stdout should not be empty");
}

/// render --as authoring rejects an output path that doesn't end in .taca.
#[test]
fn render_authoring_rejects_non_taca_output() {
    let dir = tempfile::tempdir().expect("tempdir");
    let d = dir.path();

    std::fs::write(d.join("prog.taca"), b"let x = 7 in x").unwrap();
    let _ = tacit(&["canonicalize", "prog.taca"], d);

    let out = tacit(&["render", "prog.tac", "-o", "out.txt"], d);
    assert!(
        !out.status.success(),
        "expected failure for .txt output path"
    );
}

/// load_canonical: view accepts both .tac and .taca input.
#[test]
fn view_accepts_tac_and_taca() {
    let dir = tempfile::tempdir().expect("tempdir");
    let d = dir.path();

    let src = b"let n = 99 in n";
    std::fs::write(d.join("prog.taca"), src).unwrap();

    // view a .taca (authoring) file directly
    let out1 = tacit(&["view", "--as", "authoring", "prog.taca"], d);
    assert!(
        out1.status.success(),
        "{}",
        String::from_utf8_lossy(&out1.stderr)
    );

    // canonicalize, then view the .tac (canonical) file
    let _ = tacit(&["canonicalize", "prog.taca"], d);
    let out2 = tacit(&["view", "--as", "authoring", "prog.tac"], d);
    assert!(
        out2.status.success(),
        "{}",
        String::from_utf8_lossy(&out2.stderr)
    );
}

#[test]
fn check_accepts_project_directory() {
    let dir = tempfile::tempdir().expect("tempdir");
    let d = dir.path();
    std::fs::create_dir_all(d.join("src")).unwrap();

    let provider = cli_identity_def();
    let provider_hash = cli_hash(&provider);
    let provider_unit = Node::Unit {
        imports: vec![],
        exports: vec![Node::Export {
            visibility: "package".into(),
            hash: provider_hash.clone(),
        }],
        defs: vec![provider],
    };
    let consumer = cli_apply_import_def(&provider_hash);
    let consumer_hash = cli_hash(&consumer);
    let consumer_unit = Node::Unit {
        imports: vec![Node::Import {
            hash: provider_hash,
            sig: Box::new(cli_int_to_int_sig()),
        }],
        exports: vec![Node::Export {
            visibility: "public".into(),
            hash: consumer_hash,
        }],
        defs: vec![consumer],
    };

    std::fs::write(d.join("src/provider.tac"), emit(&provider_unit)).unwrap();
    std::fs::write(d.join("src/consumer.tac"), emit(&consumer_unit)).unwrap();

    let out = tacit(&["check", ".", "--format", "json"], d);
    assert!(
        out.status.success(),
        "project check failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains(r#""errors": []"#), "{stdout}");
}

#[test]
fn view_accepts_project_directory_as_inspection() {
    let dir = tempfile::tempdir().expect("tempdir");
    let d = dir.path();
    let (_entry_hash, _provider_unit, main_unit) = write_cli_project(d);

    let out = tacit(&["view", "--as", "inspection", "."], d);
    assert!(
        out.status.success(),
        "project view failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("project blake3:"), "{stdout}");
    assert!(stdout.contains("unit views"), "{stdout}");
    assert!(stdout.contains(&cli_hash(&main_unit)), "{stdout}");
}

#[cfg(feature = "llvm")]
#[test]
fn compile_project_directory_to_ir_by_alias() {
    let dir = tempfile::tempdir().expect("tempdir");
    let d = dir.path();
    let (_entry_hash, _provider_unit, _main_unit) = write_cli_project(d);

    let out = tacit(&["compile", ".", "--entry", "main", "--emit-llvm-ir"], d);
    assert!(
        out.status.success(),
        "project compile failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("define i32 @main()"), "{stdout}");
    assert!(stdout.contains("ret i32 42"), "{stdout}");
    assert!(d.join(".tacit/derived").exists());
}

#[test]
fn lock_and_check_package_path_dependency_cli() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dep = dir.path().join("dep");
    let app = dir.path().join("app");
    std::fs::create_dir_all(dep.join("src")).unwrap();
    std::fs::create_dir_all(app.join("src")).unwrap();

    let provider = cli_const_int_def("40");
    let provider_hash = cli_hash(&provider);
    let provider_unit = Node::Unit {
        imports: vec![],
        exports: vec![Node::Export {
            visibility: "public".into(),
            hash: provider_hash.clone(),
        }],
        defs: vec![provider],
    };
    std::fs::write(dep.join("src/lib.tac"), emit(&provider_unit)).unwrap();

    let main = cli_add_import_const_def(&provider_hash, "2");
    let main_hash = cli_hash(&main);
    let main_unit = Node::Unit {
        imports: vec![Node::Import {
            hash: provider_hash,
            sig: Box::new(cli_int_sig()),
        }],
        exports: vec![Node::Export {
            visibility: "public".into(),
            hash: main_hash,
        }],
        defs: vec![main],
    };
    std::fs::write(app.join("src/main.tac"), emit(&main_unit)).unwrap();
    std::fs::write(
        app.join("tacit.toml"),
        "[dependencies]\nutil = { path = \"../dep\" }\n",
    )
    .unwrap();

    let lock = tacit(&["lock", "."], &app);
    assert!(
        lock.status.success(),
        "package lock failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&lock.stdout),
        String::from_utf8_lossy(&lock.stderr)
    );
    assert!(app.join("tacit.lock").exists());

    let check = tacit(&["check", ".", "--format", "json"], &app);
    assert!(
        check.status.success(),
        "package check failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );
    let stdout = String::from_utf8_lossy(&check.stdout);
    assert!(stdout.contains(r#""errors": []"#), "{stdout}");
}

fn write_cli_project(d: &std::path::Path) -> (String, Node, Node) {
    std::fs::create_dir_all(d.join("src")).unwrap();

    let provider = cli_const_int_def("40");
    let provider_hash = cli_hash(&provider);
    let provider_unit = Node::Unit {
        imports: vec![],
        exports: vec![Node::Export {
            visibility: "package".into(),
            hash: provider_hash.clone(),
        }],
        defs: vec![provider],
    };

    let main = cli_add_import_const_def(&provider_hash, "2");
    let main_hash = cli_hash(&main);
    let main_unit = Node::Unit {
        imports: vec![Node::Import {
            hash: provider_hash,
            sig: Box::new(cli_int_sig()),
        }],
        exports: vec![Node::Export {
            visibility: "public".into(),
            hash: main_hash.clone(),
        }],
        defs: vec![main],
    };

    std::fs::write(d.join("src/provider.tac"), emit(&provider_unit)).unwrap();
    let main_bytes = emit(&main_unit);
    std::fs::write(d.join("src/main.tac"), &main_bytes).unwrap();

    let mut export_aliases = BTreeMap::new();
    export_aliases.insert(main_hash.clone(), "main".to_string());
    Sidecar::new(
        &main_bytes,
        SidecarNode {
            export_aliases: Some(export_aliases),
            ..Default::default()
        },
    )
    .write(&d.join("src/main.tacd"))
    .unwrap();

    (main_hash, provider_unit, main_unit)
}

fn cli_sym(name: &str) -> Node {
    Node::Sym { name: name.into() }
}

fn cli_int_sig() -> Node {
    Node::Sig {
        type_: Box::new(cli_sym("Int")),
        eval_eff: Box::new(Node::EffSet { atoms: vec![] }),
    }
}

fn cli_int_to_int_sig() -> Node {
    Node::Sig {
        type_: Box::new(Node::FnTy {
            arg: Box::new(cli_sym("Int")),
            ret: Box::new(cli_sym("Int")),
            eff: Box::new(Node::EffSet { atoms: vec![] }),
        }),
        eval_eff: Box::new(Node::EffSet { atoms: vec![] }),
    }
}

fn cli_identity_def() -> Node {
    Node::Def {
        sig: Box::new(cli_int_to_int_sig()),
        body: Box::new(Node::Lam {
            body: Box::new(Node::Var { index: 0 }),
        }),
    }
}

fn cli_const_int_def(value: &str) -> Node {
    Node::Def {
        sig: Box::new(cli_int_sig()),
        body: Box::new(Node::Int {
            value: value.into(),
        }),
    }
}

fn cli_apply_import_def(import_hash: &str) -> Node {
    Node::Def {
        sig: Box::new(cli_int_to_int_sig()),
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

fn cli_add_import_const_def(import_hash: &str, value: &str) -> Node {
    Node::Def {
        sig: Box::new(cli_int_sig()),
        body: Box::new(Node::App {
            fn_: Box::new(Node::App {
                fn_: Box::new(cli_sym("add")),
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

fn cli_hash(node: &Node) -> String {
    hash_node(node)
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect()
}
