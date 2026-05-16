use tacit_typecheck::infer_module;
use tacit_typecheck::ty::{FixedIntTy, IntSign, Ty};
use tacit_views::authoring::parse_authoring;

fn parse(src: &str) -> tacit_canonical::ast::Node {
    parse_authoring(src.as_bytes())
        .unwrap_or_else(|e| panic!("parse failed: {e}"))
        .0
}

#[test]
fn fixed_width_wrap_arithmetic_typechecks() {
    let ast = parse("let x: u8 = 255 in @u8-add-wrap x 1");
    let typed = infer_module(&ast).expect("typecheck");
    assert_eq!(
        typed.ty,
        Ty::FixedInt(FixedIntTy::new(IntSign::Unsigned, 8))
    );
    assert!(typed.effects.is_pure());
}

#[test]
fn checked_arithmetic_result_is_explicit_record() {
    let ast = parse("let r = @u8-add-check 255 1 in if r.ok then r.value else 7");
    let typed = infer_module(&ast).expect("typecheck");
    assert_eq!(
        typed.ty,
        Ty::FixedInt(FixedIntTy::new(IntSign::Unsigned, 8))
    );
}

#[test]
fn fixed_literal_must_fit_annotation() {
    let ast = parse("let x: u8 = 300 in x");
    let diags = infer_module(&ast).expect_err("expected literal range error");
    assert!(diags
        .iter()
        .any(|diag| diag.kind == "integer-literal-out-of-range"));
}

#[test]
fn fixed_primitive_literal_args_must_fit() {
    let ast = parse("@u8-add-wrap 300 1");
    let diags = infer_module(&ast).expect_err("expected literal range error");
    assert!(diags
        .iter()
        .any(|diag| diag.kind == "integer-literal-out-of-range"));
}

#[test]
fn static_shift_count_must_fit_width() {
    let ast = parse("@u8-shl 1 8");
    let diags = infer_module(&ast).expect_err("expected shift-width error");
    assert!(diags.iter().any(|diag| diag.kind == "invalid-shift-width"));
}

#[test]
fn legacy_arithmetic_does_not_silently_operate_on_fixed_ints() {
    let ast = parse("let x: u8 = 1 in @add x x");
    let diags = infer_module(&ast).expect_err("expected overload error");
    assert!(diags
        .iter()
        .any(|diag| diag.kind == "operator-overload-failure"));
}
