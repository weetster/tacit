//! Phase 3 Stage 3 exit-gate tests.
//!
//! Exercises the three carry-over programs from ADR 0046 §3 end-to-end:
//! authoring view → compile → link → run.
//!
//! Gated on the `llvm` feature aggregate (turned on by any of the per-version
//! `llvm<N>-<M>` features). Without the feature this module is empty.

#![cfg(feature = "llvm")]

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use tacit_codegen::compile::compile_to_object;
use tacit_views::authoring::parse_authoring;

fn phase3_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("phase-3")
}

fn pick_linker() -> Option<String> {
    for cand in ["cc", "clang", "gcc"] {
        if which(cand) {
            return Some(cand.to_string());
        }
    }
    None
}

fn which(cmd: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {} >/dev/null 2>&1", cmd))
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

struct Built {
    exe: PathBuf,
    _tmp: tempfile::TempDir,
}

fn build(program_path: &Path, name: &str) -> Built {
    let src = std::fs::read(program_path).expect("read program");
    let (node, _sidecar) = parse_authoring(&src).expect("parse authoring view");
    let tmp = tempfile::tempdir().expect("tempdir");
    let obj = tmp.path().join(format!("{}.o", name));
    compile_to_object(&node, name, &obj).expect("emit object");
    let exe = tmp.path().join(name);
    let linker = pick_linker()
        .expect("no C linker found (cc/clang/gcc); install Xcode CLT or build-essential");
    let status = Command::new(&linker)
        .arg(&obj)
        .arg("-o")
        .arg(&exe)
        .status()
        .expect("invoke linker");
    assert!(status.success(), "linker failed for {}", name);
    Built { exe, _tmp: tmp }
}

fn run(built: &Built) -> (Vec<u8>, i32) {
    let out = Command::new(&built.exe).output().expect("run exe");
    (out.stdout, out.status.code().unwrap_or(-1))
}

fn run_with_stdin(built: &Built, stdin_bytes: &[u8]) -> (Vec<u8>, i32) {
    let mut child = Command::new(&built.exe)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("spawn exe");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin_bytes)
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait");
    (out.stdout, out.status.code().unwrap_or(-1))
}

fn build_phase3(stem: &str) -> Built {
    let path = phase3_root().join(format!("{}.tac", stem));
    build(&path, stem)
}

// ── sort ─────────────────────────────────────────────────────────────────────

#[test]
fn sort_produces_sorted_output() {
    // insertion sort over [5,3,1,4,2] → stdout "1\n2\n3\n4\n5\n", exit 0
    let built = build_phase3("sort");
    let (out, code) = run(&built);
    assert_eq!(out, b"1\n2\n3\n4\n5\n");
    assert_eq!(code, 0);
}

// ── list ─────────────────────────────────────────────────────────────────────

#[test]
fn list_sum_returns_15() {
    // recursive sum of [1,2,3,4,5] → exit code 15
    let built = build_phase3("list");
    let (out, code) = run(&built);
    assert!(out.is_empty(), "expected no stdout, got {:?}", out);
    assert_eq!(code, 15);
}

// ── sum-numbers ───────────────────────────────────────────────────────────────

#[test]
fn sum_numbers_three_lines() {
    // 1+2+3 = 6 → stdout "6\n", exit 0
    let built = build_phase3("sum-numbers");
    let (out, code) = run_with_stdin(&built, b"1\n2\n3\n");
    assert_eq!(out, b"6\n");
    assert_eq!(code, 0);
}

#[test]
fn sum_numbers_single_line() {
    // single number → stdout echoes it, exit 0
    let built = build_phase3("sum-numbers");
    let (out, code) = run_with_stdin(&built, b"42\n");
    assert_eq!(out, b"42\n");
    assert_eq!(code, 0);
}

#[test]
fn sum_numbers_empty_input() {
    // no input → sum is 0 → stdout "0\n", exit 0
    let built = build_phase3("sum-numbers");
    let (out, code) = run_with_stdin(&built, b"");
    assert_eq!(out, b"0\n");
    assert_eq!(code, 0);
}
