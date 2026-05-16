//! Phase 6 Stage 7 typed mutable memory exit-gate tests (ADR 0085).
//!
//! Exercises the new `<ty>vec` primitive surface end-to-end:
//! authoring view → compile → link → run.

#![cfg(feature = "llvm")]

use std::path::{Path, PathBuf};
use std::process::Command;

use tacit_canonical::parse as parse_canonical;
use tacit_codegen::compile::compile_to_object;

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
    (out.stdout, out.status.code().unwrap_or(-1))
}

fn run_p6(stem: &str) -> (Vec<u8>, i32) {
    let path = examples_root().join(format!("{}.tac", stem));
    run(&build(&path, stem))
}

// ── @u8vec round-trip ────────────────────────────────────────────────────────

#[test]
fn u8vec_round_trip_returns_stored_byte() {
    // store 42 at index 7, read back, zero-extend → exit code 42
    let (out, code) = run_p6("p6-u8vec-round-trip");
    assert!(out.is_empty());
    assert_eq!(code, 42);
}

// ── @u32vec round-trip ───────────────────────────────────────────────────────

#[test]
fn u32vec_round_trip_returns_stored_word() {
    // store 250 at index 3, read back, truncate to u8 → exit code 250
    let (out, code) = run_p6("p6-u32vec-round-trip");
    assert!(out.is_empty());
    assert_eq!(code, 250);
}

// ── @u8vec byte-bus typed load/store ─────────────────────────────────────────

#[test]
fn u8vec_bus_u32_le_round_trip() {
    // store 0xCAFE_BABE at offset 4, load u32-le, truncate to u8 → 0xBE = 190
    let (out, code) = run_p6("p6-u8vec-bus-u32-le");
    assert!(out.is_empty());
    assert_eq!(code, 190);
}

// ── @u8vec slice ─────────────────────────────────────────────────────────────

#[test]
fn u8vec_slice_observes_parent_writes() {
    // ram[3] = 99, slice s = ram[2..6], s[1] == ram[3] == 99
    let (out, code) = run_p6("p6-u8vec-slice");
    assert!(out.is_empty());
    assert_eq!(code, 99);
}
