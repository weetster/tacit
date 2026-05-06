//! Phase 3 Stage 3 typecheck tests: verify each carry-over program
//! typechecks against its .tacd sidecar annotation.

use std::path::PathBuf;

use tacit_canonical::parse as parse_canonical;
use tacit_typecheck::sidecar::check_against_tacd;
use tacit_views::sidecar::Sidecar;

fn phase3_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/phase-3")
}

fn check_phase3(name: &str) {
    let dir = phase3_dir();
    let tac_path = dir.join(format!("{}.tac", name));
    let tacd_path = dir.join(format!("{}.tacd", name));

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
fn p3_sort_typechecks() {
    check_phase3("sort");
}

#[test]
fn p3_list_typechecks() {
    check_phase3("list");
}

#[test]
fn p3_sum_numbers_typechecks() {
    check_phase3("sum-numbers");
}
