//! AST → LLVM IR via `inkwell`. Built only when an `llvm<N>-<M>` feature
//! is enabled; without it, this module is excluded from the crate.
//!
//! ## Lowering model (Phase 1)
//!
//! - The whole input program is the body of an implicit `int main(void)`.
//! - The program's value, computed as `i64`, becomes `main`'s exit code
//!   (truncated to `i32` per C runtime convention; ADR 0025 § Phase 1
//!   libc set notes `return 0` is preferred over `exit(0)`).
//! - `let`/anonymous `Lam` chains are closed (ADR 0026) and hoisted as
//!   top-level LLVM functions under default C calling convention (ADR 0027).
//!   `rec` members lower as direct-call helpers; Phase 3 permits hidden
//!   parameters for captured runtime values and buffers (ADR 0059).
//!   Direct calls may supply every source-level argument in a consecutive
//!   lambda chain; partial application remains unsupported.
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

use inkwell::attributes::{Attribute, AttributeLoc};
use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::{Linkage, Module};
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine,
};
use inkwell::types::BasicMetadataTypeEnum;
use inkwell::values::{BasicMetadataValueEnum, BasicValue, FunctionValue, IntValue, PointerValue};
use inkwell::{AddressSpace, IntPredicate, OptimizationLevel};

/// `LLVMCCallConv` from `llvm-sys`. Not exposed as a typed enum by inkwell
/// 0.9, which takes a raw `u32` for `set_call_conventions`. ADR 0027 § 2.
const LLVM_C_CALL_CONV: u32 = 0;

use tacit_canonical::ast::Node;

use crate::analysis::{
    check_closed, check_no_holes, collect_lam_chain, parse_int_literal, sanitize, unfold_app,
};
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

/// Build the module and return its textual LLVM IR as a `String` (for `--emit-llvm-ir`).
pub fn compile_to_ir_string(node: &Node, module_name: &str) -> Result<String> {
    let context = Context::create();
    let mut compiler = Compiler::new(&context, module_name);
    compiler.compile_program(node)?;
    Ok(compiler.print_to_string())
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
    Function(FunctionBinding<'ctx>),
    /// A stack-allocated byte buffer (from `@buf-alloc N`). Only valid as
    /// a buffer argument to `@read` / `@write`; not a first-class value (ADR 0038).
    Ptr(PointerValue<'ctx>),
}

#[derive(Clone)]
struct FunctionBinding<'ctx> {
    value: FunctionValue<'ctx>,
    arity: usize,
    captures: Vec<Binding<'ctx>>,
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
        main_fn.set_call_conventions(LLVM_C_CALL_CONV); // ADR 0027 § 2

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
                    Binding::Ptr(_) => Err(CodegenError::Unsupported(
                        "buffer pointer used in integer-value position",
                    )),
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
            Node::PatWild | Node::PatVar | Node::PatCtor { .. } | Node::PatInt { .. } => {
                Err(CodegenError::Unsupported("pattern outside match arm"))
            }
            Node::FnTy { .. }
            | Node::TyVar { .. }
            | Node::Forall { .. }
            | Node::EffSet { .. }
            | Node::EffVar { .. } => Err(CodegenError::Unsupported(
                "type expression in value position",
            )),
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
        // Special-case Let-of-Lam-chain: hoist the closed lambda chain to a
        // top-level function and bind the body under a `Function` entry.
        // ADR 0026 § 3 second bullet; Phase 3 permits direct multi-arg calls
        // when all arguments are supplied at the call site.
        if let Some((arity, lam_body)) = collect_lam_chain(rhs) {
            let fn_val = self.hoist_lambda(lam_body, arity, "let")?;
            let mut new_env = env.to_vec();
            new_env.insert(0, Binding::Function(fn_val));
            return self.compile_expr(body, &new_env, cur_fn);
        }

        // Special-case Let-of-BufAlloc (ADR 0038): `let buf = @buf-alloc N in ...`
        // emits an `alloca [N x i8]` at the function entry and binds it as `Ptr`.
        // Also handles @buf-alloc-dyn (ADR 0047): `alloca i8, %n` for runtime size.
        if let Node::App { .. } = rhs {
            let (head, args) = unfold_app(rhs);
            if let Node::Sym { name } = head {
                if name == "buf-alloc" && args.len() == 1 {
                    if let Node::Int { value } = args[0] {
                        let size: u64 = value.parse().map_err(|_| {
                            CodegenError::Llvm(format!("bad buf-alloc size: {}", value))
                        })?;
                        let i8_t = self.context.i8_type();
                        let arr_ty = i8_t.array_type(size as u32);
                        let alloca = self
                            .builder
                            .build_alloca(arr_ty, "buf")
                            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
                        let zero = self.context.i32_type().const_zero();
                        let buf_ptr = unsafe {
                            self.builder.build_in_bounds_gep(
                                arr_ty,
                                alloca,
                                &[zero, zero],
                                "buf_ptr",
                            )
                        }
                        .map_err(|e| CodegenError::Llvm(e.to_string()))?;
                        let mut new_env = env.to_vec();
                        new_env.insert(0, Binding::Ptr(buf_ptr));
                        return self.compile_expr(body, &new_env, cur_fn);
                    }
                }
                if name == "buf-alloc-dyn" && args.len() == 1 {
                    // Runtime-sized stack allocation: alloca i8, %n (ADR 0047).
                    let size_val = self.compile_expr(args[0], env, cur_fn)?;
                    let i8_t = self.context.i8_type();
                    let buf_ptr = self
                        .builder
                        .build_array_alloca(i8_t, size_val, "buf_dyn")
                        .map_err(|e| CodegenError::Llvm(e.to_string()))?;
                    let mut new_env = env.to_vec();
                    new_env.insert(0, Binding::Ptr(buf_ptr));
                    return self.compile_expr(body, &new_env, cur_fn);
                }
            }
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
    ///   2. `Lam(_)` head     → collect a closed lambda chain, hoist + direct call.
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
            Node::Lam { .. } => {
                let (arity, lam_body) =
                    collect_lam_chain(head).expect("Node::Lam has a lambda chain");
                if args.len() != arity {
                    return Err(CodegenError::FunctionArity {
                        expected: arity,
                        got: args.len(),
                    });
                }
                let fn_val = self.hoist_lambda(lam_body, arity, "anon")?;
                let arg_vals = self.compile_call_args(&args, env, cur_fn)?;
                self.call_function(&fn_val, &arg_vals)
            }
            Node::Var { index } => {
                let binding = lookup_var(env, *index)?;
                match binding {
                    Binding::Function(fn_val) => {
                        if args.len() != fn_val.arity {
                            return Err(CodegenError::FunctionArity {
                                expected: fn_val.arity,
                                got: args.len(),
                            });
                        }
                        let arg_vals = self.compile_call_args(&args, env, cur_fn)?;
                        self.call_function(fn_val, &arg_vals)
                    }
                    Binding::Value(_) | Binding::Ptr(_) => Err(CodegenError::AppNonFunction),
                }
            }
            _ => Err(CodegenError::AppNonFunction),
        }
    }

    fn compile_call_args(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<Vec<IntValue<'ctx>>> {
        let mut values = Vec::with_capacity(args.len());
        for arg in args {
            values.push(self.compile_expr(arg, env, cur_fn)?);
        }
        Ok(values)
    }

    fn call_function(
        &mut self,
        fn_binding: &FunctionBinding<'ctx>,
        args: &[IntValue<'ctx>],
    ) -> Result<IntValue<'ctx>> {
        let mut call_args: Vec<BasicMetadataValueEnum<'ctx>> = args
            .iter()
            .copied()
            .map(BasicMetadataValueEnum::IntValue)
            .collect();
        for capture in &fn_binding.captures {
            match capture {
                Binding::Value(v) => call_args.push(BasicMetadataValueEnum::IntValue(*v)),
                Binding::Ptr(p) => call_args.push(BasicMetadataValueEnum::PointerValue(*p)),
                Binding::Function(_) => {}
            }
        }
        let call = self
            .builder
            .build_call(fn_binding.value, &call_args, "call")
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
                self.emit_arith(op, a, b)
            }
            PrimKind::Cmp(op) => {
                let a = self.compile_expr(args[0], env, cur_fn)?;
                let b = self.compile_expr(args[1], env, cur_fn)?;
                self.emit_cmp(op, a, b)
            }
            PrimKind::Write => self.emit_write(args, env, cur_fn),
            PrimKind::Read => self.emit_read(args, env, cur_fn),
            PrimKind::Exit => self.emit_exit(args, env, cur_fn),
            PrimKind::BufAlloc => Err(CodegenError::Unsupported(
                "@buf-alloc must appear as the direct RHS of a `let` binding",
            )),
            PrimKind::BufAllocDyn => Err(CodegenError::Unsupported(
                "@buf-alloc-dyn must appear as the direct RHS of a `let` binding",
            )),
            PrimKind::BufGet => self.emit_buf_get(args, env, cur_fn),
            PrimKind::BufSet => self.emit_buf_set(args, env, cur_fn),
            PrimKind::BufCopy => self.emit_buf_copy(args, env, cur_fn),
            PrimKind::BufEq => self.emit_buf_eq(args, env, cur_fn),
            PrimKind::ScanByte => self.emit_scan_byte(args, env, cur_fn),
            PrimKind::ParseI64 => self.emit_parse_i64(args, env, cur_fn),
            PrimKind::FmtI64 => self.emit_fmt_i64(args, env, cur_fn),
        }
    }

    fn emit_arith(
        &mut self,
        op: ArithOp,
        a: IntValue<'ctx>,
        b: IntValue<'ctx>,
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
        let kind_id = Attribute::get_named_enum_kind_id("noreturn");
        let attr = self.context.create_enum_attribute(kind_id, 0);
        f.add_attribute(AttributeLoc::Function, attr);
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
        // exit() is noreturn (attribute set on the libc decl). Terminate the
        // current block with `unreachable`, then position the builder in a
        // fresh block so callers higher in `compile_expr` can keep emitting
        // — they expect a builder positioned at an open block and an `i64`
        // result value. Both are placeholders: the new block is dead (not
        // reachable from `entry`), and the returned `const_zero` is never
        // observed at runtime because control never falls past `unreachable`.
        // LLVM's verifier accepts dead blocks; subsequent IR emitted into
        // `after_exit` (e.g. `compile_program`'s `ret i32`) is well-formed
        // unreachable code.
        self.builder
            .build_unreachable()
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let dead = self.context.append_basic_block(cur_fn, "after_exit");
        self.builder.position_at_end(dead);
        Ok(self.context.i64_type().const_zero())
    }

    /// Lower a buffer argument for `@write` / `@read`.
    /// Accepts either a string literal (→ private global) or a `Var` resolving
    /// to a `Binding::Ptr` (→ stack buffer from `@buf-alloc`, ADR 0038).
    fn compile_buffer_arg(
        &mut self,
        node: &Node,
        hint: &str,
        constant: bool,
        env: &[Binding<'ctx>],
        _cur_fn: FunctionValue<'ctx>,
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
            // `Var` may resolve to a `Binding::Ptr` produced by `@buf-alloc` (ADR 0038).
            Node::Var { index } => {
                let binding = lookup_var(env, *index)?;
                match binding {
                    Binding::Ptr(ptr) => Ok(*ptr),
                    _ => Err(CodegenError::Unsupported(
                        "buffer argument must be a string literal or @buf-alloc binding",
                    )),
                }
            }
            _ => Err(CodegenError::Unsupported(
                "buffer argument must be a string literal or @buf-alloc binding",
            )),
        }
    }

    // ── Phase 3 primitive helpers (ADR 0047) ────────────────────────────────────

    /// Resolve a buffer argument to a `PointerValue` (must be a `Var` → `Binding::Ptr`).
    fn compile_ptr_arg<'a>(
        &self,
        node: &Node,
        env: &'a [Binding<'ctx>],
    ) -> Result<PointerValue<'ctx>> {
        match node {
            Node::Var { index } => {
                let binding = lookup_var(env, *index)?;
                match binding {
                    Binding::Ptr(ptr) => Ok(*ptr),
                    _ => Err(CodegenError::Unsupported(
                        "buffer argument must be a @buf-alloc / @buf-alloc-dyn binding",
                    )),
                }
            }
            _ => Err(CodegenError::Unsupported(
                "buffer argument must be a variable referencing a buffer binding",
            )),
        }
    }

    /// GEP to `buf_ptr[off]` (element type `i8`).
    fn ptr_at(
        &mut self,
        buf_ptr: PointerValue<'ctx>,
        off: IntValue<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>> {
        let i8_t = self.context.i8_type();
        unsafe { self.builder.build_gep(i8_t, buf_ptr, &[off], name) }
            .map_err(|e| CodegenError::Llvm(e.to_string()))
    }

    /// Load the byte at `buf_ptr[off]` and zero-extend to `i64`.
    fn load_byte(
        &mut self,
        buf_ptr: PointerValue<'ctx>,
        off: IntValue<'ctx>,
        name: &str,
    ) -> Result<IntValue<'ctx>> {
        let i8_t = self.context.i8_type();
        let i64_t = self.context.i64_type();
        let ptr = self.ptr_at(buf_ptr, off, name)?;
        let b = self
            .builder
            .build_load(i8_t, ptr, name)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_int_z_extend(b.into_int_value(), i64_t, name)
            .map_err(|e| CodegenError::Llvm(e.to_string()))
    }

    /// Store the low byte of `byte_val` at `buf_ptr[off]`.
    fn store_byte(
        &mut self,
        buf_ptr: PointerValue<'ctx>,
        off: IntValue<'ctx>,
        byte_val: IntValue<'ctx>,
    ) -> Result<()> {
        let i8_t = self.context.i8_type();
        let trunc = self
            .builder
            .build_int_truncate(byte_val, i8_t, "byte_trunc")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let ptr = self.ptr_at(buf_ptr, off, "store_ptr")?;
        self.builder
            .build_store(ptr, trunc)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        Ok(())
    }

    /// Declare (or retrieve) the `llvm.memcpy.p0.p0.i64` intrinsic (ADR 0047).
    fn llvm_memcpy(&self) -> FunctionValue<'ctx> {
        let name = "llvm.memcpy.p0.p0.i64";
        if let Some(f) = self.module.get_function(name) {
            return f;
        }
        let ptr_t = self.context.ptr_type(AddressSpace::default());
        let i64_t = self.context.i64_type();
        let i1_t = self.context.bool_type();
        let void_t = self.context.void_type();
        let fn_ty = void_t.fn_type(
            &[
                BasicMetadataTypeEnum::PointerType(ptr_t),
                BasicMetadataTypeEnum::PointerType(ptr_t),
                BasicMetadataTypeEnum::IntType(i64_t),
                BasicMetadataTypeEnum::IntType(i1_t),
            ],
            false,
        );
        self.module.add_function(name, fn_ty, None)
    }

    // ── Phase 3 emit functions (ADR 0047) ────────────────────────────────────────

    /// `@buf-get buf off` → load `buf[off]` as `i64`.
    fn emit_buf_get(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let buf = self.compile_ptr_arg(args[0], env)?;
        let off = self.compile_expr(args[1], env, cur_fn)?;
        self.load_byte(buf, off, "bg")
    }

    /// `@buf-set buf off byte` → store `byte` at `buf[off]`; return 0.
    fn emit_buf_set(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let buf = self.compile_ptr_arg(args[0], env)?;
        let off = self.compile_expr(args[1], env, cur_fn)?;
        let byte = self.compile_expr(args[2], env, cur_fn)?;
        self.store_byte(buf, off, byte)?;
        Ok(self.context.i64_type().const_zero())
    }

    /// `@buf-copy dst dst-off src src-off len` → `llvm.memcpy`; return 0.
    fn emit_buf_copy(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let dst = self.compile_ptr_arg(args[0], env)?;
        let dst_off = self.compile_expr(args[1], env, cur_fn)?;
        let src = self.compile_ptr_arg(args[2], env)?;
        let src_off = self.compile_expr(args[3], env, cur_fn)?;
        let len = self.compile_expr(args[4], env, cur_fn)?;

        let dst_ptr = self.ptr_at(dst, dst_off, "cp_dst")?;
        let src_ptr = self.ptr_at(src, src_off, "cp_src")?;
        let false_val = self.context.bool_type().const_int(0, false);

        let memcpy = self.llvm_memcpy();
        self.builder
            .build_call(
                memcpy,
                &[
                    BasicMetadataValueEnum::PointerValue(dst_ptr),
                    BasicMetadataValueEnum::PointerValue(src_ptr),
                    BasicMetadataValueEnum::IntValue(len),
                    BasicMetadataValueEnum::IntValue(false_val),
                ],
                "memcpy",
            )
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        Ok(self.context.i64_type().const_zero())
    }

    /// `@buf-eq a a-off b b-off len` → inline byte-compare loop; returns 0 or 1.
    ///
    /// Empty range (len = 0) returns 1 (vacuously equal).
    fn emit_buf_eq(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let a = self.compile_ptr_arg(args[0], env)?;
        let a_off = self.compile_expr(args[1], env, cur_fn)?;
        let b = self.compile_ptr_arg(args[2], env)?;
        let b_off = self.compile_expr(args[3], env, cur_fn)?;
        let len = self.compile_expr(args[4], env, cur_fn)?;

        let i64_t = self.context.i64_type();
        let i8_t = self.context.i8_type();
        let zero64 = i64_t.const_zero();
        let one64 = i64_t.const_int(1, false);

        // Base pointers: a_ptr = a[a_off], b_ptr = b[b_off]
        let a_base = self.ptr_at(a, a_off, "beq_a")?;
        let b_base = self.ptr_at(b, b_off, "beq_b")?;

        let entry_bb = self.builder.get_insert_block().unwrap();
        let hdr_bb = self.context.append_basic_block(cur_fn, "beq_hdr");
        let check_bb = self.context.append_basic_block(cur_fn, "beq_chk");
        let cont_bb = self.context.append_basic_block(cur_fn, "beq_cont");
        let eq_bb = self.context.append_basic_block(cur_fn, "beq_eq");
        let ne_bb = self.context.append_basic_block(cur_fn, "beq_ne");
        let merge_bb = self.context.append_basic_block(cur_fn, "beq_merge");

        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // loop header: i = phi(0, i+1)
        self.builder.position_at_end(hdr_bb);
        let i_phi = self
            .builder
            .build_phi(i64_t, "beq_i")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let i_val = i_phi.as_basic_value().into_int_value();
        let done = self
            .builder
            .build_int_compare(IntPredicate::SGE, i_val, len, "beq_done")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(done, eq_bb, check_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // load and compare a[i] vs b[i]
        self.builder.position_at_end(check_bb);
        let pa = unsafe {
            self.builder
                .build_gep(i8_t, a_base, &[i_val], "pa")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?
        };
        let pb = unsafe {
            self.builder
                .build_gep(i8_t, b_base, &[i_val], "pb")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?
        };
        let ba = self
            .builder
            .build_load(i8_t, pa, "ba")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_int_value();
        let bb = self
            .builder
            .build_load(i8_t, pb, "bb")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_int_value();
        let bytes_eq = self
            .builder
            .build_int_compare(IntPredicate::EQ, ba, bb, "bytes_eq")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(bytes_eq, cont_bb, ne_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // continue: i++
        self.builder.position_at_end(cont_bb);
        let i_next = self
            .builder
            .build_int_add(i_val, one64, "beq_inext")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // add phi incoming edges now that i_next is defined
        i_phi.add_incoming(&[
            (&zero64 as &dyn BasicValue<'ctx>, entry_bb),
            (&i_next as &dyn BasicValue<'ctx>, cont_bb),
        ]);

        // equal exit
        self.builder.position_at_end(eq_bb);
        self.builder
            .build_unconditional_branch(merge_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // not-equal exit
        self.builder.position_at_end(ne_bb);
        self.builder
            .build_unconditional_branch(merge_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(merge_bb);
        let result_phi = self
            .builder
            .build_phi(i64_t, "beq_res")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        result_phi.add_incoming(&[
            (&one64 as &dyn BasicValue<'ctx>, eq_bb),
            (&zero64 as &dyn BasicValue<'ctx>, ne_bb),
        ]);
        Ok(result_phi.as_basic_value().into_int_value())
    }

    /// `@scan-byte buf off len target` → find first `target` byte; returns index or off+len.
    fn emit_scan_byte(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let buf = self.compile_ptr_arg(args[0], env)?;
        let off = self.compile_expr(args[1], env, cur_fn)?;
        let len = self.compile_expr(args[2], env, cur_fn)?;
        let target = self.compile_expr(args[3], env, cur_fn)?;

        let i64_t = self.context.i64_type();
        let i8_t = self.context.i8_type();
        let one64 = i64_t.const_int(1, false);

        let end = self
            .builder
            .build_int_add(off, len, "sb_end")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        let entry_bb = self.builder.get_insert_block().unwrap();
        let hdr_bb = self.context.append_basic_block(cur_fn, "sb_hdr");
        let check_bb = self.context.append_basic_block(cur_fn, "sb_chk");
        let cont_bb = self.context.append_basic_block(cur_fn, "sb_cont");
        let exit_bb = self.context.append_basic_block(cur_fn, "sb_exit");

        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // loop header: i = phi(off, i+1)
        self.builder.position_at_end(hdr_bb);
        let i_phi = self
            .builder
            .build_phi(i64_t, "sb_i")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let i_val = i_phi.as_basic_value().into_int_value();
        let past_end = self
            .builder
            .build_int_compare(IntPredicate::SGE, i_val, end, "sb_past")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(past_end, exit_bb, check_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // load byte and compare
        self.builder.position_at_end(check_bb);
        let ptr_i = unsafe {
            self.builder
                .build_gep(i8_t, buf, &[i_val], "sb_ptr")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?
        };
        let byte = self
            .builder
            .build_load(i8_t, ptr_i, "sb_b")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_int_value();
        let byte64 = self
            .builder
            .build_int_z_extend(byte, i64_t, "sb_b64")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let found = self
            .builder
            .build_int_compare(IntPredicate::EQ, byte64, target, "sb_found")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        // branch: found → exit with current i; not found → continue
        self.builder
            .build_conditional_branch(found, exit_bb, cont_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // continue: i++
        self.builder.position_at_end(cont_bb);
        let i_next = self
            .builder
            .build_int_add(i_val, one64, "sb_inext")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        i_phi.add_incoming(&[
            (&off as &dyn BasicValue<'ctx>, entry_bb),
            (&i_next as &dyn BasicValue<'ctx>, cont_bb),
        ]);

        // exit: result = phi(end from hdr, i from check)
        self.builder.position_at_end(exit_bb);
        let res_phi = self
            .builder
            .build_phi(i64_t, "sb_res")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        res_phi.add_incoming(&[
            (&end as &dyn BasicValue<'ctx>, hdr_bb),
            (&i_val as &dyn BasicValue<'ctx>, check_bb),
        ]);
        Ok(res_phi.as_basic_value().into_int_value())
    }

    /// `@parse-i64 buf off len` → parse decimal integer (with optional '-'); overflow is UB.
    ///
    /// Stops at first non-digit. Empty range or no leading digit returns 0.
    fn emit_parse_i64(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let buf = self.compile_ptr_arg(args[0], env)?;
        let off = self.compile_expr(args[1], env, cur_fn)?;
        let len = self.compile_expr(args[2], env, cur_fn)?;

        let i64_t = self.context.i64_type();
        let i8_t = self.context.i8_type();
        let zero64 = i64_t.const_zero();
        let one64 = i64_t.const_int(1, false);
        let ten64 = i64_t.const_int(10, false);
        let minus_ascii = i64_t.const_int(45, false); // '-'
        let zero_ascii = i64_t.const_int(48, false); // '0'
        let nine_ascii = i64_t.const_int(57, false); // '9'

        let end = self
            .builder
            .build_int_add(off, len, "pi_end")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // If len == 0, return 0 immediately.
        let is_empty = self
            .builder
            .build_int_compare(IntPredicate::SLE, len, zero64, "pi_empty")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        let empty_bb = self.context.append_basic_block(cur_fn, "pi_empty_bb");
        let sign_bb = self.context.append_basic_block(cur_fn, "pi_sign");
        let hdr_bb = self.context.append_basic_block(cur_fn, "pi_hdr");
        let digit_bb = self.context.append_basic_block(cur_fn, "pi_digit");
        let acc_bb = self.context.append_basic_block(cur_fn, "pi_acc");
        let finish_bb = self.context.append_basic_block(cur_fn, "pi_finish");
        let ret_bb = self.context.append_basic_block(cur_fn, "pi_ret");

        self.builder
            .build_conditional_branch(is_empty, empty_bb, sign_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // empty path
        self.builder.position_at_end(empty_bb);
        self.builder
            .build_unconditional_branch(ret_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // check for leading '-'
        self.builder.position_at_end(sign_bb);
        let first_ptr = unsafe {
            self.builder
                .build_gep(i8_t, buf, &[off], "pi_fptr")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?
        };
        let first_b = self
            .builder
            .build_load(i8_t, first_ptr, "pi_fb")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_int_value();
        let first64 = self
            .builder
            .build_int_z_extend(first_b, i64_t, "pi_f64")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let is_neg = self
            .builder
            .build_int_compare(IntPredicate::EQ, first64, minus_ascii, "pi_neg")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let start = self
            .builder
            .build_select(
                is_neg,
                self.builder
                    .build_int_add(off, one64, "pi_s1")
                    .map_err(|e| CodegenError::Llvm(e.to_string()))?,
                off,
                "pi_start",
            )
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_int_value();
        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // loop header: i = phi(start, i+1); acc = phi(0, new_acc)
        self.builder.position_at_end(hdr_bb);
        let i_phi = self
            .builder
            .build_phi(i64_t, "pi_i")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let acc_phi = self
            .builder
            .build_phi(i64_t, "pi_acc")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let i_val = i_phi.as_basic_value().into_int_value();
        let acc_val = acc_phi.as_basic_value().into_int_value();
        let done = self
            .builder
            .build_int_compare(IntPredicate::SGE, i_val, end, "pi_done")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(done, finish_bb, digit_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // check digit
        self.builder.position_at_end(digit_bb);
        let ptr_i = unsafe {
            self.builder
                .build_gep(i8_t, buf, &[i_val], "pi_ptr")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?
        };
        let b = self
            .builder
            .build_load(i8_t, ptr_i, "pi_b")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_int_value();
        let b64 = self
            .builder
            .build_int_z_extend(b, i64_t, "pi_b64")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let d_lo = self
            .builder
            .build_int_compare(IntPredicate::SGE, b64, zero_ascii, "pi_dlo")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let d_hi = self
            .builder
            .build_int_compare(IntPredicate::SLE, b64, nine_ascii, "pi_dhi")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let valid = self
            .builder
            .build_and(d_lo, d_hi, "pi_valid")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(valid, acc_bb, finish_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // accumulate: new_acc = acc * 10 + (b64 - '0')
        self.builder.position_at_end(acc_bb);
        let digit = self
            .builder
            .build_int_sub(b64, zero_ascii, "pi_d")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let acc_times_10 = self
            .builder
            .build_int_nsw_mul(acc_val, ten64, "pi_a10")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let new_acc = self
            .builder
            .build_int_nsw_add(acc_times_10, digit, "pi_nacc")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let i_next = self
            .builder
            .build_int_add(i_val, one64, "pi_inext")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        i_phi.add_incoming(&[
            (&start as &dyn BasicValue<'ctx>, sign_bb),
            (&i_next as &dyn BasicValue<'ctx>, acc_bb),
        ]);
        acc_phi.add_incoming(&[
            (&zero64 as &dyn BasicValue<'ctx>, sign_bb),
            (&new_acc as &dyn BasicValue<'ctx>, acc_bb),
        ]);

        // finish: raw = acc from loop header (both paths from hdr and digit_bb share the same phi)
        self.builder.position_at_end(finish_bb);
        let raw_phi = self
            .builder
            .build_phi(i64_t, "pi_raw")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        raw_phi.add_incoming(&[
            (&acc_val as &dyn BasicValue<'ctx>, hdr_bb),
            (&acc_val as &dyn BasicValue<'ctx>, digit_bb),
        ]);
        let raw = raw_phi.as_basic_value().into_int_value();
        let neg_raw = self
            .builder
            .build_int_neg(raw, "pi_negraw")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let result = self
            .builder
            .build_select(is_neg, neg_raw, raw, "pi_res")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_int_value();
        self.builder
            .build_unconditional_branch(ret_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // return: phi(0 from empty, result from finish)
        self.builder.position_at_end(ret_bb);
        let final_phi = self
            .builder
            .build_phi(i64_t, "pi_final")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        final_phi.add_incoming(&[
            (&zero64 as &dyn BasicValue<'ctx>, empty_bb),
            (&result as &dyn BasicValue<'ctx>, finish_bb),
        ]);
        Ok(final_phi.as_basic_value().into_int_value())
    }

    /// `@fmt-i64 buf off val` → write decimal representation of `val` to `buf[off..]`;
    /// returns the number of bytes written. Handles 0 as a special case.
    fn emit_fmt_i64(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let buf = self.compile_ptr_arg(args[0], env)?;
        let off = self.compile_expr(args[1], env, cur_fn)?;
        let val = self.compile_expr(args[2], env, cur_fn)?;

        let i64_t = self.context.i64_type();
        let i8_t = self.context.i8_type();
        let zero64 = i64_t.const_zero();
        let one64 = i64_t.const_int(1, false);
        let ten64 = i64_t.const_int(10, false);
        let zero_ascii = i64_t.const_int(48, false); // '0'
        let minus_ascii = i64_t.const_int(45, false); // '-'

        let is_neg = self
            .builder
            .build_int_compare(IntPredicate::SLT, val, zero64, "fi_neg")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let neg_val = self
            .builder
            .build_int_neg(val, "fi_negval")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let abs_val = self
            .builder
            .build_select(is_neg, neg_val, val, "fi_abs")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_int_value();

        let is_zero = self
            .builder
            .build_int_compare(IntPredicate::EQ, abs_val, zero64, "fi_iszero")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        let zero_bb = self.context.append_basic_block(cur_fn, "fi_zero");
        let nonzero_bb = self.context.append_basic_block(cur_fn, "fi_nonzero");
        let sign_bb = self.context.append_basic_block(cur_fn, "fi_sign");
        let count_hdr_bb = self.context.append_basic_block(cur_fn, "fi_cnt_hdr");
        let count_body_bb = self.context.append_basic_block(cur_fn, "fi_cnt_body");
        let write_hdr_bb = self.context.append_basic_block(cur_fn, "fi_wr_hdr");
        let write_body_bb = self.context.append_basic_block(cur_fn, "fi_wr_body");
        let ret_bb = self.context.append_basic_block(cur_fn, "fi_ret");

        self.builder
            .build_conditional_branch(is_zero, zero_bb, nonzero_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // --- zero case: write '0', return 1 ---
        self.builder.position_at_end(zero_bb);
        let zero_char_byte = i8_t.const_int(48, false);
        let zero_ptr = unsafe {
            self.builder
                .build_gep(i8_t, buf, &[off], "fi_z_ptr")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?
        };
        self.builder
            .build_store(zero_ptr, zero_char_byte)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(ret_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // --- non-zero: pass 1 = count digits ---
        self.builder.position_at_end(nonzero_bb);
        self.builder
            .build_unconditional_branch(count_hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // count_hdr: n = phi(0, n+1); v = phi(abs_val, v/10)
        self.builder.position_at_end(count_hdr_bb);
        let n_phi = self
            .builder
            .build_phi(i64_t, "fi_n")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let v_phi = self
            .builder
            .build_phi(i64_t, "fi_v")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let n_val = n_phi.as_basic_value().into_int_value();
        let v_val = v_phi.as_basic_value().into_int_value();
        let v_done = self
            .builder
            .build_int_compare(IntPredicate::SLE, v_val, zero64, "fi_vdone")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(v_done, sign_bb, count_body_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(count_body_bb);
        let n_next = self
            .builder
            .build_int_add(n_val, one64, "fi_nnext")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let v_next = self
            .builder
            .build_int_signed_div(v_val, ten64, "fi_vnext")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(count_hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        n_phi.add_incoming(&[
            (&zero64 as &dyn BasicValue<'ctx>, nonzero_bb),
            (&n_next as &dyn BasicValue<'ctx>, count_body_bb),
        ]);
        v_phi.add_incoming(&[
            (&abs_val as &dyn BasicValue<'ctx>, nonzero_bb),
            (&v_next as &dyn BasicValue<'ctx>, count_body_bb),
        ]);

        // sign_bb: write '-' if negative, compute sign_len and total
        self.builder.position_at_end(sign_bb);
        let sign_len = self
            .builder
            .build_select(is_neg, one64, zero64, "fi_slen")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_int_value();
        let minus_trunc = i8_t.const_int(45, false);
        let sign_ptr = unsafe {
            self.builder
                .build_gep(i8_t, buf, &[off], "fi_sptr")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?
        };
        // Conditionally store '-': we always store but only at sign_ptr if neg
        // Simpler: just build_select the store address is always off, but store '-'
        // only if neg. Use: if is_neg { store '-' at off }
        // Build a conditional store via select: store select(is_neg, '-', buf[off])
        // We'll just store minus_trunc if is_neg using a conditional branch:
        let write_sign_bb = self.context.append_basic_block(cur_fn, "fi_wsign");
        let after_sign_bb = self.context.append_basic_block(cur_fn, "fi_asign");
        self.builder
            .build_conditional_branch(is_neg, write_sign_bb, after_sign_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(write_sign_bb);
        self.builder
            .build_store(sign_ptr, minus_trunc)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(after_sign_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(after_sign_bb);
        // total = sign_len + n_val; last digit position = off + total - 1
        let total = self
            .builder
            .build_int_add(sign_len, n_val, "fi_total")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let last_pos = self
            .builder
            .build_int_sub(
                self.builder
                    .build_int_add(off, total, "fi_end")
                    .map_err(|e| CodegenError::Llvm(e.to_string()))?,
                one64,
                "fi_last",
            )
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder
            .build_unconditional_branch(write_hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // pass 2: write digits right-to-left
        // write_hdr: pos = phi(last_pos, pos-1); u = phi(abs_val, u/10)
        self.builder.position_at_end(write_hdr_bb);
        let pos_phi = self
            .builder
            .build_phi(i64_t, "fi_pos")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let u_phi = self
            .builder
            .build_phi(i64_t, "fi_u")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let pos_val = pos_phi.as_basic_value().into_int_value();
        let u_val = u_phi.as_basic_value().into_int_value();
        let u_done = self
            .builder
            .build_int_compare(IntPredicate::SLE, u_val, zero64, "fi_udone")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(u_done, ret_bb, write_body_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(write_body_bb);
        let digit = self
            .builder
            .build_int_signed_rem(u_val, ten64, "fi_dgt")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let digit_char = self
            .builder
            .build_int_add(digit, zero_ascii, "fi_dchar")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let digit_trunc = self
            .builder
            .build_int_truncate(digit_char, i8_t, "fi_dtrunc")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let dptr = unsafe {
            self.builder
                .build_gep(i8_t, buf, &[pos_val], "fi_dptr")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?
        };
        self.builder
            .build_store(dptr, digit_trunc)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let pos_prev = self
            .builder
            .build_int_sub(pos_val, one64, "fi_pprev")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let u_next = self
            .builder
            .build_int_signed_div(u_val, ten64, "fi_unext")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(write_hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        pos_phi.add_incoming(&[
            (&last_pos as &dyn BasicValue<'ctx>, after_sign_bb),
            (&pos_prev as &dyn BasicValue<'ctx>, write_body_bb),
        ]);
        u_phi.add_incoming(&[
            (&abs_val as &dyn BasicValue<'ctx>, after_sign_bb),
            (&u_next as &dyn BasicValue<'ctx>, write_body_bb),
        ]);

        // ret_bb: result = phi(1 from zero_bb, total from write_hdr_bb)
        self.builder.position_at_end(ret_bb);
        let ret_phi = self
            .builder
            .build_phi(i64_t, "fi_ret")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        ret_phi.add_incoming(&[
            (&one64 as &dyn BasicValue<'ctx>, zero_bb),
            (&total as &dyn BasicValue<'ctx>, write_hdr_bb),
        ]);
        // suppress unused warning on these constants
        let _ = minus_ascii;
        Ok(ret_phi.as_basic_value().into_int_value())
    }

    /// Hoist a consecutive `Lam` chain to a top-level function.
    /// The lambda chain must be closed (free DeBruijn check below); body is
    /// compiled with an env containing all parameters in DeBruijn order.
    fn hoist_lambda(
        &mut self,
        lam_body: &Node,
        arity: usize,
        name_hint: &str,
    ) -> Result<FunctionBinding<'ctx>> {
        check_closed(lam_body, arity as u64)?;

        let fn_name = self.fresh_fn_name(name_hint);
        let fn_val = self.add_tacit_function(&fn_name, arity, &[]);
        // C calling convention is LLVM's default (ADR 0027); no override needed.

        self.compile_lambda_body(fn_val, lam_body, arity)?;
        Ok(FunctionBinding {
            value: fn_val,
            arity,
            captures: Vec::new(),
        })
    }

    fn add_tacit_function(
        &self,
        name: &str,
        arity: usize,
        captures: &[Binding<'ctx>],
    ) -> FunctionValue<'ctx> {
        let i64_t = self.context.i64_type();
        let ptr_t = self.context.ptr_type(AddressSpace::default());
        let mut params: Vec<BasicMetadataTypeEnum<'ctx>> = (0..arity)
            .map(|_| BasicMetadataTypeEnum::IntType(i64_t))
            .collect();
        for capture in captures {
            match capture {
                Binding::Value(_) => params.push(BasicMetadataTypeEnum::IntType(i64_t)),
                Binding::Ptr(_) => params.push(BasicMetadataTypeEnum::PointerType(ptr_t)),
                Binding::Function(_) => {}
            }
        }
        let fn_ty = i64_t.fn_type(&params, false);
        self.module
            .add_function(name, fn_ty, Some(Linkage::Private))
    }

    fn compile_lambda_body(
        &mut self,
        fn_val: FunctionValue<'ctx>,
        body: &Node,
        arity: usize,
    ) -> Result<()> {
        let saved_block = self.builder.get_insert_block();
        let entry = self.context.append_basic_block(fn_val, "entry");
        self.builder.position_at_end(entry);

        let mut env = Vec::with_capacity(arity);
        for i in (0..arity).rev() {
            let param = fn_val
                .get_nth_param(i as u32)
                .ok_or_else(|| CodegenError::Llvm(format!("lambda missing param {i}")))?
                .into_int_value();
            env.push(Binding::Value(param));
        }

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
    /// top-level direct-call function, then define each body, then emit
    /// `body` in the current scope (ADR 0027 § 1).
    ///
    /// Phase 3 adds direct-call hidden captures for the outer value/pointer
    /// environment (ADR 0059). Captures are appended after source-level
    /// lambda arguments in the function signature and at each call site.
    ///
    /// Codegen restriction: every binding member is a `Lam` chain. (Non-`Lam`
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
        // Every Rec member must be a Lam chain.
        let mut specs: Vec<(usize, &Node)> = Vec::with_capacity(bindings.len());
        for (i, b) in bindings.iter().enumerate() {
            if let Some(spec) = collect_lam_chain(b) {
                specs.push(spec);
            } else {
                return Err(CodegenError::RecGroupFailed {
                    failing_index: i,
                    cause: Box::new(CodegenError::Unsupported("rec member that is not a lambda")),
                });
            }
        }

        // Forward-declare all N functions.
        let n = bindings.len();
        let mut fns: Vec<FunctionBinding<'ctx>> = Vec::with_capacity(n);
        for (arity, _) in &specs {
            let name = self.fresh_fn_name("rec");
            let f = self.add_tacit_function(&name, *arity, env);
            fns.push(FunctionBinding {
                value: f,
                arity: *arity,
                captures: env.to_vec(),
            });
        }

        // Build the rec-frame env once; it's the same for every member's body
        // and for the rec-block body. Per ADR 0007, position K = DeBruijn K,
        // so member K lives at rec_env[K]. The outer env stacks after the
        // frame so DeBruijn N+i still resolves to env[i].
        let mut rec_env: Vec<Binding<'ctx>> = Vec::with_capacity(n + env.len());
        for f in &fns {
            rec_env.push(Binding::Function(f.clone()));
        }
        rec_env.extend_from_slice(env);

        // Define each member body.
        for (i, (arity, lam_body)) in specs.iter().enumerate() {
            // The lambda body sees its params in reverse DeBruijn order,
            // then the rec frame, then hidden capture parameters standing in
            // for the outer env. Build the per-body env accordingly.
            self.compile_lambda_body_with_rec_env(fns[i].value, lam_body, *arity, &fns, env)
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
        arity: usize,
        rec_fns: &[FunctionBinding<'ctx>],
        outer_env: &[Binding<'ctx>],
    ) -> Result<()> {
        let saved_block = self.builder.get_insert_block();
        let entry = self.context.append_basic_block(fn_val, "entry");
        self.builder.position_at_end(entry);
        let captured_env = self.capture_env_from_params(fn_val, arity, outer_env)?;
        let mut rec_env: Vec<Binding<'ctx>> =
            Vec::with_capacity(rec_fns.len() + captured_env.len());
        for f in rec_fns {
            rec_env.push(Binding::Function(FunctionBinding {
                value: f.value,
                arity: f.arity,
                captures: captured_env.clone(),
            }));
        }
        rec_env.extend_from_slice(&captured_env);

        let mut body_env: Vec<Binding<'ctx>> = Vec::with_capacity(arity + rec_env.len());
        for i in (0..arity).rev() {
            let param = fn_val
                .get_nth_param(i as u32)
                .ok_or_else(|| CodegenError::Llvm(format!("lambda missing param {i}")))?
                .into_int_value();
            body_env.push(Binding::Value(param));
        }
        body_env.extend_from_slice(&rec_env);
        let v = self.compile_expr(body, &body_env, fn_val)?;
        self.builder
            .build_return(Some(&v))
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        if let Some(saved) = saved_block {
            self.builder.position_at_end(saved);
        }
        Ok(())
    }

    fn capture_env_from_params(
        &self,
        fn_val: FunctionValue<'ctx>,
        arity: usize,
        outer_env: &[Binding<'ctx>],
    ) -> Result<Vec<Binding<'ctx>>> {
        let mut param_index = arity as u32;
        let mut captured = Vec::with_capacity(outer_env.len());
        for binding in outer_env {
            match binding {
                Binding::Value(_) => {
                    let param = fn_val
                        .get_nth_param(param_index)
                        .ok_or_else(|| {
                            CodegenError::Llvm(format!(
                                "lambda missing captured int param {}",
                                param_index
                            ))
                        })?
                        .into_int_value();
                    captured.push(Binding::Value(param));
                    param_index += 1;
                }
                Binding::Ptr(_) => {
                    let param = fn_val
                        .get_nth_param(param_index)
                        .ok_or_else(|| {
                            CodegenError::Llvm(format!(
                                "lambda missing captured ptr param {}",
                                param_index
                            ))
                        })?
                        .into_pointer_value();
                    captured.push(Binding::Ptr(param));
                    param_index += 1;
                }
                Binding::Function(f) => captured.push(Binding::Function(f.clone())),
            }
        }
        Ok(captured)
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
                Node::PatInt { value } => {
                    let lit: i64 = value
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
                Node::PatCtor { name, sub_patterns } => {
                    // Numeric `pat-ctor` names are treated as integer literal arms
                    // for backward compatibility with pre-ADR-0037 canonical files.
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
