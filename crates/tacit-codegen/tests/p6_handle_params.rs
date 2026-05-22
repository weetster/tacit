//! ADR 0098 regression: typed vector handles as down-only call-local
//! parameters. Exercises a flat program that factors a `u8vec` memory bus
//! into helpers whose explicit signatures take a handle parameter — the
//! direct-call lowering path that previously failed codegen with "typed
//! vector handle used in integer-value position".

#![cfg(feature = "llvm")]

use std::path::{Path, PathBuf};
use std::process::Command;

use tacit_canonical::{emit, parse as parse_canonical};
use tacit_codegen::compile::compile_to_object;

fn example_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("phase-6")
        .join("typed-memory")
        .join("memory-bus-helper.tac")
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

fn build_and_run(program_path: &Path, name: &str) -> (Vec<u8>, i32) {
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
    let out = Command::new(&exe).output().expect("run exe");
    (out.stdout, out.status.code().unwrap_or(-1))
}

#[test]
fn memory_bus_helper_round_trips_through_handle_parameters() {
    // A store helper writes 0xCAFEBABE little-endian at offset 0; a read
    // helper reads offset 1. Both helpers take the `u8vec` bus as an explicit
    // handle parameter — the inlined direct-call form. The little-endian byte
    // at offset 1 is 0xBA = 186.
    let (out, code) = build_and_run(&example_path(), "memory-bus-helper");
    assert!(out.is_empty());
    assert_eq!(code, 186);
}

#[test]
fn memory_bus_helper_example_is_canonical() {
    // The checked-in `.tac` example must be byte-exact canonical text.
    let src = std::fs::read(example_path()).expect("read program");
    let node = parse_canonical(&src).expect("parse canonical");
    assert_eq!(emit(&node), src);
}
