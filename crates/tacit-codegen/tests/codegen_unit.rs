//! Unit-level tests on AST → IR text. These run the LLVM emission path
//! and so are gated on the `llvm` feature aggregate (turned on by any of
//! the per-version `llvm<N>-<M>` features). Without the feature, the
//! file compiles to an empty test module and `cargo test` skips it.

#![cfg(feature = "llvm")]

use tacit_canonical::ast::Node;
use tacit_codegen::compile_to_ir_string;
use tacit_codegen::error::CodegenError;
use tacit_codegen::primitives::PrimKind;
use tacit_views::authoring::parse_authoring;

#[test]
fn primitive_lookup_smoke() {
    assert!(PrimKind::lookup("write").is_some());
    assert!(PrimKind::lookup("add").is_some());
    assert!(PrimKind::lookup("lt").is_some());
    assert!(PrimKind::lookup("frobnicate").is_none());
}

#[test]
fn module_node_unsupported() {
    let node = Node::Module {
        bindings: vec![Node::Int { value: "0".into() }],
    };
    let context = inkwell::context::Context::create();
    let mut compiler = tacit_codegen::compile::Compiler::new(&context, "m");
    let err = compiler.compile_program(&node).expect_err("expected error");
    assert!(matches!(err, CodegenError::Unsupported(_)));
}

#[test]
fn unknown_primitive_errors() {
    // (app (sym frobnicate) (int 0))
    let node = Node::App {
        fn_: Box::new(Node::Sym {
            name: "frobnicate".into(),
        }),
        arg: Box::new(Node::Int { value: "0".into() }),
    };
    let context = inkwell::context::Context::create();
    let mut compiler = tacit_codegen::compile::Compiler::new(&context, "m");
    let err = compiler.compile_program(&node).expect_err("expected error");
    assert!(
        matches!(err, CodegenError::UnknownPrimitive { ref name } if name == "frobnicate"),
        "got {:?}",
        err
    );
}

#[test]
fn primitive_arity_mismatch() {
    // (app (sym add) (int 1)) — only 1 arg, expected 2
    let node = Node::App {
        fn_: Box::new(Node::Sym { name: "add".into() }),
        arg: Box::new(Node::Int { value: "1".into() }),
    };
    let context = inkwell::context::Context::create();
    let mut compiler = tacit_codegen::compile::Compiler::new(&context, "m");
    let err = compiler.compile_program(&node).expect_err("expected error");
    assert!(matches!(
        err,
        CodegenError::PrimitiveArity {
            expected: 2,
            got: 1,
            ..
        }
    ));
}

#[test]
fn hole_node_hard_fails() {
    let node = Node::Hole {
        diag_id: "X".into(),
        payload: Box::new(Node::Str { value: "".into() }),
    };
    let context = inkwell::context::Context::create();
    let mut compiler = tacit_codegen::compile::Compiler::new(&context, "m");
    let err = compiler.compile_program(&node).expect_err("expected error");
    assert!(matches!(err, CodegenError::Hole { .. }));
}

#[test]
fn free_var_in_lambda_rejected() {
    // (lam (var 1)) — Var 1 is free relative to a single-arg lambda.
    let node = Node::App {
        fn_: Box::new(Node::Lam {
            body: Box::new(Node::Var { index: 1 }),
        }),
        arg: Box::new(Node::Int { value: "0".into() }),
    };
    let context = inkwell::context::Context::create();
    let mut compiler = tacit_codegen::compile::Compiler::new(&context, "m");
    let err = compiler.compile_program(&node).expect_err("expected error");
    assert!(matches!(err, CodegenError::FreeVarInLambda { index: 1 }));
}

#[test]
fn integer_overflow_rejected() {
    let node = Node::Int {
        value: "999999999999999999999999".into(),
    };
    let context = inkwell::context::Context::create();
    let mut compiler = tacit_codegen::compile::Compiler::new(&context, "m");
    let err = compiler.compile_program(&node).expect_err("expected error");
    assert!(matches!(err, CodegenError::IntegerOverflow { .. }));
}

#[test]
fn closed_multi_arg_let_lowers_as_direct_call() {
    let src = b"let add2 = lambda x. lambda y. @add x y in add2 40 2";
    let (node, _) = parse_authoring(src).expect("parse");
    let ir = compile_to_ir_string(&node, "multi_arg_let").expect("codegen");
    assert!(ir.contains("define private i64 @tacit_fn_0_let(i64"));
    assert!(ir.contains(", i64"));
}

#[test]
fn closed_multi_arg_rec_lowers_as_direct_call() {
    let src = b"rec { gcd = lambda a. lambda b. if b then gcd b (@mod a b) else a } in gcd 12 18";
    let (node, _) = parse_authoring(src).expect("parse");
    let ir = compile_to_ir_string(&node, "multi_arg_rec").expect("codegen");
    assert!(ir.contains("define private i64 @tacit_fn_0_rec(i64"));
    assert!(ir.contains("call i64 @tacit_fn_0_rec"));
}
