//! Phase 1 Stage 4 exit-gate test: compile each smoke program in
//! `examples/smoke/` end-to-end (authoring text → AST → object → linked
//! executable) and assert the expected stdout / exit code.
//!
//! Gated on the `llvm` feature aggregate (turned on by any of the
//! per-version `llvm<N>-<M>` features). Without the feature, this file
//! compiles to an empty test module — fine for environments without an
//! installed LLVM library.

#![cfg(feature = "llvm")]

use std::path::{Path, PathBuf};
use std::process::Command;

use tacit_codegen::compile::compile_to_object;
use tacit_canonical::parse as parse_canonical;

fn examples_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("smoke")
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
    let node = parse_canonical(&src).expect("parse canonical");

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
    let code = out.status.code().unwrap_or(-1);
    (out.stdout, code)
}

fn run_smoke(file_stem: &str) -> (Vec<u8>, i32) {
    let path = examples_root().join(format!("{}.tac", file_stem));
    let built = build(&path, file_stem);
    run(&built)
}

fn run_with_stdin(built: &Built, stdin_bytes: &[u8]) -> (Vec<u8>, i32) {
    use std::io::Write;
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
    let code = out.status.code().unwrap_or(-1);
    (out.stdout, code)
}

#[test]
fn return_zero() {
    let (out, code) = run_smoke("return-zero");
    assert!(out.is_empty(), "expected empty stdout, got {:?}", out);
    assert_eq!(code, 0);
}

#[test]
fn return_computed() {
    // (5 * 8) - 7 = 33
    let (out, code) = run_smoke("return-computed");
    assert!(out.is_empty());
    assert_eq!(code, 33);
}

#[test]
fn hello_world() {
    let (out, code) = run_smoke("hello");
    assert_eq!(out, b"hello, world\n");
    assert_eq!(code, 0);
}

#[test]
fn if_branch_true() {
    // x = 5; @gt 5 3 = 1 (truthy); takes then branch → 1
    let (out, code) = run_smoke("if-branch");
    assert!(out.is_empty());
    assert_eq!(code, 1);
}

#[test]
fn factorial_5() {
    let (out, code) = run_smoke("factorial");
    assert!(out.is_empty());
    assert_eq!(code, 120);
}

#[test]
fn even_odd_4() {
    // even(4) = odd(3) = even(2) = odd(1) = even(0) = 1
    let (out, code) = run_smoke("even-odd");
    assert!(out.is_empty());
    assert_eq!(code, 1);
}

#[test]
fn exit_nonzero() {
    let (out, code) = run_smoke("exit-nonzero");
    assert!(out.is_empty());
    assert_eq!(code, 7);
}

#[test]
fn match_int() {
    // `match 0 with | 0 => 42 | _ => 0` — first arm matches, exit 42.
    let (out, code) = run_smoke("match-int");
    assert!(out.is_empty());
    assert_eq!(code, 42);
}

#[test]
fn echo() {
    let path = examples_root().join("echo.tac");
    let built = build(&path, "echo");
    let (out, code) = run_with_stdin(&built, b"hi\n");
    assert_eq!(code, 0);
    assert_eq!(out, b"hi\n");
}
