use std::process::Command;

use tacit_canonical::ast::Node;
use tacit_canonical::{emit, hash_node};

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

fn cli_sym(name: &str) -> Node {
    Node::Sym { name: name.into() }
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

fn cli_hash(node: &Node) -> String {
    hash_node(node)
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect()
}
