//! Phase 6 Stage 8 data-layout and decode executable fixtures (ADR 0086).

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
        .join("phase-6")
        .join("data-layout")
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

fn run_example(stem: &str) -> (Vec<u8>, i32) {
    let path = examples_root().join(format!("{}.tac", stem));
    let built = build(&path, stem);
    let out = Command::new(&built.exe).output().expect("run exe");
    (out.stdout, out.status.code().unwrap_or(-1))
}

#[test]
fn cpu_state_record_projects_program_counter_low_byte() {
    let (out, code) = run_example("cpu-state-record");
    assert!(out.is_empty());
    assert_eq!(code, 52);
}

#[test]
fn opcode_decode_record_dispatches_by_high_nibble() {
    let (out, code) = run_example("opcode-decode-record");
    assert!(out.is_empty());
    assert_eq!(code, 18);
}
