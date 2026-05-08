//! Compile and execute the durable examples under `examples/phase-4/`.

#![cfg(feature = "llvm")]

use std::path::{Path, PathBuf};
use std::process::Command;

use tacit_canonical::parse as parse_canonical;
use tacit_codegen::compile::compile_to_object;

fn examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/phase-4")
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
    let obj = tmp.path().join(format!("{name}.o"));
    compile_to_object(&node, name, &obj).expect("emit object");

    let exe = tmp.path().join(name);
    let linker = pick_linker().expect("no C linker found (cc/clang/gcc); install a C toolchain");
    let status = Command::new(&linker)
        .arg(&obj)
        .arg("-o")
        .arg(&exe)
        .status()
        .expect("invoke linker");
    assert!(status.success(), "linker failed for {name}");

    Built { exe, _tmp: tmp }
}

fn run_example(name: &str) -> (Vec<u8>, i32) {
    let path = examples_dir().join(format!("{name}.tac"));
    let built = build(&path, name);
    let out = Command::new(&built.exe).output().expect("run exe");
    (out.stdout, out.status.code().unwrap_or(-1))
}

#[test]
fn record_accumulator() {
    let (out, code) = run_example("record-accumulator");
    assert!(out.is_empty());
    assert_eq!(code, 9);
}

#[test]
fn closure_pipeline() {
    let (out, code) = run_example("closure-pipeline");
    assert!(out.is_empty());
    assert_eq!(code, 42);
}

#[test]
fn stored_callback_record() {
    let (out, code) = run_example("stored-callback-record");
    assert!(out.is_empty());
    assert_eq!(code, 41);
}

#[test]
fn vector_combinators() {
    let (out, code) = run_example("vector-combinators");
    assert_eq!(out, b"A\nB\n");
    assert_eq!(code, 18);
}
