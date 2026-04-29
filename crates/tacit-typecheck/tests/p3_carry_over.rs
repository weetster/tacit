//! Phase 3 Stage 3 typecheck tests: verify each carry-over program
//! typechecks against its sidecar annotation.

use std::path::PathBuf;

use tacit_typecheck::sidecar::{check_against_sidecar, TypeSidecar};

fn phase3_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/phase-3")
}

fn check_phase3(name: &str) {
    let dir = phase3_dir();
    let tac_path = dir.join(format!("{}.tac", name));
    let sidecar_path = dir.join(format!("{}.tac.sidecar.toml", name));

    let src = std::fs::read(&tac_path)
        .unwrap_or_else(|e| panic!("could not read {}.tac: {}", name, e));

    let (ast, _sidecar_node) = tacit_views::authoring::parse_authoring(&src)
        .unwrap_or_else(|e| panic!("parse error in {}.tac: {:?}", name, e));

    let type_sidecar = TypeSidecar::load(&sidecar_path)
        .unwrap_or_else(|e| panic!("sidecar load error for {}: {}", name, e));

    check_against_sidecar(&ast, &type_sidecar).unwrap_or_else(|diags| {
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
