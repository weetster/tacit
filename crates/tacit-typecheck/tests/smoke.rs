//! Smoke corpus integration tests: every Phase 1 smoke program typechecks
//! and matches its .tacd sidecar annotation.

use std::path::PathBuf;

use tacit_canonical::parse as parse_canonical;
use tacit_typecheck::sidecar::check_against_tacd;
use tacit_views::sidecar::Sidecar;

fn smoke_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/smoke")
}

fn check_smoke(name: &str) {
    let smoke = smoke_dir();
    let tac_path = smoke.join(format!("{}.tac", name));
    let tacd_path = smoke.join(format!("{}.tacd", name));

    let src =
        std::fs::read(&tac_path).unwrap_or_else(|e| panic!("could not read {}.tac: {}", name, e));

    let ast = parse_canonical(&src)
        .unwrap_or_else(|e| panic!("parse error in {}.tac: {:?}", name, e));

    let sidecar = Sidecar::read(&tacd_path)
        .unwrap_or_else(|e| panic!("sidecar load error for {}: {}", name, e));

    check_against_tacd(&ast, &sidecar).unwrap_or_else(|diags| {
        let msgs: Vec<String> = diags
            .iter()
            .map(|d| format!("{}: {}", d.kind, d.message))
            .collect();
        panic!("{}.tac typecheck failed:\n{}", name, msgs.join("\n"));
    });
}

#[test]
fn smoke_return_zero() {
    check_smoke("return-zero");
}

#[test]
fn smoke_return_computed() {
    check_smoke("return-computed");
}

#[test]
fn smoke_hello() {
    check_smoke("hello");
}

#[test]
fn smoke_if_branch() {
    check_smoke("if-branch");
}

#[test]
fn smoke_factorial() {
    check_smoke("factorial");
}

#[test]
fn smoke_even_odd() {
    check_smoke("even-odd");
}

#[test]
fn smoke_exit_nonzero() {
    check_smoke("exit-nonzero");
}

#[test]
fn smoke_match_int() {
    check_smoke("match-int");
}

#[test]
fn smoke_echo() {
    check_smoke("echo");
}
