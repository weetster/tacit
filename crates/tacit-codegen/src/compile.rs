//! AST → LLVM IR via `inkwell`. Built only when an `llvm<N>-<M>` feature
//! is enabled; without it, this module is excluded from the crate.
//!
//! ## Lowering model (Phase 1)
//!
//! - The whole input program is the body of an implicit `int main(void)`.
//! - The program's value, computed as `i64`, becomes `main`'s exit code
//!   (truncated to `i32` per C runtime convention; ADR 0025 § Phase 1
//!   libc set notes `return 0` is preferred over `exit(0)`).
//! - Every `Lam` is closed (ADR 0026), unary, and hoisted as a top-level
//!   LLVM function `(i64) -> i64` under default C calling convention
//!   (ADR 0027). Multi-arg functions are not supported; the Phase 1
//!   smoke corpus is unary-only.
//! - `App(Lam_or_RecMember, arg)` lowers as a direct call. Other `App`
//!   shapes (e.g., applying a `Var` resolving to a non-`Lam` binding)
//!   fail with `CodegenError::AppNonFunction`.
//! - `App` whose left-spine head is `Sym(name)` with `name` in the
//!   Phase 1 allowlist (LIBC ∪ ARITH ∪ CMP per ADR 0028 + ADR 0030)
//!   lowers as a direct primitive emission. `Sym` heads outside the
//!   allowlist fail with `UnknownPrimitive`.
//! - `Rec { bindings, body }` forward-declares every binding member,
//!   then defines each body, then lowers `body` (ADR 0027).
//! - `If`'s condition is an `i64`; non-zero takes the `then` branch
//!   (ADR 0030 § `if` truthy semantics).
//! - `Match` supports integer-literal arms (`pat-ctor` of `Int`-named
//!   ctors is *not* the surface; see § Match below) plus a single
//!   trailing wildcard. Phase 1 keeps `match` mechanical for smoke #7.
//!
//! ## Match in Phase 1
//!
//! Canonical-text-format § 2 lists `pat-wild`, `pat-var`, `pat-ctor`
//! as the only pattern kinds. There is no `pat-int`. Phase 1 smoke
//! corpus #7 (`match-int.tac`) names integer-arm patterns via
//! `(pat-ctor "<decimal>")` — the ctor name is a decimal-integer
//! string. The codegen interprets these as integer literals; this
//! is the smallest interpretation consistent with the frozen
//! canonical surface and ADR 0028's reservation of `sym` for
//! curated namespaces. A non-numeric `pat-ctor` name fails with
//! `UnsupportedMatchPattern`. (When Phase 2 introduces user-defined
//! ADTs, this special-case retreats; in Phase 1 every `pat-ctor`
//! name is interpreted as an integer literal arm, which mirrors how
//! `Sym` at App head is interpreted as a primitive name.)

use std::path::Path;

use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::{Linkage, Module};
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine,
};
use inkwell::types::BasicMetadataTypeEnum;
use inkwell::values::{BasicMetadataValueEnum, FunctionValue, IntValue, PointerValue};
use inkwell::{AddressSpace, IntPredicate, OptimizationLevel};

use tacit_canonical::ast::Node;

use crate::analysis::{check_closed, check_no_holes, parse_int_literal, sanitize, unfold_app};
use crate::error::CodegenError;
use crate::primitives::{ArithOp, CmpOp, PrimKind};

type Result<T> = std::result::Result<T, CodegenError>;

/// Top-level entry: build an `inkwell::Module` from a Tacit AST and emit it
/// to an object file at `out_path`. Returns the path the object was written to.
pub fn compile_to_object(node: &Node, module_name: &str, out_path: &Path) -> Result<()> {
    let context = Context::create();
    let mut compiler = Compiler::new(&context, module_name);
    compiler.compile_program(node)?;
    compiler.write_object(out_path)
}

/// Holds the inkwell-side state for one compilation unit.
pub struct Compiler<'ctx> {
    context: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    /// Counter for deterministic synthetic function names (ADR 0026 § 2,
    /// ADR 0027 § 4). Function names are `tacit_fn_<n>` in pre-order
    /// hoisting order; collisions across compilation units are not a
    /// Phase 1 concern (no link-step deduplication).
    next_fn_id: usize,
}

/// Per-binder entry on the binding stack. Innermost binder is `last`.
#[derive(Clone)]
enum Binding<'ctx> {
    /// A computed `i64` value bound by `let` or `lam` parameter.
    Value(IntValue<'ctx>),
    /// A top-level function reference. Only callable at `App` head;
    /// reading the binding outside head position is `FirstClassFunction`.
    Function(FunctionValue<'ctx>),
}

impl<'ctx> Compiler<'ctx> {
    pub fn new(context: &'ctx Context, module_name: &str) -> Self {
        let module = context.create_module(module_name);
        let builder = context.create_builder();
        Compiler {
            context,
            module,
            builder,
            next_fn_id: 0,
        }
    }

    fn fresh_fn_name(&mut self, hint: &str) -> String {
        let id = self.next_fn_id;
        self.next_fn_id += 1;
        // Deterministic and collision-free per ADR 0026 § 2 / ADR 0027 § 4.
        // Keep a hint suffix for legibility in `--emit-llvm-ir` output;
        // hint is only advisory, identity is the numeric prefix.
        format!("tacit_fn_{}_{}", id, sanitize(hint))
    }

    /// Compile the whole program as `int main(void)`.
    pub fn compile_program(&mut self, node: &Node) -> Result<()> {
        // Reject Hole nodes up front (ADR 0023; Phase 1 hard-fails).
        check_no_holes(node)?;

        let i32_t = self.context.i32_type();
        let main_ty = i32_t.fn_type(&[], false);
        let main_fn = self
            .module
            .add_function("main", main_ty, Some(Linkage::External));
        main_fn.set_call_conventions(0); // C calling convention (ADR 0027)

        let entry = self.context.append_basic_block(main_fn, "entry");
        self.builder.position_at_end(entry);

        // The program's value is an i64; main returns its low 32 bits.
        let env: Vec<Binding<'ctx>> = Vec::new();
        let value = self.compile_expr(node, &env, main_fn)?;
        let i32_val = self.builder.build_int_truncate(value, i32_t, "main_ret");
        let i32_val = i32_val.map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_return(Some(&i32_val))
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // Verify the module before handing it on; surface verification errors
        // as `CodegenError::Llvm` so callers don't get an LLVM panic.
        self.module
            .verify()
            .map_err(|e| CodegenError::Llvm(e.to_string_lossy().into_owned()))?;
        Ok(())
    }

    /// Print the constructed module to a `.ll` file (for `--emit-llvm-ir`).
    pub fn print_to_string(&self) -> String {
        self.module.print_to_string().to_string()
    }

    /// Emit the module to an object file at `out_path`.
    pub fn write_object(&self, out_path: &Path) -> Result<()> {
        Target::initialize_native(&InitializationConfig::default())
            .map_err(|e| CodegenError::Llvm(format!("native target init: {e}")))?;

        let triple = TargetMachine::get_default_triple();
        let target = Target::from_triple(&triple).map_err(|e| CodegenError::Llvm(e.to_string()))?;

        let cpu = TargetMachine::get_host_cpu_name().to_string();
        let features = TargetMachine::get_host_cpu_features().to_string();

        let target_machine = target
            .create_target_machine(
                &triple,
                &cpu,
                &features,
                OptimizationLevel::None,
                RelocMode::PIC,
                CodeModel::Default,
            )
            .ok_or_else(|| CodegenError::Llvm("failed to create target machine".into()))?;

        self.module.set_triple(&triple);
        self.module
            .set_data_layout(&target_machine.get_target_data().get_data_layout());

        target_machine
            .write_to_file(&self.module, FileType::Object, out_path)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        Ok(())
    }

    /// Compile an expression to an `i64` SSA value, given a binder stack.
    fn compile_expr(
        &mut self,
        node: &Node,
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        match node {
            Node::Int { value } => self.compile_int_literal(value),
            Node::Str { .. } => Err(CodegenError::Unsupported(
                "string literal outside of @write/@read primitive call",
            )),
            Node::Var { index } => {
                let entry = lookup_var(env, *index)?;
                match entry {
                    Binding::Value(v) => Ok(*v),
                    Binding::Function(_) => Err(CodegenError::FirstClassFunction),
                }
            }
            Node::Let { rhs, body } => self.compile_let(rhs, body, env, cur_fn),
            Node::If { cond, then, else_ } => self.compile_if(cond, then, else_, env, cur_fn),
            Node::App { .. } => self.compile_app(node, env, cur_fn),
            Node::Lam { .. } => Err(CodegenError::FirstClassFunction),
            Node::Rec { bindings, body } => self.compile_rec(bindings, body, env, cur_fn),
            Node::Module { .. } => Err(CodegenError::Unsupported("top-level module")),
            Node::Match { scrutinee, arms } => self.compile_match(scrutinee, arms, env, cur_fn),
            Node::Arm { .. } => Err(CodegenError::Unsupported("bare arm outside match")),
            Node::Record { .. } => Err(CodegenError::Unsupported("record")),
            Node::Proj { .. } => Err(CodegenError::Unsupported("proj")),
            Node::Ctor { .. } => Err(CodegenError::Unsupported("ctor in expression position")),
            Node::Ann { expr, .. } => self.compile_expr(expr, env, cur_fn),
            Node::Sym { .. } => Err(CodegenError::Unsupported(
                "bare sym outside primitive-call head",
            )),
            Node::Hole { diag_id, .. } => Err(CodegenError::Hole {
                diag_id: diag_id.clone(),
            }),
            Node::PatWild | Node::PatVar | Node::PatCtor { .. } => {
                Err(CodegenError::Unsupported("pattern outside match arm"))
            }
        }
    }

    fn compile_int_literal(&self, decimal: &str) -> Result<IntValue<'ctx>> {
        let parsed = parse_int_literal(decimal)?;
        let i64_t = self.context.i64_type();
        // sign_extend = true so negatives lower as themselves.
        Ok(i64_t.const_int(parsed as u64, true))
    }

    fn compile_let(
        &mut self,
        rhs: &Node,
        body: &Node,
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        // Special-case Let-of-Lam: hoist the lambda to a top-level function
        // and bind the body under a `Function` entry. ADR 0026 § 3 second bullet.
        if let Node::Lam { body: lam_body } = rhs {
            let fn_val = self.hoist_lambda(lam_body, "let")?;
            let mut new_env = env.to_vec();
            new_env.insert(0, Binding::Function(fn_val));
            return self.compile_expr(body, &new_env, cur_fn);
        }

        let v = self.compile_expr(rhs, env, cur_fn)?;
        let mut new_env = env.to_vec();
        new_env.insert(0, Binding::Value(v));
        self.compile_expr(body, &new_env, cur_fn)
    }

    fn compile_if(
        &mut self,
        cond: &Node,
        then_node: &Node,
        else_node: &Node,
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        // ADR 0030 § if truthy semantics: branch on `icmp ne cond, 0`.
        let cond_val = self.compile_expr(cond, env, cur_fn)?;
        let zero = self.context.i64_type().const_zero();
        let cond_i1 = self
            .builder
            .build_int_compare(IntPredicate::NE, cond_val, zero, "if_cond")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        let then_bb = self.context.append_basic_block(cur_fn, "then");
        let else_bb = self.context.append_basic_block(cur_fn, "else");
        let merge_bb = self.context.append_basic_block(cur_fn, "ifcont");

        self.builder
            .build_conditional_branch(cond_i1, then_bb, else_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(then_bb);
        let then_val = self.compile_expr(then_node, env, cur_fn)?;
        let then_end_bb = self.builder.get_insert_block().unwrap();
        self.builder
            .build_unconditional_branch(merge_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(else_bb);
        let else_val = self.compile_expr(else_node, env, cur_fn)?;
        let else_end_bb = self.builder.get_insert_block().unwrap();
        self.builder
            .build_unconditional_branch(merge_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(merge_bb);
        let phi = self
            .builder
            .build_phi(self.context.i64_type(), "ifval")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        phi.add_incoming(&[(&then_val, then_end_bb), (&else_val, else_end_bb)]);
        Ok(phi.as_basic_value().into_int_value())
    }

    /// Compile `App` left-spine. Recognises:
    ///   1. `Sym(name)` head → primitive (libc / arith / cmp).
    ///   2. `Lam(_)` head     → hoist + direct call.
    ///   3. `Var(i)` head whose binding is `Function(_)` → direct call.
    ///
    /// Anything else fails.
    fn compile_app(
        &mut self,
        node: &Node,
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let (head, args) = unfold_app(node);

        match head {
            Node::Sym { name } => self.compile_primitive_call(name, &args, env, cur_fn),
            Node::Lam { body: lam_body } => {
                if args.len() != 1 {
                    return Err(CodegenError::Unsupported(
                        "multi-argument application of bare lambda (Phase 1 unary only)",
                    ));
                }
                let fn_val = self.hoist_lambda(lam_body, "anon")?;
                let arg_val = self.compile_expr(args[0], env, cur_fn)?;
                self.call_function(fn_val, arg_val)
            }
            Node::Var { index } => {
                let binding = lookup_var(env, *index)?;
                match binding {
                    Binding::Function(fn_val) => {
                        if args.len() != 1 {
                            return Err(CodegenError::Unsupported(
                                "multi-argument application (Phase 1 unary only)",
                            ));
                        }
                        let arg_val = self.compile_expr(args[0], env, cur_fn)?;
                        self.call_function(*fn_val, arg_val)
                    }
                    Binding::Value(_) => Err(CodegenError::AppNonFunction),
                }
            }
            _ => Err(CodegenError::AppNonFunction),
        }
    }

    fn call_function(
        &mut self,
        fn_val: FunctionValue<'ctx>,
        arg: IntValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let call = self
            .builder
            .build_call(fn_val, &[BasicMetadataValueEnum::IntValue(arg)], "call")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        // C calling convention is the LLVM default (ADR 0027 § 2); no override needed.
        let ret = call
            .try_as_basic_value()
            .basic()
            .ok_or_else(|| CodegenError::Llvm("call returned no value".into()))?;
        Ok(ret.into_int_value())
    }

    fn compile_primitive_call(
        &mut self,
        name: &str,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let kind = PrimKind::lookup(name)
            .ok_or_else(|| CodegenError::UnknownPrimitive { name: name.into() })?;

        if args.len() != kind.arity() {
            return Err(CodegenError::PrimitiveArity {
                name: name.into(),
                expected: kind.arity(),
                got: args.len(),
            });
        }

        match kind {
            PrimKind::Arith(op) => {
                let a = self.compile_expr(args[0], env, cur_fn)?;
                let b = self.compile_expr(args[1], env, cur_fn)?;
                self.emit_arith(op, a, b, name)
            }
            PrimKind::Cmp(op) => {
                let a = self.compile_expr(args[0], env, cur_fn)?;
                let b = self.compile_expr(args[1], env, cur_fn)?;
                self.emit_cmp(op, a, b)
            }
            PrimKind::Write => self.emit_write(args, env, cur_fn),
            PrimKind::Read => self.emit_read(args, env, cur_fn),
            PrimKind::Exit => self.emit_exit(args, env, cur_fn),
        }
    }

    fn emit_arith(
        &mut self,
        op: ArithOp,
        a: IntValue<'ctx>,
        b: IntValue<'ctx>,
        _name: &str,
    ) -> Result<IntValue<'ctx>> {
        let v = match op {
            ArithOp::Add => self.builder.build_int_nsw_add(a, b, "add"),
            ArithOp::Sub => self.builder.build_int_nsw_sub(a, b, "sub"),
            ArithOp::Mul => self.builder.build_int_nsw_mul(a, b, "mul"),
            ArithOp::Div => self.builder.build_int_signed_div(a, b, "div"),
            ArithOp::Mod => self.builder.build_int_signed_rem(a, b, "mod"),
        };
        v.map_err(|e| CodegenError::Llvm(e.to_string()))
    }

    fn emit_cmp(
        &mut self,
        op: CmpOp,
        a: IntValue<'ctx>,
        b: IntValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let pred = match op {
            CmpOp::Eq => IntPredicate::EQ,
            CmpOp::Ne => IntPredicate::NE,
            CmpOp::Lt => IntPredicate::SLT,
            CmpOp::Le => IntPredicate::SLE,
            CmpOp::Gt => IntPredicate::SGT,
            CmpOp::Ge => IntPredicate::SGE,
        };
        let cmp = self
            .builder
            .build_int_compare(pred, a, b, "cmp")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let i64_t = self.context.i64_type();
        let zext = self
            .builder
            .build_int_z_extend(cmp, i64_t, "cmp_zext")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        Ok(zext)
    }

    fn libc_write(&self) -> FunctionValue<'ctx> {
        if let Some(f) = self.module.get_function("write") {
            return f;
        }
        let i32_t = self.context.i32_type();
        let i64_t = self.context.i64_type();
        let i8ptr = self.context.ptr_type(AddressSpace::default());
        let ty = i64_t.fn_type(
            &[
                BasicMetadataTypeEnum::IntType(i32_t),
                BasicMetadataTypeEnum::PointerType(i8ptr),
                BasicMetadataTypeEnum::IntType(i64_t),
            ],
            false,
        );
        self.module
            .add_function("write", ty, Some(Linkage::External))
    }

    fn libc_read(&self) -> FunctionValue<'ctx> {
        if let Some(f) = self.module.get_function("read") {
            return f;
        }
        let i32_t = self.context.i32_type();
        let i64_t = self.context.i64_type();
        let i8ptr = self.context.ptr_type(AddressSpace::default());
        let ty = i64_t.fn_type(
            &[
                BasicMetadataTypeEnum::IntType(i32_t),
                BasicMetadataTypeEnum::PointerType(i8ptr),
                BasicMetadataTypeEnum::IntType(i64_t),
            ],
            false,
        );
        self.module
            .add_function("read", ty, Some(Linkage::External))
    }

    fn libc_exit(&self) -> FunctionValue<'ctx> {
        if let Some(f) = self.module.get_function("exit") {
            return f;
        }
        let i32_t = self.context.i32_type();
        let void_t = self.context.void_type();
        let ty = void_t.fn_type(&[BasicMetadataTypeEnum::IntType(i32_t)], false);
        let f = self
            .module
            .add_function("exit", ty, Some(Linkage::External));
        // exit is noreturn; mark accordingly so LLVM doesn't expect a fallthrough.
        // (The codegen emits an `unreachable` immediately after the call too.)
        f
    }

    /// `@write fd buf len` lowers to `call i64 @write(i32 fd, i8* buf, i64 len)`.
    /// The first arg must be an i64 (truncated to i32). The middle arg is a
    /// string literal in the smoke corpus; it lowers to a private global.
    fn emit_write(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let fd = self.compile_expr(args[0], env, cur_fn)?;
        let buf = self.compile_buffer_arg(args[1], "write_buf", true, env, cur_fn)?;
        let len = self.compile_expr(args[2], env, cur_fn)?;
        let fd_i32 = self
            .builder
            .build_int_truncate(fd, self.context.i32_type(), "fd_i32")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let f = self.libc_write();
        let call = self
            .builder
            .build_call(
                f,
                &[
                    BasicMetadataValueEnum::IntValue(fd_i32),
                    BasicMetadataValueEnum::PointerValue(buf),
                    BasicMetadataValueEnum::IntValue(len),
                ],
                "wr",
            )
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        Ok(call
            .try_as_basic_value()
            .basic()
            .ok_or_else(|| CodegenError::Llvm("write returned no value".into()))?
            .into_int_value())
    }

    fn emit_read(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let fd = self.compile_expr(args[0], env, cur_fn)?;
        let buf = self.compile_buffer_arg(args[1], "read_buf", false, env, cur_fn)?;
        let len = self.compile_expr(args[2], env, cur_fn)?;
        let fd_i32 = self
            .builder
            .build_int_truncate(fd, self.context.i32_type(), "fd_i32")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let f = self.libc_read();
        let call = self
            .builder
            .build_call(
                f,
                &[
                    BasicMetadataValueEnum::IntValue(fd_i32),
                    BasicMetadataValueEnum::PointerValue(buf),
                    BasicMetadataValueEnum::IntValue(len),
                ],
                "rd",
            )
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        Ok(call
            .try_as_basic_value()
            .basic()
            .ok_or_else(|| CodegenError::Llvm("read returned no value".into()))?
            .into_int_value())
    }

    fn emit_exit(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let code = self.compile_expr(args[0], env, cur_fn)?;
        let code_i32 = self
            .builder
            .build_int_truncate(code, self.context.i32_type(), "exit_code")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let f = self.libc_exit();
        self.builder
            .build_call(f, &[BasicMetadataValueEnum::IntValue(code_i32)], "do_exit")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        // exit() is noreturn; emit `unreachable` and start a dead block so
        // the surrounding code can still build a valid CFG (the value here
        // is never observed; we return an i64 0 placeholder via a phi-free path).
        self.builder
            .build_unreachable()
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let dead = self.context.append_basic_block(cur_fn, "after_exit");
        self.builder.position_at_end(dead);
        Ok(self.context.i64_type().const_zero())
    }

    /// Lower a buffer argument for `@write` / `@read`.
    /// `@write` takes a string-literal middle arg in Phase 1 (smoke #3); this
    /// is lowered to a private `i8` array global, returned as a pointer.
    /// `@read` needs a writable buffer; smoke #8's `echo.tac` allocates one
    /// via a `let` whose rhs is a `(str "...")` of N spaces, which lowers to
    /// the same global path. This is a Phase 1 simplification — `@read` into
    /// a *literal* global is well-defined-but-pointless, but lets the smoke
    /// program build without introducing a stack-buffer construct that would
    /// also belong to a future ADR. The smoke program for echo uses a stack-
    /// allocated array allocated up front via the `BUF` form below.
    fn compile_buffer_arg(
        &mut self,
        node: &Node,
        hint: &str,
        constant: bool,
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<PointerValue<'ctx>> {
        match node {
            Node::Str { value } => {
                let bytes = value.as_bytes();
                let i8_t = self.context.i8_type();
                let arr_ty = i8_t.array_type(bytes.len() as u32);
                let arr_const = self.context.const_string(bytes, false);
                let global = self.module.add_global(arr_ty, None, hint);
                global.set_initializer(&arr_const);
                global.set_constant(constant);
                global.set_linkage(Linkage::Private);
                let zero = self.context.i32_type().const_zero();
                let ptr = unsafe {
                    self.builder.build_in_bounds_gep(
                        arr_ty,
                        global.as_pointer_value(),
                        &[zero, zero],
                        "buf_ptr",
                    )
                };
                ptr.map_err(|e| CodegenError::Llvm(e.to_string()))
            }
            // Allow a `let` whose body is the buffer: no — too complex. For
            // Phase 1, only literal Str args lower as buffers. echo.tac uses
            // a string literal as the read target buffer, which is valid IR
            // (LLVM lets you `read()` into a non-const region by linkage);
            // for Phase 1 correctness we make the global non-constant when
            // the arg is the read buffer slot. Done at the call site above.
            _ => {
                // Fallback: compile the expression as an i64; treat it as a
                // raw pointer. Useful if a future smoke program builds a
                // pointer via primitives. Not exercised in the smoke corpus.
                let v = self.compile_expr(node, env, cur_fn)?;
                let ptr_ty = self.context.ptr_type(AddressSpace::default());
                let ptr = self
                    .builder
                    .build_int_to_ptr(v, ptr_ty, "int_as_ptr")
                    .map_err(|e| CodegenError::Llvm(e.to_string()))?;
                Ok(ptr)
            }
        }
    }

    /// Hoist a `Lam` body to a top-level function `i64 -> i64`.
    /// The lambda must be closed (free DeBruijn check below); body is
    /// compiled with the new env containing only the parameter at index 0.
    fn hoist_lambda(&mut self, lam_body: &Node, name_hint: &str) -> Result<FunctionValue<'ctx>> {
        check_closed(lam_body, 1)?;

        let fn_name = self.fresh_fn_name(name_hint);
        let i64_t = self.context.i64_type();
        let fn_ty = i64_t.fn_type(&[BasicMetadataTypeEnum::IntType(i64_t)], false);
        let fn_val = self
            .module
            .add_function(&fn_name, fn_ty, Some(Linkage::Private));
        // C calling convention is LLVM's default (ADR 0027); no override needed.

        self.compile_lambda_body(fn_val, lam_body)?;
        Ok(fn_val)
    }

    fn compile_lambda_body(&mut self, fn_val: FunctionValue<'ctx>, body: &Node) -> Result<()> {
        let saved_block = self.builder.get_insert_block();
        let entry = self.context.append_basic_block(fn_val, "entry");
        self.builder.position_at_end(entry);

        let param = fn_val
            .get_nth_param(0)
            .ok_or_else(|| CodegenError::Llvm("lambda missing param 0".into()))?
            .into_int_value();
        let env = vec![Binding::Value(param)];

        let v = self.compile_expr(body, &env, fn_val)?;
        self.builder
            .build_return(Some(&v))
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        if let Some(saved) = saved_block {
            self.builder.position_at_end(saved);
        }
        Ok(())
    }

    /// `Rec { bindings, body }`: forward-declare every binding member as a
    /// top-level `i64 → i64` function, then define each body, then emit
    /// `body` in the current scope (ADR 0027 § 1).
    ///
    /// Phase 1 restriction: every binding member is a `Lam`. (Non-`Lam`
    /// members would create mutually recursive *values*, which is a
    /// different problem.) Each member sees all members at DeBruijn
    /// indices 0..N (per ADR 0007).
    fn compile_rec(
        &mut self,
        bindings: &[Node],
        body: &Node,
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        // Phase 1: every Rec member must be a Lam.
        for (i, b) in bindings.iter().enumerate() {
            if !matches!(b, Node::Lam { .. }) {
                return Err(CodegenError::RecGroupFailed {
                    failing_index: i,
                    cause: Box::new(CodegenError::Unsupported("rec member that is not a lambda")),
                });
            }
        }

        // Forward-declare all N functions.
        let n = bindings.len();
        let i64_t = self.context.i64_type();
        let fn_ty = i64_t.fn_type(&[BasicMetadataTypeEnum::IntType(i64_t)], false);
        let mut fns: Vec<FunctionValue<'ctx>> = Vec::with_capacity(n);
        for _ in 0..n {
            let name = self.fresh_fn_name("rec");
            let f = self
                .module
                .add_function(&name, fn_ty, Some(Linkage::Private));
            fns.push(f);
        }

        // Build the rec-frame env once; it's the same for every member's body
        // and for the rec-block body. Per ADR 0007, position K = DeBruijn K,
        // so member 0 ends up at index 0 — which means in `env`, member 0 is
        // *innermost* (first), and member N-1 is *outermost* in this frame.
        // We stack the frame on top of the existing env.
        let mut rec_env: Vec<Binding<'ctx>> = Vec::with_capacity(n + env.len());
        for f in &fns {
            rec_env.insert(0, Binding::Function(*f));
        }
        // After the loop above, fns[0] is at index N-1 and fns[N-1] is at index 0.
        // Per ADR 0007 we want fns[K] at index K. Reverse:
        rec_env.reverse();
        // Now rec_env[0..N] has fns[0..N] in order. Append outer env after the
        // frame so DeBruijn N+i still resolves to env[i].
        rec_env.extend_from_slice(env);

        // Define each member body.
        for (i, b) in bindings.iter().enumerate() {
            let lam_body = match b {
                Node::Lam { body } => body,
                _ => unreachable!(),
            };
            // The lambda body sees: 1 param at index 0, then the rec frame at
            // 1..=N, then the outer env. Build the per-body env accordingly.
            // (Done inline in compile_lambda_body_with_env below.)
            self.compile_lambda_body_with_rec_env(fns[i], lam_body, &rec_env)
                .map_err(|cause| CodegenError::RecGroupFailed {
                    failing_index: i,
                    cause: Box::new(cause),
                })?;
        }

        // Compile the rec-block body in the current scope, with the rec frame
        // on top of the existing env.
        self.compile_expr(body, &rec_env, cur_fn)
    }

    fn compile_lambda_body_with_rec_env(
        &mut self,
        fn_val: FunctionValue<'ctx>,
        body: &Node,
        rec_env: &[Binding<'ctx>],
    ) -> Result<()> {
        let saved_block = self.builder.get_insert_block();
        let entry = self.context.append_basic_block(fn_val, "entry");
        self.builder.position_at_end(entry);
        let param = fn_val
            .get_nth_param(0)
            .ok_or_else(|| CodegenError::Llvm("lambda missing param 0".into()))?
            .into_int_value();
        let mut body_env: Vec<Binding<'ctx>> = vec![Binding::Value(param)];
        body_env.extend_from_slice(rec_env);
        let v = self.compile_expr(body, &body_env, fn_val)?;
        self.builder
            .build_return(Some(&v))
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        if let Some(saved) = saved_block {
            self.builder.position_at_end(saved);
        }
        Ok(())
    }

    /// `Match` over an integer scrutinee. Each `arm` has a `pat-ctor "<int>"`
    /// or `pat-wild` pattern. `pat-wild` must appear at most once and only
    /// as the trailing arm (Phase 1 simplification). Lowers as a chain of
    /// `icmp eq` branches; the wildcard arm becomes the final fallthrough.
    /// Without a wildcard, fall-through results in `NonExhaustiveMatch`-trap
    /// (emitted as `@exit(NonExhaustive)` so the binary terminates rather
    /// than UB-trap; this keeps smoke programs deterministic).
    fn compile_match(
        &mut self,
        scrutinee: &Node,
        arms: &[Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let scrut = self.compile_expr(scrutinee, env, cur_fn)?;

        let merge_bb = self.context.append_basic_block(cur_fn, "match_end");
        // Collect (basic-block, value) for the phi at merge.
        let mut incoming: Vec<(IntValue<'ctx>, BasicBlock<'ctx>)> = Vec::new();

        // Iterate arms in order. Track whether a wildcard has been seen.
        let mut wildcard_seen = false;
        for (i, arm) in arms.iter().enumerate() {
            if wildcard_seen {
                return Err(CodegenError::UnsupportedMatchPattern); // wildcard must be last
            }
            let (pat, body) = match arm {
                Node::Arm { pattern, body } => (pattern.as_ref(), body.as_ref()),
                _ => return Err(CodegenError::Unsupported("match child must be arm")),
            };
            match pat {
                Node::PatWild => {
                    wildcard_seen = true;
                    let v = self.compile_expr(body, env, cur_fn)?;
                    let end_bb = self.builder.get_insert_block().unwrap();
                    incoming.push((v, end_bb));
                    self.builder
                        .build_unconditional_branch(merge_bb)
                        .map_err(|e| CodegenError::Llvm(e.to_string()))?;
                }
                Node::PatCtor { name, sub_patterns } => {
                    if !sub_patterns.is_empty() {
                        return Err(CodegenError::UnsupportedMatchPattern);
                    }
                    let lit: i64 = name
                        .parse()
                        .map_err(|_| CodegenError::UnsupportedMatchPattern)?;
                    let lit_val = self.context.i64_type().const_int(lit as u64, true);
                    let cond = self
                        .builder
                        .build_int_compare(IntPredicate::EQ, scrut, lit_val, "arm_cmp")
                        .map_err(|e| CodegenError::Llvm(e.to_string()))?;
                    let arm_bb = self
                        .context
                        .append_basic_block(cur_fn, &format!("arm{}", i));
                    let next_bb = self
                        .context
                        .append_basic_block(cur_fn, &format!("next{}", i));
                    self.builder
                        .build_conditional_branch(cond, arm_bb, next_bb)
                        .map_err(|e| CodegenError::Llvm(e.to_string()))?;
                    self.builder.position_at_end(arm_bb);
                    let v = self.compile_expr(body, env, cur_fn)?;
                    let end_bb = self.builder.get_insert_block().unwrap();
                    incoming.push((v, end_bb));
                    self.builder
                        .build_unconditional_branch(merge_bb)
                        .map_err(|e| CodegenError::Llvm(e.to_string()))?;
                    self.builder.position_at_end(next_bb);
                }
                _ => return Err(CodegenError::UnsupportedMatchPattern),
            }
        }

        if !wildcard_seen {
            // Non-exhaustive: emit a deterministic trap via libc exit(101).
            // 101 is a Phase-1 sentinel for "non-exhaustive match"; not user-
            // observable in the smoke corpus, which always provides a wildcard
            // arm or covers the value space.
            let f = self.libc_exit();
            let code = self.context.i32_type().const_int(101, false);
            self.builder
                .build_call(f, &[BasicMetadataValueEnum::IntValue(code)], "trap")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
            self.builder
                .build_unreachable()
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        }

        self.builder.position_at_end(merge_bb);
        if incoming.is_empty() {
            // No arms produced a value path. Match must have at least one arm
            // by canonical-form rules, so this is unreachable; emit i64 0
            // placeholder.
            return Ok(self.context.i64_type().const_zero());
        }
        let phi = self
            .builder
            .build_phi(self.context.i64_type(), "match_val")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let refs: Vec<(&dyn inkwell::values::BasicValue<'ctx>, BasicBlock<'ctx>)> = incoming
            .iter()
            .map(|(v, b)| (v as &dyn inkwell::values::BasicValue<'ctx>, *b))
            .collect();
        phi.add_incoming(&refs);
        Ok(phi.as_basic_value().into_int_value())
    }
}

fn lookup_var<'a, 'ctx>(env: &'a [Binding<'ctx>], idx: u64) -> Result<&'a Binding<'ctx>> {
    let i = idx as usize;
    if i >= env.len() {
        return Err(CodegenError::FreeVarInLambda { index: idx });
    }
    Ok(&env[i])
}
