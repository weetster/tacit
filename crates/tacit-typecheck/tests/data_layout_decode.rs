//! ADR 0086 Stage 8 data-layout and decode fixtures.

use std::path::PathBuf;

use tacit_canonical::parse as parse_canonical;
use tacit_typecheck::ty::{FixedIntTy, IntSign};
use tacit_typecheck::{infer_module, Ty};
use tacit_views::authoring::parse_authoring;

fn examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("phase-6")
        .join("data-layout")
}

fn infer_example(name: &str) -> tacit_typecheck::TypedModule {
    let path = examples_dir().join(name);
    let src = std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let ast = parse_canonical(&src).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));
    infer_module(&ast).unwrap_or_else(|diags| {
        let msgs: Vec<_> = diags
            .iter()
            .map(|d| format!("{}: {}", d.kind, d.message))
            .collect();
        panic!(
            "typecheck failed for {}:\n{}",
            path.display(),
            msgs.join("\n")
        );
    })
}

fn infer_authoring(src: &str) -> tacit_typecheck::TypedModule {
    let (ast, _) =
        parse_authoring(src.as_bytes()).unwrap_or_else(|e| panic!("parse failed for {src:?}: {e}"));
    infer_module(&ast).unwrap_or_else(|diags| {
        let msgs: Vec<_> = diags
            .iter()
            .map(|d| format!("{}: {}", d.kind, d.message))
            .collect();
        panic!("typecheck failed for {src:?}:\n{}", msgs.join("\n"));
    })
}

#[test]
fn cpu_state_record_example_typechecks() {
    let typed = infer_example("cpu-state-record.tac");
    assert_eq!(
        typed.ty,
        Ty::FixedInt(FixedIntTy::new(IntSign::Unsigned, 8))
    );
    assert!(typed.effects.is_pure());
}

#[test]
fn opcode_decode_record_example_typechecks() {
    let typed = infer_example("opcode-decode-record.tac");
    assert_eq!(
        typed.ty,
        Ty::FixedInt(FixedIntTy::new(IntSign::Unsigned, 8))
    );
    assert!(typed.effects.is_pure());
}

#[test]
fn decode_records_use_explicit_tag_fields() {
    let typed = infer_authoring(
        "let opcode: u8 = 33 in
         let operand = @u8-and opcode 15 in
         let tag = @u8-shr opcode 4 in
         match tag with
         | 2 => {kind: (32:@u8), mode: (1:@u8), operand: operand}
         | _ => {kind: (255:@u8), mode: (0:@u8), operand: operand}",
    );
    assert_eq!(
        typed.ty,
        Ty::Record(
            [
                (
                    "kind".to_string(),
                    Ty::FixedInt(FixedIntTy::new(IntSign::Unsigned, 8)),
                ),
                (
                    "mode".to_string(),
                    Ty::FixedInt(FixedIntTy::new(IntSign::Unsigned, 8)),
                ),
                (
                    "operand".to_string(),
                    Ty::FixedInt(FixedIntTy::new(IntSign::Unsigned, 8)),
                ),
            ]
            .into()
        )
    );
}
