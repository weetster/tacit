//! Stage 6 release validation: exercise the staged share/tacit/ tree (the same
//! tree the binary archive ships) against a project created in a temp dir
//! outside the workspace. Mirrors the validation block in
//! `plans/toolchain-export-plan.md` and ADR 0090.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::SystemTime;

use serde_json::Value;

fn tacit_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_tacit"))
}

fn staged_share() -> PathBuf {
    // The build script for tacit-cli writes share/tacit/ under
    // target/<profile>/build/tacit-cli-<hash>/out. With multiple feature
    // combinations (e.g. with/without llvm19-1) Cargo keeps separate OUT_DIRs;
    // pick the one whose `toolchain-release.json` byte-matches the manifest
    // embedded in *this* binary so version/installed_manifest comparisons
    // line up.
    let embedded_hash = embedded_release_hash();
    let bin = tacit_bin();
    let profile_dir = bin
        .parent()
        .expect("CARGO_BIN_EXE_tacit has a parent directory");
    let build_dir = profile_dir.join("build");

    let mut matching: Option<(SystemTime, PathBuf)> = None;
    for entry in fs::read_dir(&build_dir).expect("read target/build/") {
        let entry = entry.expect("read entry");
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.starts_with("tacit-cli-") {
            continue;
        }
        let share = path.join("out/share/tacit");
        let manifest_path = share.join("toolchain-release.json");
        let Ok(bytes) = fs::read(&manifest_path) else {
            continue;
        };
        let hash = blake3_prefixed(&bytes);
        if hash != embedded_hash {
            continue;
        }
        let mtime = fs::metadata(&share)
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        if matching.as_ref().is_none_or(|(t, _)| *t < mtime) {
            matching = Some((mtime, share));
        }
    }
    matching
        .map(|(_, path)| path)
        .expect("could not locate a staged share/tacit/ whose manifest matches the test binary")
}

fn embedded_release_hash() -> String {
    // Calling `tacit version --format json` with no TACIT_TOOLCHAIN_ASSET_ROOT
    // and a cwd that has no adjacent share/tacit makes installed_manifest
    // report `missing`, while `release_hash` always reports the embedded
    // manifest hash.
    let dir = tempfile::tempdir().expect("tempdir");
    let out = Command::new(tacit_bin())
        .args(["version", "--format", "json"])
        .current_dir(dir.path())
        .env_remove("TACIT_TOOLCHAIN_ASSET_ROOT")
        .output()
        .expect("spawn tacit version");
    assert!(out.status.success(), "tacit version failed");
    let json: Value = serde_json::from_slice(&out.stdout).expect("version json");
    json["release_hash"]
        .as_str()
        .expect("release_hash")
        .to_string()
}

fn blake3_prefixed(bytes: &[u8]) -> String {
    format!(
        "blake3:{}",
        tacit_canonical::hash_bytes(bytes)
            .iter()
            .fold(String::new(), |mut acc, byte| {
                use std::fmt::Write;
                let _ = write!(acc, "{byte:02x}");
                acc
            })
    )
}

fn run_with_asset_root(asset_root: &Path, args: &[&str], cwd: &Path) -> Output {
    Command::new(tacit_bin())
        .args(args)
        .current_dir(cwd)
        .env("TACIT_TOOLCHAIN_ASSET_ROOT", asset_root)
        .output()
        .expect("failed to spawn tacit binary")
}

fn must_succeed(output: &Output, what: &str) {
    assert!(
        output.status.success(),
        "{what} failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn version_against_staged_share_reports_matched_manifest() {
    let asset_root = staged_share();
    let scratch = tempfile::tempdir().expect("tempdir");
    let out = run_with_asset_root(
        &asset_root,
        &["version", "--format", "json"],
        scratch.path(),
    );
    must_succeed(&out, "version --format json");
    let json: Value = serde_json::from_slice(&out.stdout).expect("version json");
    assert_eq!(json["toolchain_version"], "0.7.7");
    assert_eq!(
        json["installed_manifest"]["status"], "matched",
        "expected staged manifest to match embedded copy"
    );
    let release_hash = json["release_hash"].as_str().expect("release hash");
    assert_eq!(
        json["installed_manifest"]["hash"].as_str().expect("hash"),
        release_hash,
        "embedded release hash must equal staged file hash"
    );
}

#[test]
fn primer_check_against_staged_primer_succeeds() {
    let asset_root = staged_share();
    let scratch = tempfile::tempdir().expect("tempdir");
    let primer_path = asset_root.join("primer/tacit-lite.md");
    let primer_str = primer_path.to_string_lossy().to_string();
    let out = run_with_asset_root(
        &asset_root,
        &["primer", "--check", &primer_str, "--format", "json"],
        scratch.path(),
    );
    must_succeed(&out, "primer --check");
    let json: Value = serde_json::from_slice(&out.stdout).expect("check json");
    assert_eq!(json["ok"], true);
}

#[test]
fn staged_toolchain_drives_full_external_project_flow() {
    let asset_root = staged_share();
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf();
    let scratch = tempfile::tempdir().expect("tempdir");

    // Sanity: the temp scratch space must live outside the workspace tree.
    let canonical_scratch = scratch.path().canonicalize().expect("canonicalize scratch");
    let canonical_workspace = workspace_root
        .canonicalize()
        .expect("canonicalize workspace");
    assert!(
        !canonical_scratch.starts_with(&canonical_workspace),
        "scratch dir {} is inside workspace {}",
        canonical_scratch.display(),
        canonical_workspace.display()
    );

    let init = run_with_asset_root(
        &asset_root,
        &["init", "hello", "--with-stdlib"],
        scratch.path(),
    );
    must_succeed(&init, "init");
    let project = scratch.path().join("hello");
    assert!(project.join("tacit-toolchain.toml").exists());
    assert!(project.join("tacit.toml").exists());
    assert!(project.join("tacit.lock").exists());

    let version = run_with_asset_root(&asset_root, &["version", "--format", "json"], &project);
    must_succeed(&version, "project version");

    let primer = run_with_asset_root(&asset_root, &["primer", "--format", "json"], &project);
    must_succeed(&primer, "project primer json");

    let stdlib = run_with_asset_root(
        &asset_root,
        &["stdlib", "list", "--format", "json"],
        &project,
    );
    must_succeed(&stdlib, "stdlib list");
    let stdlib_json: Value = serde_json::from_slice(&stdlib.stdout).expect("stdlib list json");
    assert_eq!(
        stdlib_json["packages"]
            .as_array()
            .expect("stdlib packages")
            .len(),
        6
    );

    let check = run_with_asset_root(&asset_root, &["check", ".", "--format", "json"], &project);
    must_succeed(&check, "check");
    assert!(
        String::from_utf8_lossy(&check.stdout).contains(r#""errors": []"#),
        "check stdout: {}",
        String::from_utf8_lossy(&check.stdout)
    );

    let lock = run_with_asset_root(&asset_root, &["lock", "."], &project);
    must_succeed(&lock, "lock");

    // Test + compile require LLVM. Run them only when the staged manifest
    // reports codegen support, so this test passes both with and without the
    // llvm19-1 feature.
    let version_json: Value = serde_json::from_slice(&version.stdout).expect("version json");
    let codegen = version_json["manifest"]["llvm"]["codegen"]
        .as_bool()
        .unwrap_or(false);
    if !codegen {
        return;
    }

    let tests = run_with_asset_root(&asset_root, &["test", ".", "--format", "json"], &project);
    must_succeed(&tests, "test");
    let test_json: Value = serde_json::from_slice(&tests.stdout).expect("test json");
    assert_eq!(test_json["outcome"], "pass");

    let compile_out = project.join("hello-bin");
    let compile = run_with_asset_root(&asset_root, &["compile", ".", "-o", "hello-bin"], &project);
    must_succeed(&compile, "compile");
    assert!(compile_out.exists());
    let run = Command::new(&compile_out)
        .output()
        .expect("run compiled bin");
    assert!(run.status.success(), "compiled bin failed");
}
