//! Phase 3 Stage 2 exit-gate tests (ADR 0047).
//!
//! Exercises each new Phase 3 primitive end-to-end — authoring view → compile
//! → link → run.  One positive and one boundary case per primitive.
//!
//! Gated on the `llvm` feature aggregate (turned on by any of the per-version
//! `llvm<N>-<M>` features). Without the feature this module is empty.

#![cfg(feature = "llvm")]

use std::path::{Path, PathBuf};
use std::process::Command;

use tacit_codegen::compile::compile_to_object;
use tacit_views::authoring::parse_authoring;

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

fn run_p3(stem: &str) -> (Vec<u8>, i32) {
    let path = examples_root().join(format!("{}.tac", stem));
    run(&build(&path, stem))
}

// ── @buf-alloc-dyn ────────────────────────────────────────────────────────────

#[test]
fn buf_alloc_dyn_positive() {
    // alloc 3 bytes at runtime, set buf[0]=77, get buf[0] → exit 77
    let (out, code) = run_p3("p3-buf-alloc-dyn");
    assert!(out.is_empty());
    assert_eq!(code, 77);
}

#[test]
fn buf_alloc_dyn_min_size() {
    // alloc exactly 1 byte (boundary: minimum useful size)
    let (out, code) = run_p3("p3-buf-alloc-dyn-min");
    assert!(out.is_empty());
    assert_eq!(code, 99);
}

// ── @buf-get / @buf-set ───────────────────────────────────────────────────────

#[test]
fn buf_get_set_nonzero_offset() {
    // set buf[2]=88, get buf[2] → exit 88 (positive: non-zero offset)
    let (out, code) = run_p3("p3-buf-get-set");
    assert!(out.is_empty());
    assert_eq!(code, 88);
}

#[test]
fn buf_get_set_zero_offset() {
    // set buf[0]=55, get buf[0] → exit 55 (boundary: offset 0)
    let (out, code) = run_p3("p3-buf-get-set-zero");
    assert!(out.is_empty());
    assert_eq!(code, 55);
}

// ── @buf-copy ────────────────────────────────────────────────────────────────

#[test]
fn buf_copy_positive() {
    // copy "Hi\n" from src to dst, write to stdout
    let (out, code) = run_p3("p3-buf-copy");
    assert_eq!(out, b"Hi\n");
    assert_eq!(code, 0);
}

#[test]
fn buf_copy_zero_len() {
    // copy 0 bytes → dst unchanged; get dst[0] which was set to 90 ('Z')
    let (out, code) = run_p3("p3-buf-copy-zero");
    assert!(out.is_empty());
    assert_eq!(code, 90);
}

// ── @buf-eq ───────────────────────────────────────────────────────────────────

#[test]
fn buf_eq_equal() {
    // two identical 2-byte regions → 1
    let (out, code) = run_p3("p3-buf-eq-equal");
    assert!(out.is_empty());
    assert_eq!(code, 1);
}

#[test]
fn buf_eq_not_equal() {
    // two regions differing at index 1 → 0
    let (out, code) = run_p3("p3-buf-eq-neq");
    assert!(out.is_empty());
    assert_eq!(code, 0);
}

// ── @scan-byte ────────────────────────────────────────────────────────────────

#[test]
fn scan_byte_found() {
    // "hello\n" in buf, scan for '\n' (10) → exit 5
    let (out, code) = run_p3("p3-scan-byte-found");
    assert!(out.is_empty());
    assert_eq!(code, 5);
}

#[test]
fn scan_byte_not_found() {
    // "hello" in buf (no '\n'), scan for 10 → exit off+len = 0+5 = 5
    let (out, code) = run_p3("p3-scan-byte-notfound");
    assert!(out.is_empty());
    assert_eq!(code, 5);
}

// ── @parse-i64 ────────────────────────────────────────────────────────────────

#[test]
fn parse_i64_positive() {
    // parse "42" from buf → exit 42
    let (out, code) = run_p3("p3-parse-i64");
    assert!(out.is_empty());
    assert_eq!(code, 42);
}

#[test]
fn parse_i64_empty_range() {
    // parse with len=0 → exit 0 (boundary: empty range)
    let (out, code) = run_p3("p3-parse-i64-empty");
    assert!(out.is_empty());
    assert_eq!(code, 0);
}

// ── @fmt-i64 ────────────────────────────────────────────────────────────────

#[test]
fn fmt_i64_positive() {
    // format 42 → stdout "42"
    let (out, code) = run_p3("p3-fmt-i64");
    assert_eq!(out, b"42");
    assert_eq!(code, 0);
}

#[test]
fn fmt_i64_zero() {
    // format 0 → stdout "0" (boundary: special zero case)
    let (out, code) = run_p3("p3-fmt-i64-zero");
    assert_eq!(out, b"0");
    assert_eq!(code, 0);
}
