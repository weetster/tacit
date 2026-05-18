use std::collections::BTreeMap;
use std::process::Command;

use serde_json::Value;
use tacit_canonical::ast::Node;
use tacit_canonical::{emit, hash_node};
use tacit_views::authoring::parse_authoring;
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

fn tacit_with_env(
    args: &[&str],
    dir: &std::path::Path,
    envs: &[(&str, &str)],
) -> std::process::Output {
    let mut command = Command::new(tacit_bin());
    command.args(args).current_dir(dir);
    for (key, value) in envs {
        command.env(key, value);
    }
    command.output().expect("failed to spawn tacit binary")
}

#[test]
fn version_json_reports_release_metadata() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = tacit(&["version", "--format", "json"], dir.path());
    assert!(
        out.status.success(),
        "version failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let json: Value = serde_json::from_slice(&out.stdout).expect("version json");
    assert_eq!(json["format"], "tacit-version-v1");
    assert_eq!(json["toolchain_version"], "0.7.6");
    assert_eq!(json["manifest"]["format"], "tacit-toolchain-release-v1");
    assert_eq!(json["manifest"]["toolchain_version"], "0.7.6");
    assert_eq!(
        json["manifest"]["schemas"]["canonical"],
        "tacit-canonical-v1"
    );
    assert_eq!(json["manifest"]["schemas"]["lockfile"], "tacit-lock-v1");
    assert_eq!(json["manifest"]["schemas"]["package"], "tacit-package-v1");
    assert_eq!(json["manifest"]["schemas"]["test_results"], "tacit-test-v1");
    assert_eq!(
        json["manifest"]["schemas"]["toolchain_release"],
        "tacit-toolchain-release-v1"
    );
    assert_eq!(json["manifest"]["assets"]["root"], "share/tacit");
    assert_eq!(json["manifest"]["assets"]["primer"]["id"], "tacit-lite");
    assert_eq!(
        json["manifest"]["assets"]["primer"]["toolchain_version"],
        "0.7.6"
    );
    assert_eq!(
        json["manifest"]["assets"]["primer"]["path"],
        "share/tacit/primer/tacit-lite.md"
    );
    assert_eq!(
        json["manifest"]["assets"]["primer"]["metadata_path"],
        "share/tacit/primer/tacit-lite.toml"
    );
    assert_eq!(
        json["manifest"]["assets"]["primer"]["tokenizer"],
        "o200k_base"
    );
    assert_eq!(json["manifest"]["assets"]["primer"]["tokens"], 26878);
    assert!(json["manifest"]["stdlib"]["tacit.text"]
        .as_str()
        .expect("tacit.text hash")
        .starts_with("blake3:"));
    assert_eq!(
        json["manifest"]["assets"]["stdlib"]["cache_path"],
        "share/tacit/stdlib-cache"
    );
    assert_eq!(
        json["manifest"]["assets"]["stdlib"]["source_path"],
        "share/tacit/stdlib-src/tacit"
    );
    assert_eq!(
        json["manifest"]["assets"]["stdlib"]["packages"]
            .as_array()
            .expect("stdlib packages")
            .len(),
        6
    );
    let release_hash = json["release_hash"].as_str().expect("release hash");
    assert!(
        release_hash.starts_with("blake3:") && release_hash.len() == "blake3:".len() + 64,
        "{release_hash}"
    );
}

#[test]
fn version_flag_uses_toolchain_version() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = tacit(&["--version"], dir.path());
    assert!(
        out.status.success(),
        "version flag failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("0.7.6"), "{stdout}");
}

#[test]
fn version_json_verifies_adjacent_manifest() {
    let dir = tempfile::tempdir().expect("tempdir");
    let asset_root = dir.path().join("share/tacit");
    std::fs::create_dir_all(&asset_root).unwrap();
    std::fs::write(asset_root.join("toolchain-release.json"), b"{}\n").unwrap();
    let asset_root_text = asset_root.display().to_string();

    let out = tacit_with_env(
        &["version", "--format", "json"],
        dir.path(),
        &[("TACIT_TOOLCHAIN_ASSET_ROOT", asset_root_text.as_str())],
    );
    assert!(
        out.status.success(),
        "version failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let json: Value = serde_json::from_slice(&out.stdout).expect("version json");
    assert_eq!(json["installed_manifest"]["status"], "mismatch");
    assert!(json["installed_manifest"]["hash"]
        .as_str()
        .expect("installed hash")
        .starts_with("blake3:"));
}

#[test]
fn primer_text_matches_planning_copy() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = tacit(&["primer"], dir.path());
    assert!(
        out.status.success(),
        "primer failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let planning = std::fs::read(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("plans/primer/tacit-lite-primer.md"),
    )
    .unwrap();
    assert_eq!(out.stdout, planning);
}

#[test]
fn primer_json_reports_hash_and_tokens() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = tacit(&["primer", "--format", "json"], dir.path());
    assert!(
        out.status.success(),
        "primer json failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let json: Value = serde_json::from_slice(&out.stdout).expect("primer json");
    assert_eq!(json["format"], "tacit-primer-v1");
    assert_eq!(json["id"], "tacit-lite");
    assert_eq!(json["version"], "0.7.6");
    assert_eq!(json["toolchain_version"], "0.7.6");
    assert_eq!(json["path"], "share/tacit/primer/tacit-lite.md");
    assert_eq!(json["metadata_path"], "share/tacit/primer/tacit-lite.toml");
    assert_eq!(json["tokenizer"], "o200k_base");
    assert_eq!(json["tokens"], 26878);
    let hash = json["hash"].as_str().expect("primer hash");
    assert!(
        hash.starts_with("blake3:") && hash.len() == "blake3:".len() + 64,
        "{hash}"
    );
}

#[test]
fn primer_check_accepts_exact_bytes_and_rejects_edits() {
    let dir = tempfile::tempdir().expect("tempdir");
    let d = dir.path();
    let primer = tacit(&["primer"], d);
    assert!(primer.status.success());

    std::fs::write(d.join("primer.md"), &primer.stdout).unwrap();
    let ok = tacit(&["primer", "--check", "primer.md", "--format", "json"], d);
    assert!(
        ok.status.success(),
        "primer check failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&ok.stdout),
        String::from_utf8_lossy(&ok.stderr)
    );
    let ok_json: Value = serde_json::from_slice(&ok.stdout).expect("check json");
    assert_eq!(ok_json["format"], "tacit-primer-check-v1");
    assert_eq!(ok_json["ok"], true);

    let mut edited = primer.stdout;
    edited.extend_from_slice(b"\n<!-- edited -->\n");
    std::fs::write(d.join("edited.md"), edited).unwrap();
    let bad = tacit(&["primer", "--check", "edited.md"], d);
    assert!(!bad.status.success(), "edited primer should fail check");
    assert!(
        String::from_utf8_lossy(&bad.stderr).contains("primer hash mismatch"),
        "{}",
        String::from_utf8_lossy(&bad.stderr)
    );
}

#[test]
fn stdlib_list_json_reports_bundled_packages() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = tacit(&["stdlib", "list", "--format", "json"], dir.path());
    assert!(
        out.status.success(),
        "stdlib list failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let json: Value = serde_json::from_slice(&out.stdout).expect("stdlib list json");
    assert_eq!(json["format"], "tacit-stdlib-v1");
    assert_eq!(json["toolchain_version"], "0.7.6");
    assert_eq!(json["cache_path"], "share/tacit/stdlib-cache");
    assert_eq!(json["source_path"], "share/tacit/stdlib-src/tacit");
    let packages = json["packages"].as_array().expect("packages");
    assert_eq!(packages.len(), 6);
    let text = packages
        .iter()
        .find(|package| package["name"] == "tacit.text")
        .expect("tacit.text package");
    assert!(text["hash"].as_str().unwrap().starts_with("blake3:"));
    assert_eq!(
        text["public_exports"]
            .as_array()
            .unwrap()
            .iter()
            .find(|export| export["alias"] == "ascii-is-digit")
            .unwrap()["hash"],
        "blake3:f7babbf21591eeeb64d2c990e40b6be53def9032770ae10c346c7e3132173a5a"
    );
}

#[test]
fn stdlib_seed_allows_hash_dependency_without_repo_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let app = dir.path().join("app");
    std::fs::create_dir_all(app.join("src")).unwrap();

    let seed = tacit(&["stdlib", "seed", "--root", "app"], dir.path());
    assert!(
        seed.status.success(),
        "stdlib seed failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&seed.stdout),
        String::from_utf8_lossy(&seed.stderr)
    );

    let list = tacit(&["stdlib", "list", "--format", "json"], dir.path());
    assert!(list.status.success());
    let json: Value = serde_json::from_slice(&list.stdout).expect("stdlib list json");
    let text = json["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|package| package["name"] == "tacit.text")
        .expect("tacit.text package");
    let text_package_hash = text["hash"].as_str().unwrap();
    let ascii_is_digit = text["public_exports"]
        .as_array()
        .unwrap()
        .iter()
        .find(|export| export["alias"] == "ascii-is-digit")
        .unwrap()["hash"]
        .as_str()
        .unwrap()
        .strip_prefix("blake3:")
        .unwrap()
        .to_string();

    let test_def = cli_apply_int_to_bool_import_def(&ascii_is_digit, "57");
    let test_hash = cli_hash(&test_def);
    let unit = Node::Unit {
        imports: vec![Node::Import {
            hash: ascii_is_digit,
            sig: Box::new(cli_int_to_bool_sig()),
        }],
        exports: vec![Node::Export {
            visibility: "public".into(),
            hash: test_hash.clone(),
        }],
        defs: vec![test_def],
    };
    std::fs::write(app.join("src/main.tac"), emit(&unit)).unwrap();
    std::fs::write(
        app.join("tacit.toml"),
        format!(
            "[package]\nname = \"stdlib-seeded-consumer\"\n\n[dependencies]\ntext = {{ hash = \"{}\", source = {{ registry = \"builtin\", name = \"tacit.text\" }} }}\n\n[exports]\ndigit = \"blake3:{}\"\n",
            text_package_hash, test_hash
        ),
    )
    .unwrap();

    let lock = tacit(&["lock", "."], &app);
    assert!(
        lock.status.success(),
        "lock failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&lock.stdout),
        String::from_utf8_lossy(&lock.stderr)
    );
    let check = tacit(&["check", ".", "--format", "json"], &app);
    assert!(
        check.status.success(),
        "check failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );

    let lockfile = std::fs::read_to_string(app.join("tacit.lock")).unwrap();
    assert!(lockfile.contains(r#""registry": "builtin""#), "{lockfile}");
    assert!(!lockfile.contains(r#""path":"#), "{lockfile}");
    assert!(app
        .join(".tacit/cache/packages")
        .join(text_package_hash.strip_prefix("blake3:").unwrap())
        .join("manifest.toml")
        .exists());
}

#[cfg(feature = "llvm")]
#[test]
fn init_executable_project_passes_lock_check_test_and_compile() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = tacit(&["init", "hello"], dir.path());
    assert!(
        out.status.success(),
        "init failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let project = dir.path().join("hello");
    assert!(project.join("tacit-toolchain.toml").exists());
    assert!(project.join("tacit.toml").exists());
    assert!(project.join("tacit.lock").exists());
    assert!(project.join("AGENTS.md").exists());
    assert!(project.join("CLAUDE.md").exists());
    assert!(project.join("src/main.tac").exists());
    assert!(project.join("src/main.tacd").exists());
    assert!(
        !project.join("src/main.taca").exists(),
        "init must not generate .taca files"
    );

    let manifest = std::fs::read_to_string(project.join("tacit.toml")).unwrap();
    assert!(manifest.contains("[exports]"), "{manifest}");
    assert!(manifest.contains("[bin]"), "{manifest}");
    assert!(manifest.contains("[[tests]]"), "{manifest}");

    let pin = std::fs::read_to_string(project.join("tacit-toolchain.toml")).unwrap();
    assert!(pin.contains("format = \"tacit-toolchain-pin-v1\""), "{pin}");
    assert!(pin.contains("\"tacit.text\" = \"blake3:"), "{pin}");
    let agents = std::fs::read_to_string(project.join("AGENTS.md")).unwrap();
    assert!(agents.contains("tacit primer"), "{agents}");
    assert_eq!(
        agents,
        std::fs::read_to_string(project.join("CLAUDE.md")).unwrap()
    );

    let check = tacit(&["check", ".", "--format", "json"], &project);
    assert!(
        check.status.success(),
        "check failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );
    let lock = tacit(&["lock", "."], &project);
    assert!(
        lock.status.success(),
        "lock failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&lock.stdout),
        String::from_utf8_lossy(&lock.stderr)
    );
    let tests = tacit(&["test", ".", "--format", "json"], &project);
    assert!(
        tests.status.success(),
        "test failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&tests.stdout),
        String::from_utf8_lossy(&tests.stderr)
    );
    let test_json: Value = serde_json::from_slice(&tests.stdout).unwrap();
    assert_eq!(test_json["outcome"], "pass");
    assert_eq!(test_json["summary"]["pass"], 1);

    let compile = tacit(&["compile", ".", "--emit-llvm-ir"], &project);
    assert!(
        compile.status.success(),
        "compile failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr)
    );
    assert!(String::from_utf8_lossy(&compile.stdout).contains("ret i32 0"));
}

#[cfg(feature = "llvm")]
#[test]
fn init_library_with_stdlib_passes_lock_check_test_and_interface_library() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = tacit(
        &["init", "math-lib", "--template", "library", "--with-stdlib"],
        dir.path(),
    );
    assert!(
        out.status.success(),
        "init failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let project = dir.path().join("math-lib");
    let manifest = std::fs::read_to_string(project.join("tacit.toml")).unwrap();
    assert!(manifest.contains("[dependencies]"), "{manifest}");
    assert!(manifest.contains("text = { hash = \"blake3:"), "{manifest}");
    assert!(manifest.contains("[exports]"), "{manifest}");
    assert!(!manifest.contains("[bin]"), "{manifest}");
    assert!(project.join(".tacit/cache/packages").exists());

    let check = tacit(&["check", ".", "--format", "json"], &project);
    assert!(
        check.status.success(),
        "check failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );
    let lock = tacit(&["lock", "."], &project);
    assert!(
        lock.status.success(),
        "lock failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&lock.stdout),
        String::from_utf8_lossy(&lock.stderr)
    );
    let tests = tacit(&["test", ".", "--format", "json"], &project);
    assert!(
        tests.status.success(),
        "test failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&tests.stdout),
        String::from_utf8_lossy(&tests.stderr)
    );
    let interface = tacit(&["interface", ".", "--emit-library"], &project);
    assert!(
        interface.status.success(),
        "interface failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&interface.stdout),
        String::from_utf8_lossy(&interface.stderr)
    );
    assert!(
        String::from_utf8_lossy(&interface.stdout).contains(".a"),
        "{}",
        String::from_utf8_lossy(&interface.stdout)
    );
}

#[test]
fn check_warns_but_succeeds_when_pin_is_missing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let d = dir.path();
    std::fs::create_dir_all(d.join("src")).unwrap();
    std::fs::write(
        d.join("src/main.tac"),
        emit(&Node::Unit {
            imports: vec![],
            exports: vec![],
            defs: vec![cli_const_int_def("0")],
        }),
    )
    .unwrap();

    let out = tacit(&["check", ".", "--format", "json"], d);
    assert!(
        out.status.success(),
        "check without pin should warn, not fail\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("warning: ") && stderr.contains("missing tacit-toolchain.toml"),
        "expected missing-pin warning on stderr, got: {stderr}"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains(r#""errors": []"#), "{stdout}");
}

#[test]
fn check_fails_when_pin_toolchain_version_mismatches() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = tacit(&["init", "hello"], dir.path());
    assert!(out.status.success());
    let project = dir.path().join("hello");

    let pin_path = project.join("tacit-toolchain.toml");
    let pin = std::fs::read_to_string(&pin_path).unwrap();
    std::fs::write(
        &pin_path,
        pin.replace("version = \"0.7.6\"\n", "version = \"99.0.0\"\n"),
    )
    .unwrap();

    let check = tacit(&["check", ".", "--format", "json"], &project);
    assert!(
        !check.status.success(),
        "check should fail on mismatched pin\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );
    let stderr = String::from_utf8_lossy(&check.stderr);
    let envelope: Value = serde_json::from_str(stderr.trim()).expect("pin diagnostics json");
    let errors = envelope["errors"].as_array().expect("errors array");
    assert!(
        errors
            .iter()
            .any(|err| err["kind"] == "toolchain-pin-version-mismatch"),
        "expected toolchain-pin-version-mismatch, got {errors:?}"
    );
}

#[test]
fn lock_fails_when_primer_hash_pin_mismatches() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = tacit(&["init", "hello"], dir.path());
    assert!(out.status.success());
    let project = dir.path().join("hello");

    let pin_path = project.join("tacit-toolchain.toml");
    let pin = std::fs::read_to_string(&pin_path).unwrap();
    let bogus = "blake3:dead0000000000000000000000000000000000000000000000000000000000ad";
    let original = pin
        .lines()
        .find(|line| line.starts_with("hash = "))
        .expect("primer hash line")
        .trim_start_matches("hash = ")
        .trim_matches('"');
    std::fs::write(&pin_path, pin.replace(original, bogus)).unwrap();

    let lock = tacit(&["lock", "."], &project);
    assert!(
        !lock.status.success(),
        "lock should fail when primer pin diverges from installed primer"
    );
    let stderr = String::from_utf8_lossy(&lock.stderr);
    let envelope: Value = serde_json::from_str(stderr.trim()).expect("pin diagnostics json");
    let errors = envelope["errors"].as_array().expect("errors array");
    assert!(
        errors
            .iter()
            .any(|err| err["kind"] == "toolchain-pin-primer-mismatch"),
        "expected toolchain-pin-primer-mismatch, got {errors:?}"
    );
}

#[test]
fn check_fails_when_stdlib_pin_hash_diverges() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = tacit(&["init", "hello"], dir.path());
    assert!(out.status.success());
    let project = dir.path().join("hello");

    let pin_path = project.join("tacit-toolchain.toml");
    let pin = std::fs::read_to_string(&pin_path).unwrap();
    let core_line = pin
        .lines()
        .find(|line| line.starts_with("\"tacit.core\" = "))
        .expect("tacit.core line");
    let bogus = "\"tacit.core\" = \"blake3:abad0000000000000000000000000000000000000000000000000000000000ad\"";
    std::fs::write(&pin_path, pin.replace(core_line, bogus)).unwrap();

    let check = tacit(&["check", ".", "--format", "json"], &project);
    assert!(
        !check.status.success(),
        "check should fail when a stdlib pin hash diverges"
    );
    let stderr = String::from_utf8_lossy(&check.stderr);
    let envelope: Value = serde_json::from_str(stderr.trim()).expect("pin diagnostics json");
    let errors = envelope["errors"].as_array().expect("errors array");
    assert!(
        errors
            .iter()
            .any(|err| err["kind"] == "toolchain-pin-stdlib-mismatch"),
        "expected toolchain-pin-stdlib-mismatch, got {errors:?}"
    );
}

#[test]
fn check_reports_body_errors_when_lockfile_is_stale() {
    let dir = tempfile::tempdir().expect("tempdir");
    let d = dir.path();
    std::fs::create_dir_all(d.join("src")).unwrap();

    let valid = cli_const_int_def("0");
    let valid_hash = cli_hash(&valid);
    std::fs::write(
        d.join("src/main.tac"),
        emit(&Node::Unit {
            imports: vec![],
            exports: vec![Node::Export {
                visibility: "public".into(),
                hash: valid_hash.clone(),
            }],
            defs: vec![valid],
        }),
    )
    .unwrap();
    std::fs::write(
        d.join("tacit.toml"),
        format!("[package]\nname = \"stale-check\"\n\n[exports]\nmain = \"blake3:{valid_hash}\"\n"),
    )
    .unwrap();

    let lock = tacit(&["lock", "."], d);
    assert!(
        lock.status.success(),
        "lock failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&lock.stdout),
        String::from_utf8_lossy(&lock.stderr)
    );

    let invalid = cli_invalid_capture_def();
    let invalid_hash = cli_hash(&invalid);
    std::fs::write(
        d.join("src/main.tac"),
        emit(&Node::Unit {
            imports: vec![],
            exports: vec![Node::Export {
                visibility: "public".into(),
                hash: invalid_hash,
            }],
            defs: vec![invalid],
        }),
    )
    .unwrap();

    let check = tacit(&["check", ".", "--format", "json"], d);
    assert!(
        !check.status.success(),
        "check should fail\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );
    let envelope: Value = serde_json::from_slice(&check.stdout).expect("check json");
    let errors = envelope["errors"].as_array().expect("errors array");
    assert!(
        errors.iter().any(|err| err["kind"] == "lockfile-drift"),
        "expected lockfile-drift, got {errors:?}"
    );
    assert!(
        errors.iter().any(|err| err["kind"] == "unresolved-entry"),
        "expected unresolved-entry, got {errors:?}"
    );
    assert!(
        errors.iter().any(|err| err["kind"] == "invalid-capture"),
        "expected invalid-capture, got {errors:?}"
    );
}

#[test]
fn check_fails_when_pin_schema_is_wrong() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = tacit(&["init", "hello"], dir.path());
    assert!(out.status.success());
    let project = dir.path().join("hello");

    let pin_path = project.join("tacit-toolchain.toml");
    let pin = std::fs::read_to_string(&pin_path).unwrap();
    std::fs::write(
        &pin_path,
        pin.replace("tacit-toolchain-pin-v1", "tacit-toolchain-pin-v999"),
    )
    .unwrap();

    let check = tacit(&["check", ".", "--format", "json"], &project);
    assert!(!check.status.success());
    let stderr = String::from_utf8_lossy(&check.stderr);
    let envelope: Value = serde_json::from_str(stderr.trim()).expect("pin diagnostics json");
    let errors = envelope["errors"].as_array().expect("errors array");
    assert!(
        errors
            .iter()
            .any(|err| err["kind"] == "toolchain-pin-schema-mismatch"),
        "expected toolchain-pin-schema-mismatch, got {errors:?}"
    );
}

#[test]
fn init_refuses_non_empty_directory() {
    let dir = tempfile::tempdir().expect("tempdir");
    let project = dir.path().join("existing");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(project.join("note.txt"), "keep me\n").unwrap();

    let out = tacit(&["init", "existing"], dir.path());
    assert!(!out.status.success(), "init should reject non-empty dir");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("directory is not empty"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(project.join("note.txt")).unwrap(),
        "keep me\n"
    );
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

#[test]
fn interface_command_writes_metadata_header_and_rust_bindings() {
    let dir = tempfile::tempdir().expect("tempdir");
    let d = dir.path();
    std::fs::create_dir_all(d.join("src")).unwrap();

    let host_import = Node::HostImport {
        capability: "tacit.host.log".into(),
        operation: "write-byte".into(),
        sig: Box::new(cli_u8_to_int_io_sig()),
    };
    let host_hash = cli_hash(&host_import);
    let export = Node::Def {
        sig: Box::new(cli_u8_to_int_io_sig()),
        body: Box::new(Node::Lam {
            body: Box::new(Node::App {
                fn_: Box::new(Node::Ref { hash: host_hash }),
                arg: Box::new(Node::Var { index: 0 }),
            }),
        }),
    };
    let export_hash = cli_hash(&export);
    let unit = Node::Unit {
        imports: vec![host_import],
        exports: vec![Node::Export {
            visibility: "public".into(),
            hash: export_hash,
        }],
        defs: vec![export],
    };
    std::fs::write(d.join("src/lib.tac"), emit(&unit)).unwrap();

    let out = tacit(&["interface", "."], d);
    assert!(
        out.status.success(),
        "interface failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("interface.json"), "{stdout}");
    assert!(d.join(".tacit/cache").exists());
    assert!(d.join(".tacit/derived").exists());

    let wasm = tacit(&["interface", ".", "--target", "wasm"], d);
    assert!(!wasm.status.success(), "wasm target should fail");
    assert!(String::from_utf8_lossy(&wasm.stderr).contains("abi-unsupported-target"));
}

#[cfg(feature = "llvm")]
#[test]
fn test_package_json_passes_multimodule_private_test() {
    let dir = tempfile::tempdir().expect("tempdir");
    let d = dir.path();
    let test_hash = write_cli_test_package(d, cli_eq_import_const_def, "40");

    let out = tacit(&["test", ".", "--format", "json"], d);
    assert!(
        out.status.success(),
        "package test failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let json: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json["schema_version"], "tacit-test-v1");
    assert_eq!(json["outcome"], "pass");
    assert_eq!(json["summary"]["pass"], 1);
    assert_eq!(json["results"][0]["name"], "provider_matches");
    assert_eq!(
        json["results"][0]["definition_hash"],
        format!("blake3:{test_hash}")
    );
    assert_eq!(json["results"][0]["observed"]["bool"], true);
    assert!(d.join(".tacit/derived").exists());
}

#[cfg(feature = "llvm")]
#[test]
fn test_package_json_reports_bool_false_as_fail() {
    let dir = tempfile::tempdir().expect("tempdir");
    let d = dir.path();
    write_cli_test_package(d, cli_eq_import_const_def, "41");

    let out = tacit(&["test", ".", "--format", "json"], d);
    assert_eq!(out.status.code(), Some(1));
    let json: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json["outcome"], "fail");
    assert_eq!(json["summary"]["fail"], 1);
    assert_eq!(json["results"][0]["status"], "fail");
    assert_eq!(json["results"][0]["observed"]["bool"], false);
}

#[test]
fn test_package_json_reports_signature_mismatch_as_compile_fail() {
    let dir = tempfile::tempdir().expect("tempdir");
    let d = dir.path();
    std::fs::create_dir_all(d.join("src")).unwrap();
    let def = cli_const_int_def("1");
    let def_hash = cli_hash(&def);
    let unit = Node::Unit {
        imports: vec![],
        exports: vec![],
        defs: vec![def],
    };
    std::fs::write(d.join("src/tests.tac"), emit(&unit)).unwrap();
    write_test_manifest(d, "not_bool", &def_hash, "");

    let out = tacit(&["test", ".", "--format", "json"], d);
    assert_eq!(out.status.code(), Some(2));
    let json: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json["outcome"], "error");
    assert_eq!(json["summary"]["compile_fail"], 1);
    assert_eq!(
        json["results"][0]["diagnostics"]["errors"][0]["kind"],
        "test-signature-mismatch"
    );
}

#[test]
fn test_package_json_reports_effect_violation() {
    let dir = tempfile::tempdir().expect("tempdir");
    let d = dir.path();
    std::fs::create_dir_all(d.join("src")).unwrap();
    let def = cli_bool_def_with_effect(["IO"]);
    let def_hash = cli_hash(&def);
    let unit = Node::Unit {
        imports: vec![],
        exports: vec![],
        defs: vec![def],
    };
    std::fs::write(d.join("src/tests.tac"), emit(&unit)).unwrap();
    write_test_manifest(d, "io_test", &def_hash, "");

    let out = tacit(&["test", ".", "--format", "json"], d);
    assert_eq!(out.status.code(), Some(2));
    let json: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json["summary"]["effect_fail"], 1);
    assert_eq!(json["results"][0]["status"], "effect-fail");
    assert_eq!(
        json["results"][0]["declared_effects"],
        serde_json::json!(["IO"])
    );
}

#[cfg(feature = "llvm")]
#[test]
fn test_package_json_reports_runtime_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let d = dir.path();
    std::fs::create_dir_all(d.join("src")).unwrap();
    let def = cli_non_exhaustive_bool_match_def();
    let def_hash = cli_hash(&def);
    let unit = Node::Unit {
        imports: vec![],
        exports: vec![],
        defs: vec![def],
    };
    std::fs::write(d.join("src/tests.tac"), emit(&unit)).unwrap();
    write_test_manifest(d, "runtime_error", &def_hash, "");

    let out = tacit(&["test", ".", "--format", "json"], d);
    assert_eq!(out.status.code(), Some(2));
    let json: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json["summary"]["error"], 1);
    assert_eq!(
        json["results"][0]["diagnostics"]["errors"][0]["kind"],
        "test-runtime-error"
    );
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

#[cfg(feature = "llvm")]
fn write_cli_test_package(
    d: &std::path::Path,
    test_def: fn(&str, &str) -> Node,
    expected_value: &str,
) -> String {
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

    let test = test_def(&provider_hash, expected_value);
    let test_hash = cli_hash(&test);
    let test_unit = Node::Unit {
        imports: vec![Node::Import {
            hash: provider_hash,
            sig: Box::new(cli_int_sig()),
        }],
        exports: vec![],
        defs: vec![test],
    };

    std::fs::write(d.join("src/provider.tac"), emit(&provider_unit)).unwrap();
    std::fs::write(d.join("src/tests.tac"), emit(&test_unit)).unwrap();
    write_test_manifest(d, "provider_matches", &test_hash, "");
    test_hash
}

fn write_test_manifest(d: &std::path::Path, name: &str, target: &str, effects: &str) {
    let effects = if effects.is_empty() {
        String::new()
    } else {
        format!("effects = [{}]\n", effects)
    };
    std::fs::write(
        d.join("tacit.toml"),
        format!(
            "[package]\nname = \"cli-test\"\n\n[[tests]]\nname = \"{}\"\ntarget = \"blake3:{}\"\n{}",
            name, target, effects
        ),
    )
    .unwrap();
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

fn cli_int_alloc_sig() -> Node {
    Node::Sig {
        type_: Box::new(cli_sym("Int")),
        eval_eff: Box::new(Node::EffSet {
            atoms: vec!["Alloc".to_string()],
        }),
    }
}

fn cli_bool_sig() -> Node {
    Node::Sig {
        type_: Box::new(cli_sym("Bool")),
        eval_eff: Box::new(Node::EffSet { atoms: vec![] }),
    }
}

fn cli_bool_sig_with_effect<const N: usize>(atoms: [&str; N]) -> Node {
    Node::Sig {
        type_: Box::new(cli_sym("Bool")),
        eval_eff: Box::new(Node::EffSet {
            atoms: atoms.iter().map(|atom| atom.to_string()).collect(),
        }),
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

fn cli_int_to_bool_sig() -> Node {
    Node::Sig {
        type_: Box::new(Node::FnTy {
            arg: Box::new(cli_sym("Int")),
            ret: Box::new(cli_sym("Bool")),
            eff: Box::new(Node::EffSet { atoms: vec![] }),
        }),
        eval_eff: Box::new(Node::EffSet { atoms: vec![] }),
    }
}

fn cli_u8_to_int_io_sig() -> Node {
    Node::Sig {
        type_: Box::new(Node::FnTy {
            arg: Box::new(cli_sym("u8")),
            ret: Box::new(cli_sym("Int")),
            eff: Box::new(Node::EffSet {
                atoms: vec!["IO".to_string()],
            }),
        }),
        eval_eff: Box::new(Node::EffSet { atoms: vec![] }),
    }
}

fn cli_bool_def_with_effect<const N: usize>(atoms: [&str; N]) -> Node {
    Node::Def {
        sig: Box::new(cli_bool_sig_with_effect(atoms)),
        body: Box::new(cli_eq_ints("1", "1")),
    }
}

#[cfg(feature = "llvm")]
fn cli_non_exhaustive_bool_match_def() -> Node {
    Node::Def {
        sig: Box::new(cli_bool_sig()),
        body: Box::new(Node::Match {
            scrutinee: Box::new(Node::Int { value: "2".into() }),
            arms: vec![Node::Arm {
                pattern: Box::new(Node::PatInt { value: "1".into() }),
                body: Box::new(cli_eq_ints("1", "1")),
            }],
        }),
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

#[cfg(feature = "llvm")]
fn cli_eq_import_const_def(import_hash: &str, value: &str) -> Node {
    Node::Def {
        sig: Box::new(cli_bool_sig()),
        body: Box::new(Node::App {
            fn_: Box::new(Node::App {
                fn_: Box::new(cli_sym("eq")),
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

fn cli_eq_ints(left: &str, right: &str) -> Node {
    Node::App {
        fn_: Box::new(Node::App {
            fn_: Box::new(cli_sym("eq")),
            arg: Box::new(Node::Int { value: left.into() }),
        }),
        arg: Box::new(Node::Int {
            value: right.into(),
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

fn cli_invalid_capture_def() -> Node {
    let (body, _) = parse_authoring(
        b"let buf = @u8vec-alloc 16 in
          let get = lambda i. @u8vec-get buf i in
          get 0",
    )
    .expect("invalid-capture fixture parses");
    Node::Def {
        sig: Box::new(cli_int_alloc_sig()),
        body: Box::new(body),
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

fn cli_apply_int_to_bool_import_def(import_hash: &str, value: &str) -> Node {
    Node::Def {
        sig: Box::new(cli_bool_sig()),
        body: Box::new(Node::App {
            fn_: Box::new(Node::Ref {
                hash: import_hash.into(),
            }),
            arg: Box::new(Node::Int {
                value: value.into(),
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
