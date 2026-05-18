//! ADR 0093: bounded-stack `@loop` primitive execution tests.
//!
//! These tests compile small Tacit programs that exercise `@loop`, link
//! them, and run the resulting binary to verify the loop's exit code.
//! The million-iteration test verifies the bounded-stack property: an
//! unbounded-stack implementation would segfault on a default 8 MB stack
//! long before 1M iterations.

#![cfg(feature = "llvm")]

use std::process::Command;

use tacit_codegen::compile::compile_to_object;
use tacit_views::authoring::parse_authoring;

fn pick_linker() -> Option<String> {
    for cand in ["cc", "clang", "gcc"] {
        if Command::new("sh")
            .arg("-c")
            .arg(format!("command -v {cand} >/dev/null 2>&1"))
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return Some(cand.to_string());
        }
    }
    None
}

fn build_and_run(src: &[u8], name: &str) -> (Vec<u8>, i32) {
    let (node, _) = parse_authoring(src).expect("parse");
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
    let out = Command::new(&exe).output().expect("run exe");
    (out.stdout, out.status.code().unwrap_or(-1))
}

#[test]
fn loop_counts_to_one_million_without_stack_overflow() {
    // 1_000_000 iterations.  A naive recursive lowering blows the default
    // 8 MB stack at ~100K frames; if this test completes the back-edge is
    // truly a basic-block branch, not a call.  Unix exit codes are 8-bit,
    // so the assertion compares the low byte of 1_000_000 (= 64).
    let src = b"@loop 0 (lambda s. if @lt s 1000000 then @loop-step (@add s 1) else @loop-exit s)";
    let (out, code) = build_and_run(src, "loop_million");
    assert!(out.is_empty());
    assert_eq!(code, 1_000_000 & 0xff);
}

#[test]
fn loop_record_state_sums_one_through_ten() {
    // State {acc, i}; sum 1..=10 → 55.  Verifies records flow through the
    // state PHI and back-edge correctly.
    let src = b"(@loop {acc: 0, i: 10} (lambda s. if @lt 0 s.i then @loop-step {acc: @add s.acc s.i, i: @sub s.i 1} else @loop-exit s)).acc";
    let (out, code) = build_and_run(src, "loop_record_sum");
    assert!(out.is_empty());
    assert_eq!(code, 55);
}

#[test]
fn loop_exits_immediately_on_first_iteration() {
    // Body unconditionally returns @loop-exit on the first call.  Verifies
    // the exit branch fires before any back-edge.
    let src = b"@loop 99 (lambda s. @loop-exit s)";
    let (out, code) = build_and_run(src, "loop_exit_first");
    assert!(out.is_empty());
    assert_eq!(code, 99);
}
