//! AST → LLVM IR via `inkwell`. Built only when an `llvm<N>-<M>` feature
//! is enabled; without it, this module is excluded from the crate.
//!
//! ## Lowering model
//!
//! - The whole input program is the body of an implicit `int main(void)`.
//! - The program's value, computed as `i64`, becomes `main`'s exit code
//!   (truncated to `i32` per C runtime convention; ADR 0025 § Phase 1
//!   libc set notes `return 0` is preferred over `exit(0)`).
//! - Known saturated closed `let`/anonymous `Lam` chains still hoist as
//!   direct-call LLVM functions under default C calling convention (ADR 0027).
//!   `rec` members lower as direct-call helpers; Phase 3 permits hidden
//!   parameters for captured runtime values and buffers (ADR 0059).
//! - First-class function values lower as Phase 4 closure pairs:
//!   `{ code: ptr, env: ptr }`. Closure entries take `(env, arg)` and return
//!   the statically known result type. Captures are minimized by free
//!   DeBruijn references and stored by value in compiler-managed environments
//!   (ADR 0073).
//! - Applying a function-typed expression emits an indirect closure-entry
//!   call unless an existing direct-call optimization applies.
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

use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;

use inkwell::attributes::{Attribute, AttributeLoc};
use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::{Linkage, Module};
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine,
};
use inkwell::types::{BasicMetadataTypeEnum, BasicType, BasicTypeEnum, FunctionType};
use inkwell::values::{
    BasicMetadataValueEnum, BasicValue, BasicValueEnum, FunctionValue, IntValue, PointerValue,
};
use inkwell::{AddressSpace, IntPredicate, OptimizationLevel};

/// `LLVMCCallConv` from `llvm-sys`. Not exposed as a typed enum by inkwell
/// 0.9, which takes a raw `u32` for `set_call_conventions`. ADR 0027 § 2.
const LLVM_C_CALL_CONV: u32 = 0;

use tacit_canonical::ast::Node;
use tacit_typecheck::primitives::{
    FixedArithMode, FixedArithOp, FixedBitOp, FixedCastKind, FixedEndian, FixedPrim, FixedShiftOp,
    U8VecBusOp, U8VecOp, VecOp, VecPrim,
};
use tacit_typecheck::ty::{FixedIntTy, IntSign, Subst, Ty};
use tacit_typecheck::type_from_node::type_from_node;

use crate::analysis::{
    check_closed, check_no_holes, collect_lam_chain, parse_int_literal, sanitize, unfold_app,
};
use crate::error::CodegenError;
use crate::primitives::{ArithOp, CmpOp, PrimKind};

type Result<T> = std::result::Result<T, CodegenError>;
const TEST_STEP_BUDGET_EXIT_CODE: u64 = 124;

#[derive(Debug, Clone, Copy, Default)]
pub struct CompileOptions {
    pub loop_step_budget: Option<u64>,
}

/// Top-level entry: build an `inkwell::Module` from a Tacit AST and emit it
/// to an object file at `out_path`. Returns the path the object was written to.
pub fn compile_to_object(node: &Node, module_name: &str, out_path: &Path) -> Result<()> {
    compile_to_object_with_options(node, module_name, out_path, CompileOptions::default())
}

pub fn compile_to_object_with_options(
    node: &Node,
    module_name: &str,
    out_path: &Path,
    options: CompileOptions,
) -> Result<()> {
    let context = Context::create();
    let mut compiler = Compiler::new_with_options(&context, module_name, options);
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
    /// Host-import dispatch functions, keyed by host_import_hash.
    /// When `Ref { hash }` is encountered during expression compilation and
    /// the hash matches, the registered direct-call function lowers as a
    /// callback-table indirect call into the host context.
    host_imports: std::collections::BTreeMap<String, FunctionBinding<'ctx>>,
    /// ABI-shaped host import dispatch functions. This preserves borrowed
    /// vector parameters, which are not first-class Tacit values and cannot be
    /// represented by `FunctionBinding::param_tys`.
    host_import_abis: std::collections::BTreeMap<String, HostImportBinding<'ctx>>,
    state_runtime: Option<StateRuntime<'ctx>>,
    options: CompileOptions,
}

#[derive(Clone)]
struct StateRuntime<'ctx> {
    instance_ty: inkwell::types::StructType<'ctx>,
    instance_tls: inkwell::values::GlobalValue<'ctx>,
    status_tls: inkwell::values::GlobalValue<'ctx>,
    fields: Vec<StateFieldLayout<'ctx>>,
}

#[derive(Clone)]
struct StateFieldLayout<'ctx> {
    name: String,
    ty: StateFieldTy<'ctx>,
}

#[derive(Clone)]
enum StateFieldTy<'ctx> {
    Scalar(LibScalar),
    Record {
        struct_ty: inkwell::types::StructType<'ctx>,
        fields: Vec<StateFieldLayout<'ctx>>,
    },
    Vec {
        elem: FixedIntTy,
        slot_ty: inkwell::types::StructType<'ctx>,
    },
}

/// Per-binder entry on the binding stack. Innermost binder is `last`.
#[derive(Clone)]
enum Binding<'ctx> {
    /// A computed first-class scalar or product value bound by `let` or a
    /// `lam` parameter.
    Value(CompiledValue<'ctx>),
    /// A direct-call function reference. Saturated calls use `value`; value
    /// position or partial application reifies `closure_template` when present.
    Function(FunctionBinding<'ctx>),
    /// A stack-allocated buffer-like handle. Handles are valid only as
    /// primitive arguments for their own family; they are not first-class values.
    Ptr {
        ptr: PointerValue<'ctx>,
        kind: PtrKind,
    },
    /// A Stage 7 typed mutable vector handle (ADR 0085): pointer to the
    /// element-typed storage plus the element count carried in the handle.
    /// Anti-escape; not a first-class value.
    VecHandle {
        ptr: PointerValue<'ctx>,
        len: IntValue<'ctx>,
        ty: FixedIntTy,
    },
    /// Placeholder for an outer binding that was not captured into a closure
    /// environment. A correct free-variable analysis means this is never read.
    Unavailable,
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum ValueTy {
    /// Runtime `i64`. Tacit `Bool` is also represented as `i64` 0/1 in the
    /// existing codegen surface.
    Int,
    /// First-class function value. Effects remain typechecker facts; codegen
    /// only needs the argument and result ABI shape for closure entries.
    Fn(Box<ValueTy>, Box<ValueTy>),
    /// Canonical sorted field order. Field names remain source-level type
    /// information; LLVM layout only sees the field value types.
    Record(Vec<(String, ValueTy)>),
}

impl fmt::Display for ValueTy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueTy::Int => write!(f, "Int"),
            ValueTy::Fn(arg, ret) => {
                let parens = matches!(arg.as_ref(), ValueTy::Fn(_, _));
                if parens {
                    write!(f, "({}) -> {}", arg, ret)
                } else {
                    write!(f, "{} -> {}", arg, ret)
                }
            }
            ValueTy::Record(fields) => {
                write!(f, "{{")?;
                for (i, (name, ty)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", name, ty)?;
                }
                write!(f, "}}")
            }
        }
    }
}

/// A typed-vector-handle parameter kind. A handle is a *call-local borrow*
/// (ADR 0098): it may travel *down* the call tree as a direct-call argument,
/// but it may not be returned, stored in a record, or captured by an escaping
/// closure. Handles have no first-class `ValueTy` representation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum HandleTy {
    /// Writable byte buffer handle (`Buf`); a single machine word.
    Buf,
    /// Opaque `i64` vector handle (`I64Vec`); a single machine word.
    I64Vec,
    /// Length-carrying typed mutable vector handle; a two-word `(ptr, len)`
    /// pair, the same representation as a `rec` hidden capture (ADR 0085).
    Vec(FixedIntTy),
}

impl fmt::Display for HandleTy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HandleTy::Buf => write!(f, "Buf"),
            HandleTy::I64Vec => write!(f, "I64Vec"),
            HandleTy::Vec(elem) => write!(f, "{}vec", elem),
        }
    }
}

/// A direct-call function parameter: either a first-class value, or a
/// call-local handle borrow (ADR 0098). Handles are not `ValueTy`s, so a
/// parameter list is a `ParamTy` list rather than a `ValueTy` list. A handle
/// parameter is only ever valid on a direct-call function; a function that
/// declares one cannot be reified into a first-class closure value.
#[derive(Clone, PartialEq, Eq, Debug)]
enum ParamTy {
    Value(ValueTy),
    Handle(HandleTy),
}

impl ParamTy {
    fn as_value(&self) -> Option<&ValueTy> {
        match self {
            ParamTy::Value(ty) => Some(ty),
            ParamTy::Handle(_) => None,
        }
    }
}

impl fmt::Display for ParamTy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParamTy::Value(ty) => write!(f, "{}", ty),
            ParamTy::Handle(handle) => write!(f, "{}", handle),
        }
    }
}

#[derive(Clone)]
struct CompiledValue<'ctx> {
    ty: ValueTy,
    value: BasicValueEnum<'ctx>,
}

impl<'ctx> CompiledValue<'ctx> {
    fn int(value: IntValue<'ctx>) -> Self {
        Self {
            ty: ValueTy::Int,
            value: value.into(),
        }
    }

    fn into_int(self) -> Result<IntValue<'ctx>> {
        if self.ty != ValueTy::Int {
            return Err(CodegenError::ExpectedIntValue {
                actual: self.ty.to_string(),
            });
        }
        Ok(self.value.into_int_value())
    }
}

/// A compiled call argument for a direct-call function: either a first-class
/// value, or a call-local handle borrow lowered to its `(ptr[, len])` words.
/// There is one internal handle calling convention (ADR 0098): a handle
/// argument, a handle parameter, and a `rec` hidden handle capture all lower
/// to the same words.
enum CompiledArg<'ctx> {
    Value(CompiledValue<'ctx>),
    Handle {
        ptr: PointerValue<'ctx>,
        /// `Some` for typed vectors (which carry an explicit length), `None`
        /// for `Buf` / `I64Vec`.
        len: Option<IntValue<'ctx>>,
        ty: HandleTy,
    },
}

impl<'ctx> CompiledArg<'ctx> {
    /// The parameter type this argument satisfies.
    fn param_ty(&self) -> ParamTy {
        match self {
            CompiledArg::Value(v) => ParamTy::Value(v.ty.clone()),
            CompiledArg::Handle { ty, .. } => ParamTy::Handle(*ty),
        }
    }

    /// Append this argument's LLVM call word(s) to `out`.
    fn push_call_words(&self, out: &mut Vec<BasicMetadataValueEnum<'ctx>>) {
        match self {
            CompiledArg::Value(v) => out.push(v.value.into()),
            CompiledArg::Handle { ptr, len, .. } => {
                out.push(BasicMetadataValueEnum::PointerValue(*ptr));
                if let Some(len) = len {
                    out.push(BasicMetadataValueEnum::IntValue(*len));
                }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PtrKind {
    Buf,
    I64Vec,
}

#[derive(Clone)]
enum BindingTy {
    Value(ValueTy),
    Function {
        param_tys: Vec<ValueTy>,
        ret_ty: ValueTy,
    },
    Ptr,
}

/// Direction for ASCII case shift primitives (ADR 0068).
#[derive(Clone, Copy, PartialEq, Eq)]
enum AsciiCase {
    Lower,
    Upper,
}

#[derive(Clone)]
struct FunctionBinding<'ctx> {
    value: FunctionValue<'ctx>,
    param_tys: Vec<ParamTy>,
    ret_ty: ValueTy,
    captures: Vec<Binding<'ctx>>,
    closure_template: Option<ClosureTemplate<'ctx>>,
}

impl<'ctx> FunctionBinding<'ctx> {
    fn arity(&self) -> usize {
        self.param_tys.len()
    }

    /// Whether any declared parameter is a call-local handle (ADR 0098).
    /// Such a function is direct-call only — it cannot be reified into a
    /// first-class closure value.
    fn has_handle_param(&self) -> bool {
        self.param_tys
            .iter()
            .any(|p| matches!(p, ParamTy::Handle(_)))
    }

    /// The parameter list as `ValueTy`s, or `None` if any parameter is a
    /// handle. Used by closure-conversion paths that only accept first-class
    /// parameters.
    fn value_param_tys(&self) -> Option<Vec<ValueTy>> {
        self.param_tys
            .iter()
            .map(|p| p.as_value().cloned())
            .collect()
    }
}

#[derive(Clone)]
struct HostImportBinding<'ctx> {
    value: FunctionValue<'ctx>,
    params: Vec<LibAbiType>,
    result: LibAbiType,
}

#[derive(Clone)]
struct ClosureTemplate<'ctx> {
    expr: Box<Node>,
    ty: ValueTy,
    env: Vec<Binding<'ctx>>,
}

struct CaptureValue<'ctx> {
    outer_index: usize,
    value: CompiledValue<'ctx>,
}

struct LamSpec<'a> {
    param_tys: Vec<ParamTy>,
    ret_ty: ValueTy,
    body: &'a Node,
}

impl<'ctx> Compiler<'ctx> {
    pub fn new(context: &'ctx Context, module_name: &str) -> Self {
        Self::new_with_options(context, module_name, CompileOptions::default())
    }

    pub fn new_with_options(
        context: &'ctx Context,
        module_name: &str,
        options: CompileOptions,
    ) -> Self {
        let module = context.create_module(module_name);
        let builder = context.create_builder();
        Compiler {
            context,
            module,
            builder,
            next_fn_id: 0,
            host_imports: std::collections::BTreeMap::new(),
            host_import_abis: std::collections::BTreeMap::new(),
            state_runtime: None,
            options,
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

    /// Compile an expression that must lower to the runtime integer
    /// representation (`i64`).
    fn compile_expr(
        &mut self,
        node: &Node,
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        self.compile_value_expr(node, env, cur_fn)?.into_int()
    }

    /// Compile an expression to a first-class runtime value. Phase 4 Stage 2
    /// supports integers and structural records of supported first-class
    /// values; buffer-like handles remain separate scoped bindings.
    fn compile_value_expr(
        &mut self,
        node: &Node,
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<CompiledValue<'ctx>> {
        match node {
            Node::Int { value } => Ok(CompiledValue::int(self.compile_int_literal(value)?)),
            Node::Str { .. } => Err(CodegenError::Unsupported(
                "string literal outside of @write/@read primitive call",
            )),
            Node::Var { index } => {
                let entry = lookup_var(env, *index)?;
                match entry {
                    Binding::Value(v) => Ok(v.clone()),
                    Binding::Function(f) => self.reify_function_binding(f, cur_fn),
                    Binding::Ptr { .. } => Err(CodegenError::Unsupported(
                        "buffer-like handle used in integer-value position",
                    )),
                    Binding::VecHandle { .. } => Err(CodegenError::Unsupported(
                        "typed vector handle used in integer-value position",
                    )),
                    Binding::Unavailable => Err(CodegenError::UnavailableCapture { index: *index }),
                }
            }
            Node::Let { rhs, body } => self.compile_let(rhs, body, env, cur_fn),
            Node::If { cond, then, else_ } => self.compile_if(cond, then, else_, env, cur_fn),
            Node::App { .. } => self.compile_app(node, env, cur_fn),
            Node::Lam { .. } => self.compile_closure_value(node, None, env, cur_fn),
            Node::Rec { bindings, body } => self.compile_rec(bindings, body, env, cur_fn),
            Node::Module { .. } => Err(CodegenError::Unsupported("module binding group")),
            Node::Unit { .. } => Err(CodegenError::Unsupported("logical unit artifact")),
            Node::Match { scrutinee, arms } => self.compile_match(scrutinee, arms, env, cur_fn),
            Node::Arm { .. } => Err(CodegenError::Unsupported("bare arm outside match")),
            Node::Record { fields } => self.compile_record(fields, env, cur_fn),
            Node::Proj { record, field } => self.compile_proj(record, field, env, cur_fn),
            Node::Ctor { .. } => Err(CodegenError::Unsupported("ctor in expression position")),
            Node::Ann { expr, type_ } => {
                if matches!(expr.as_ref(), Node::Lam { .. }) {
                    let expected = value_ty_from_ty(&ty_from_type_node(type_)?)?;
                    self.compile_closure_value(expr, Some(expected), env, cur_fn)
                } else {
                    self.compile_value_expr(expr, env, cur_fn)
                }
            }
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
            Node::Ref { hash } => {
                if let Some(binding) = self.host_imports.get(hash).cloned() {
                    return self.reify_function_binding(&binding, cur_fn);
                }
                Err(CodegenError::Unsupported(
                    "unit artifact node in value position",
                ))
            }
            Node::Imports { .. }
            | Node::Import { .. }
            | Node::HostImport { .. }
            | Node::Exports { .. }
            | Node::Export { .. }
            | Node::Defs { .. }
            | Node::Def { .. }
            | Node::State { .. }
            | Node::Sig { .. } => Err(CodegenError::Unsupported(
                "unit artifact node in value position",
            )),
        }
    }

    fn compile_int_literal(&self, decimal: &str) -> Result<IntValue<'ctx>> {
        let parsed = parse_int_literal(decimal)?;
        let i64_t = self.context.i64_type();
        // sign_extend = true so negatives lower as themselves.
        Ok(i64_t.const_int(parsed as u64, true))
    }

    fn compile_record(
        &mut self,
        fields: &[(String, Node)],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<CompiledValue<'ctx>> {
        let mut ordered: Vec<&(String, Node)> = fields.iter().collect();
        ordered.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));

        let mut compiled_fields = Vec::with_capacity(ordered.len());
        let mut field_tys = Vec::with_capacity(ordered.len());
        for (name, expr) in ordered {
            let value = self.compile_value_expr(expr, env, cur_fn)?;
            field_tys.push((name.clone(), value.ty.clone()));
            compiled_fields.push(value);
        }

        let ty = ValueTy::Record(field_tys);
        let struct_ty = self.llvm_struct_type(&ty)?;
        let mut aggregate = struct_ty.get_undef();
        for (i, value) in compiled_fields.into_iter().enumerate() {
            aggregate = self
                .builder
                .build_insert_value(aggregate, value.value, i as u32, "record_insert")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?
                .into_struct_value();
        }
        Ok(CompiledValue {
            ty,
            value: aggregate.into(),
        })
    }

    fn compile_proj(
        &mut self,
        record: &Node,
        field: &str,
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<CompiledValue<'ctx>> {
        let record_value = self.compile_value_expr(record, env, cur_fn)?;
        let ValueTy::Record(fields) = &record_value.ty else {
            return Err(CodegenError::InvalidProjection {
                field: field.to_string(),
                actual: record_value.ty.to_string(),
            });
        };
        let Some((index, (_, field_ty))) = fields
            .iter()
            .enumerate()
            .find(|(_, (name, _))| name == field)
        else {
            return Err(CodegenError::MissingField {
                field: field.to_string(),
            });
        };
        let extracted = self
            .builder
            .build_extract_value(record_value.value.into_struct_value(), index as u32, "proj")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        Ok(CompiledValue {
            ty: field_ty.clone(),
            value: extracted,
        })
    }

    fn llvm_type(&self, ty: &ValueTy) -> Result<BasicTypeEnum<'ctx>> {
        match ty {
            ValueTy::Int => Ok(self.context.i64_type().into()),
            ValueTy::Fn(_, _) => Ok(self.closure_type().into()),
            ValueTy::Record(_) => Ok(self.llvm_struct_type(ty)?.into()),
        }
    }

    fn closure_type(&self) -> inkwell::types::StructType<'ctx> {
        let ptr_t = self.context.ptr_type(AddressSpace::default());
        self.context
            .struct_type(&[ptr_t.into(), ptr_t.into()], false)
    }

    fn closure_entry_type(&self, fn_ty: &ValueTy) -> Result<FunctionType<'ctx>> {
        let ValueTy::Fn(arg_ty, ret_ty) = fn_ty else {
            return Err(CodegenError::AppNonFunction);
        };
        let ptr_t = self.context.ptr_type(AddressSpace::default());
        let params = &[
            BasicMetadataTypeEnum::PointerType(ptr_t),
            self.llvm_type(arg_ty)?.into(),
        ];
        Ok(self.llvm_type(ret_ty)?.fn_type(params, false))
    }

    fn build_closure_pair(
        &mut self,
        code_ptr: PointerValue<'ctx>,
        env_ptr: PointerValue<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>> {
        let mut closure = self.closure_type().get_undef();
        closure = self
            .builder
            .build_insert_value(closure, code_ptr, 0, "closure_code")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_struct_value();
        closure = self
            .builder
            .build_insert_value(closure, env_ptr, 1, "closure_env")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_struct_value();
        Ok(closure.into())
    }

    fn llvm_struct_type(&self, ty: &ValueTy) -> Result<inkwell::types::StructType<'ctx>> {
        let ValueTy::Record(fields) = ty else {
            return Err(CodegenError::UnsupportedValueType { ty: ty.to_string() });
        };
        let field_types = fields
            .iter()
            .map(|(_, field_ty)| self.llvm_type(field_ty))
            .collect::<Result<Vec<_>>>()?;
        Ok(self.context.struct_type(&field_types, false))
    }

    fn signature_for_lam(
        &self,
        lam_body: &Node,
        arity: usize,
        ann_ty: Option<&Node>,
        supplied_param_tys: &[ParamTy],
        outer_env_tys: &[BindingTy],
    ) -> Result<(Vec<ParamTy>, ValueTy)> {
        if let Some(type_node) = ann_ty {
            let (param_tys, ret_ty) = function_signature_from_type_node(type_node)?;
            if param_tys.len() != arity {
                return Err(CodegenError::FunctionArity {
                    expected: param_tys.len(),
                    got: arity,
                });
            }
            return Ok((param_tys, ret_ty));
        }

        let param_tys = if supplied_param_tys.len() == arity {
            supplied_param_tys.to_vec()
        } else {
            vec![ParamTy::Value(ValueTy::Int); arity]
        };
        // A handle parameter has no first-class type; in the body-type
        // environment it stands as a non-escapable `Ptr` so that any attempt
        // to use it as a value fails the same way an `@<ty>vec-alloc` binding
        // would (ADR 0098).
        let mut body_env: Vec<BindingTy> = param_tys
            .iter()
            .rev()
            .map(|p| match p {
                ParamTy::Value(ty) => BindingTy::Value(ty.clone()),
                ParamTy::Handle(_) => BindingTy::Ptr,
            })
            .collect();
        body_env.extend_from_slice(outer_env_tys);
        let ret_ty = self
            .infer_value_ty(lam_body, &body_env)
            .unwrap_or(ValueTy::Int);
        Ok((param_tys, ret_ty))
    }

    fn compile_let(
        &mut self,
        rhs: &Node,
        body: &Node,
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<CompiledValue<'ctx>> {
        // Special-case Let-of-Lam-chain: hoist the closed lambda chain to a
        // top-level function and bind the body under a `Function` entry.
        // ADR 0026 § 3 second bullet; Phase 3 permits direct multi-arg calls
        // when all arguments are supplied at the call site.
        if let Some((arity, lam_body, ann_ty)) = collect_annotated_lam_chain(rhs) {
            let env_tys = binding_tys_from_env(env);
            let (param_tys, ret_ty) =
                self.signature_for_lam(lam_body, arity, ann_ty, &[], &env_tys)?;
            if check_closed(lam_body, arity as u64).is_ok() {
                // A handle-parameter helper has no first-class function type,
                // so it carries no closure template; reifying it later fails
                // cleanly (ADR 0098).
                let template =
                    try_nested_fn_ty(&param_tys, ret_ty.clone()).map(|fn_ty| (rhs, fn_ty));
                let fn_val = self.hoist_lambda(lam_body, param_tys, ret_ty, "let", template)?;
                let mut new_env = env.to_vec();
                new_env.insert(0, Binding::Function(fn_val));
                return self.compile_value_expr(body, &new_env, cur_fn);
            }

            // A non-closed `let` lambda becomes a first-class closure value;
            // a handle parameter is forbidden there (ADR 0098 — handles do
            // not escape into closure values).
            let closure_ty = try_nested_fn_ty(&param_tys, ret_ty).ok_or(
                CodegenError::Unsupported("handle-typed parameter on a non-direct-call function"),
            )?;
            let closure = self.compile_closure_value(rhs, Some(closure_ty), env, cur_fn)?;
            let mut new_env = env.to_vec();
            new_env.insert(0, Binding::Value(closure));
            return self.compile_value_expr(body, &new_env, cur_fn);
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
                        new_env.insert(
                            0,
                            Binding::Ptr {
                                ptr: buf_ptr,
                                kind: PtrKind::Buf,
                            },
                        );
                        return self.compile_value_expr(body, &new_env, cur_fn);
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
                    new_env.insert(
                        0,
                        Binding::Ptr {
                            ptr: buf_ptr,
                            kind: PtrKind::Buf,
                        },
                    );
                    return self.compile_value_expr(body, &new_env, cur_fn);
                }
                if name == "i64-alloc" && args.len() == 1 {
                    // Runtime-sized stack allocation: alloca i64, %n (ADR 0061).
                    let count_val = self.compile_expr(args[0], env, cur_fn)?;
                    let i64_t = self.context.i64_type();
                    let vec_ptr = self
                        .builder
                        .build_array_alloca(i64_t, count_val, "i64_vec")
                        .map_err(|e| CodegenError::Llvm(e.to_string()))?;
                    let mut new_env = env.to_vec();
                    new_env.insert(
                        0,
                        Binding::Ptr {
                            ptr: vec_ptr,
                            kind: PtrKind::I64Vec,
                        },
                    );
                    return self.compile_value_expr(body, &new_env, cur_fn);
                }

                // Stage 7 typed vec alloc (ADR 0085): `let v = @<ty>vec-alloc n in ...`.
                if args.len() == 1 {
                    if let Some(VecPrim::Vec {
                        ty,
                        op: VecOp::Alloc,
                    }) = tacit_typecheck::primitives::parse_vec_prim(name)
                    {
                        let count_val = self.compile_expr(args[0], env, cur_fn)?;
                        let elem_ty = self.llvm_int_type_for_width(ty.width);
                        let ptr = self
                            .builder
                            .build_array_alloca(elem_ty, count_val, "vec_alloc")
                            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
                        let mut new_env = env.to_vec();
                        new_env.insert(
                            0,
                            Binding::VecHandle {
                                ptr,
                                len: count_val,
                                ty,
                            },
                        );
                        return self.compile_value_expr(body, &new_env, cur_fn);
                    }
                }

                // Stage 7 u8vec slice (ADR 0085):
                // `let s = @u8vec-slice v off len in ...` binds a sub-view.
                if name == "u8vec-slice" && args.len() == 3 {
                    let u8_ty = FixedIntTy::new(IntSign::Unsigned, 8);
                    let (parent_ptr, parent_len) = self.resolve_vec_arg(args[0], env, u8_ty)?;
                    let off = self.compile_expr(args[1], env, cur_fn)?;
                    let slice_len = self.compile_expr(args[2], env, cur_fn)?;
                    self.check_range(cur_fn, off, slice_len, parent_len, "u8vec_slice")?;
                    let slice_ptr = self.ptr_at(parent_ptr, off, "u8vec_slice_ptr")?;
                    let mut new_env = env.to_vec();
                    new_env.insert(
                        0,
                        Binding::VecHandle {
                            ptr: slice_ptr,
                            len: slice_len,
                            ty: u8_ty,
                        },
                    );
                    return self.compile_value_expr(body, &new_env, cur_fn);
                }

                if name == "state-load" && args.len() == 1 {
                    let runtime = self.state_runtime()?;
                    let field_name = Self::state_field_name_arg(args[0])?;
                    let (field_ptr, field) = self.state_field_ptr(&runtime, field_name)?;
                    if let StateFieldTy::Vec { elem, slot_ty } = field.ty {
                        let data_pp = self
                            .builder
                            .build_struct_gep(slot_ty, field_ptr, 0, "state_load_vec_data_pp")
                            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
                        let len_pp = self
                            .builder
                            .build_struct_gep(slot_ty, field_ptr, 1, "state_load_vec_len_pp")
                            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
                        let ptr = self
                            .builder
                            .build_load(
                                self.context.ptr_type(AddressSpace::default()),
                                data_pp,
                                "state_load_vec_data",
                            )
                            .map_err(|e| CodegenError::Llvm(e.to_string()))?
                            .into_pointer_value();
                        let len = self
                            .builder
                            .build_load(self.context.i64_type(), len_pp, "state_load_vec_len")
                            .map_err(|e| CodegenError::Llvm(e.to_string()))?
                            .into_int_value();
                        let mut new_env = env.to_vec();
                        new_env.insert(0, Binding::VecHandle { ptr, len, ty: elem });
                        return self.compile_value_expr(body, &new_env, cur_fn);
                    }
                }

                if name == "state-slice" && args.len() == 3 {
                    let runtime = self.state_runtime()?;
                    let field_name = Self::state_field_name_arg(args[0])?;
                    let (field_ptr, field) = self.state_field_ptr(&runtime, field_name)?;
                    let StateFieldTy::Vec { elem, slot_ty } = field.ty else {
                        return Err(CodegenError::Unsupported(
                            "@state-slice requires vector field",
                        ));
                    };
                    let u8_ty = FixedIntTy::new(IntSign::Unsigned, 8);
                    if elem != u8_ty {
                        return Err(CodegenError::Unsupported(
                            "@state-slice requires u8vec field",
                        ));
                    }
                    let data_pp = self
                        .builder
                        .build_struct_gep(slot_ty, field_ptr, 0, "state_slice_data_pp")
                        .map_err(|e| CodegenError::Llvm(e.to_string()))?;
                    let len_pp = self
                        .builder
                        .build_struct_gep(slot_ty, field_ptr, 1, "state_slice_len_pp")
                        .map_err(|e| CodegenError::Llvm(e.to_string()))?;
                    let parent_ptr = self
                        .builder
                        .build_load(
                            self.context.ptr_type(AddressSpace::default()),
                            data_pp,
                            "state_slice_data",
                        )
                        .map_err(|e| CodegenError::Llvm(e.to_string()))?
                        .into_pointer_value();
                    let parent_len = self
                        .builder
                        .build_load(self.context.i64_type(), len_pp, "state_slice_len")
                        .map_err(|e| CodegenError::Llvm(e.to_string()))?
                        .into_int_value();
                    let off = self.compile_expr(args[1], env, cur_fn)?;
                    let slice_len = self.compile_expr(args[2], env, cur_fn)?;
                    self.check_range(cur_fn, off, slice_len, parent_len, "state_slice")?;
                    let slice_ptr = self.ptr_at(parent_ptr, off, "state_slice_ptr")?;
                    let mut new_env = env.to_vec();
                    new_env.insert(
                        0,
                        Binding::VecHandle {
                            ptr: slice_ptr,
                            len: slice_len,
                            ty: u8_ty,
                        },
                    );
                    return self.compile_value_expr(body, &new_env, cur_fn);
                }
            }
        }

        let v = self.compile_value_expr(rhs, env, cur_fn)?;
        let mut new_env = env.to_vec();
        new_env.insert(0, Binding::Value(v));
        self.compile_value_expr(body, &new_env, cur_fn)
    }

    fn compile_if(
        &mut self,
        cond: &Node,
        then_node: &Node,
        else_node: &Node,
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<CompiledValue<'ctx>> {
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
        let then_val = self.compile_value_expr(then_node, env, cur_fn)?;
        let then_end_bb = self.builder.get_insert_block().unwrap();
        self.builder
            .build_unconditional_branch(merge_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(else_bb);
        let else_val = self.compile_value_expr(else_node, env, cur_fn)?;
        if then_val.ty != else_val.ty {
            return Err(CodegenError::ValueTypeMismatch {
                expected: then_val.ty.to_string(),
                actual: else_val.ty.to_string(),
            });
        }
        let else_end_bb = self.builder.get_insert_block().unwrap();
        self.builder
            .build_unconditional_branch(merge_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(merge_bb);
        let phi = self
            .builder
            .build_phi(self.llvm_type(&then_val.ty)?, "ifval")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        phi.add_incoming(&[
            (&then_val.value as &dyn BasicValue<'ctx>, then_end_bb),
            (&else_val.value as &dyn BasicValue<'ctx>, else_end_bb),
        ]);
        Ok(CompiledValue {
            ty: then_val.ty,
            value: phi.as_basic_value(),
        })
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
    ) -> Result<CompiledValue<'ctx>> {
        let (head, args) = unfold_app(node);

        match head {
            Node::Sym { name } => self.compile_primitive_call(name, &args, env, cur_fn),
            Node::Ref { hash } => {
                if let Some(binding) = self.host_import_abis.get(hash).cloned() {
                    if args.len() == binding.params.len() {
                        return self.call_host_import_direct(&binding, &args, env, cur_fn);
                    }
                }
                if let Some(binding) = self.host_imports.get(hash).cloned() {
                    let arg_vals = self.compile_call_args(&args, env, cur_fn)?;
                    if args.len() == binding.arity() {
                        let call_args: Vec<CompiledArg<'ctx>> =
                            arg_vals.into_iter().map(CompiledArg::Value).collect();
                        self.call_function(&binding, &call_args)
                    } else {
                        let closure = self.reify_function_binding(&binding, cur_fn)?;
                        self.call_closure_spine(closure, &arg_vals)
                    }
                } else {
                    Err(CodegenError::Unsupported(
                        "ref to unknown definition in app head",
                    ))
                }
            }
            Node::Lam { .. } | Node::Ann { .. } => {
                let (arity, lam_body, ann_ty) =
                    collect_annotated_lam_chain(head).ok_or(CodegenError::AppNonFunction)?;
                if args.len() == arity && check_closed(lam_body, arity as u64).is_ok() {
                    // Saturated direct call: the inlined `App(Lam, ...)` form
                    // `package_library` produces (ADR 0098). Arguments may
                    // include call-local handle borrows.
                    let call_args = self.compile_call_args_mixed(&args, env, cur_fn)?;
                    let supplied: Vec<ParamTy> = call_args.iter().map(|a| a.param_ty()).collect();
                    let (param_tys, ret_ty) =
                        self.signature_for_lam(lam_body, arity, ann_ty, &supplied, &[])?;
                    let fn_val = self.hoist_lambda(lam_body, param_tys, ret_ty, "anon", None)?;
                    return self.call_function(&fn_val, &call_args);
                }
                // Non-saturated / non-closed: a first-class closure value.
                // Handle arguments are forbidden here (ADR 0098); a handle
                // argument fails `compile_call_args` as a non-value.
                let arg_vals = self.compile_call_args(&args, env, cur_fn)?;
                let arg_tys: Vec<ParamTy> = arg_vals
                    .iter()
                    .map(|v| ParamTy::Value(v.ty.clone()))
                    .collect();
                let (param_tys, ret_ty) = self.signature_for_lam(
                    lam_body,
                    arity,
                    ann_ty,
                    &arg_tys,
                    &binding_tys_from_env(env),
                )?;
                let closure_ty =
                    try_nested_fn_ty(&param_tys, ret_ty).ok_or(CodegenError::Unsupported(
                        "handle-typed parameter on a non-direct-call function",
                    ))?;
                let closure = self.compile_closure_value(head, Some(closure_ty), env, cur_fn)?;
                self.call_closure_spine(closure, &arg_vals)
            }
            Node::Var { index } => {
                let binding = lookup_var(env, *index)?;
                match binding {
                    Binding::Function(fn_val) => {
                        if args.len() == fn_val.arity() {
                            let call_args = self.compile_call_args_mixed(&args, env, cur_fn)?;
                            self.call_function(fn_val, &call_args)
                        } else {
                            // Partial application reifies the function as a
                            // closure value; a handle-parameter function
                            // cannot be reified (ADR 0098).
                            let arg_vals = self.compile_call_args(&args, env, cur_fn)?;
                            let closure = self.reify_function_binding(fn_val, cur_fn)?;
                            self.call_closure_spine(closure, &arg_vals)
                        }
                    }
                    Binding::Value(v) => {
                        let closure = v.clone();
                        let arg_vals = self.compile_call_args(&args, env, cur_fn)?;
                        self.call_closure_spine(closure, &arg_vals)
                    }
                    Binding::Ptr { .. } | Binding::VecHandle { .. } | Binding::Unavailable => {
                        Err(CodegenError::AppNonFunction)
                    }
                }
            }
            _ => {
                let closure = self.compile_value_expr(head, env, cur_fn)?;
                let arg_vals = self.compile_call_args(&args, env, cur_fn)?;
                self.call_closure_spine(closure, &arg_vals)
            }
        }
    }

    fn compile_call_args(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<Vec<CompiledValue<'ctx>>> {
        let mut values = Vec::with_capacity(args.len());
        for arg in args {
            values.push(self.compile_value_expr(arg, env, cur_fn)?);
        }
        Ok(values)
    }

    /// Compile direct-call arguments, classifying each as a first-class value
    /// or a call-local handle borrow (ADR 0098).
    fn compile_call_args_mixed(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<Vec<CompiledArg<'ctx>>> {
        let mut out = Vec::with_capacity(args.len());
        for arg in args {
            out.push(self.compile_call_arg(arg, env, cur_fn)?);
        }
        Ok(out)
    }

    fn compile_call_arg(
        &mut self,
        arg: &Node,
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<CompiledArg<'ctx>> {
        // A handle has no first-class value, so the only expression that can
        // denote one is a `Var` resolving to a handle binding — the same
        // anti-escape rule `resolve_vec_arg` enforces for vec primitives.
        if let Node::Var { index } = arg {
            match lookup_var(env, *index)? {
                Binding::Ptr { ptr, kind } => {
                    let ty = match kind {
                        PtrKind::Buf => HandleTy::Buf,
                        PtrKind::I64Vec => HandleTy::I64Vec,
                    };
                    return Ok(CompiledArg::Handle {
                        ptr: *ptr,
                        len: None,
                        ty,
                    });
                }
                Binding::VecHandle { ptr, len, ty } => {
                    return Ok(CompiledArg::Handle {
                        ptr: *ptr,
                        len: Some(*len),
                        ty: HandleTy::Vec(*ty),
                    });
                }
                _ => {}
            }
        }
        Ok(CompiledArg::Value(
            self.compile_value_expr(arg, env, cur_fn)?,
        ))
    }

    fn call_function(
        &mut self,
        fn_binding: &FunctionBinding<'ctx>,
        args: &[CompiledArg<'ctx>],
    ) -> Result<CompiledValue<'ctx>> {
        if args.len() != fn_binding.param_tys.len() {
            return Err(CodegenError::FunctionArity {
                expected: fn_binding.param_tys.len(),
                got: args.len(),
            });
        }
        for (arg, expected) in args.iter().zip(&fn_binding.param_tys) {
            let actual = arg.param_ty();
            if &actual != expected {
                return Err(CodegenError::ValueTypeMismatch {
                    expected: expected.to_string(),
                    actual: actual.to_string(),
                });
            }
        }

        let mut call_args: Vec<BasicMetadataValueEnum<'ctx>> = Vec::with_capacity(args.len());
        for arg in args {
            arg.push_call_words(&mut call_args);
        }
        for capture in &fn_binding.captures {
            match capture {
                Binding::Value(v) => call_args.push(v.value.into()),
                Binding::Ptr { ptr, .. } => {
                    call_args.push(BasicMetadataValueEnum::PointerValue(*ptr))
                }
                Binding::VecHandle { ptr, len, .. } => {
                    call_args.push(BasicMetadataValueEnum::PointerValue(*ptr));
                    call_args.push(BasicMetadataValueEnum::IntValue(*len));
                }
                Binding::Function(_) | Binding::Unavailable => {}
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
        Ok(CompiledValue {
            ty: fn_binding.ret_ty.clone(),
            value: ret,
        })
    }

    fn call_host_import_direct(
        &mut self,
        binding: &HostImportBinding<'ctx>,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<CompiledValue<'ctx>> {
        if args.len() != binding.params.len() {
            return Err(CodegenError::FunctionArity {
                expected: binding.params.len(),
                got: args.len(),
            });
        }

        let mut call_args = Vec::new();
        for (arg, abi_ty) in args.iter().zip(&binding.params) {
            match abi_ty {
                LibAbiType::BorrowedVector(vec_ty) => {
                    let (ptr, len) = self.resolve_vec_arg(arg, env, *vec_ty)?;
                    call_args.push(BasicMetadataValueEnum::PointerValue(ptr));
                    call_args.push(BasicMetadataValueEnum::IntValue(len));
                }
                _ => {
                    let value = self.compile_value_expr(arg, env, cur_fn)?;
                    let expected = Self::internal_value_ty_for_abi(abi_ty)?;
                    if value.ty != expected {
                        return Err(CodegenError::ValueTypeMismatch {
                            expected: expected.to_string(),
                            actual: value.ty.to_string(),
                        });
                    }
                    call_args.push(value.value.into());
                }
            }
        }

        let call = self
            .builder
            .build_call(binding.value, &call_args, "host_import")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let ret = call
            .try_as_basic_value()
            .basic()
            .ok_or_else(|| CodegenError::Llvm("host import returned no value".into()))?;
        Ok(CompiledValue {
            ty: Self::internal_value_ty_for_abi(&binding.result)?,
            value: ret,
        })
    }

    fn call_closure_spine(
        &mut self,
        mut callee: CompiledValue<'ctx>,
        args: &[CompiledValue<'ctx>],
    ) -> Result<CompiledValue<'ctx>> {
        for arg in args {
            callee = self.call_closure_value(callee, arg)?;
        }
        Ok(callee)
    }

    fn call_closure_value(
        &mut self,
        callee: CompiledValue<'ctx>,
        arg: &CompiledValue<'ctx>,
    ) -> Result<CompiledValue<'ctx>> {
        let ValueTy::Fn(param_ty, ret_ty) = &callee.ty else {
            return Err(CodegenError::AppNonFunction);
        };
        if param_ty.as_ref() != &arg.ty {
            return Err(CodegenError::ValueTypeMismatch {
                expected: param_ty.to_string(),
                actual: arg.ty.to_string(),
            });
        }

        let closure = callee.value.into_struct_value();
        let code_ptr = self
            .builder
            .build_extract_value(closure, 0, "closure_code")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_pointer_value();
        let env_ptr = self
            .builder
            .build_extract_value(closure, 1, "closure_env")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_pointer_value();
        let fn_type = self.closure_entry_type(&callee.ty)?;
        let call = self
            .builder
            .build_indirect_call(
                fn_type,
                code_ptr,
                &[env_ptr.into(), arg.value.into()],
                "closure_call",
            )
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let ret = call
            .try_as_basic_value()
            .basic()
            .ok_or_else(|| CodegenError::Llvm("closure call returned no value".into()))?;
        Ok(CompiledValue {
            ty: ret_ty.as_ref().clone(),
            value: ret,
        })
    }

    fn compile_primitive_call(
        &mut self,
        name: &str,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<CompiledValue<'ctx>> {
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
            PrimKind::Fixed(prim) => self.emit_fixed_primitive(prim, args, env, cur_fn),
            PrimKind::Vec(prim) => self.emit_vec_primitive(prim, args, env, cur_fn),
            PrimKind::Arith(op) => {
                let a = self.compile_expr(args[0], env, cur_fn)?;
                let b = self.compile_expr(args[1], env, cur_fn)?;
                self.emit_arith(op, a, b).map(CompiledValue::int)
            }
            PrimKind::Cmp(op) => {
                let a = self.compile_expr(args[0], env, cur_fn)?;
                let b = self.compile_expr(args[1], env, cur_fn)?;
                self.emit_cmp(op, a, b).map(CompiledValue::int)
            }
            PrimKind::Write => self.emit_write(args, env, cur_fn).map(CompiledValue::int),
            PrimKind::Read => self.emit_read(args, env, cur_fn).map(CompiledValue::int),
            PrimKind::Exit => self.emit_exit(args, env, cur_fn).map(CompiledValue::int),
            PrimKind::BufAlloc => Err(CodegenError::Unsupported(
                "@buf-alloc must appear as the direct RHS of a `let` binding",
            )),
            PrimKind::BufAllocDyn => Err(CodegenError::Unsupported(
                "@buf-alloc-dyn must appear as the direct RHS of a `let` binding",
            )),
            PrimKind::I64Alloc => Err(CodegenError::Unsupported(
                "@i64-alloc must appear as the direct RHS of a `let` binding",
            )),
            PrimKind::BufGet => self.emit_buf_get(args, env, cur_fn).map(CompiledValue::int),
            PrimKind::BufSet => self.emit_buf_set(args, env, cur_fn).map(CompiledValue::int),
            PrimKind::BufCopy => self
                .emit_buf_copy(args, env, cur_fn)
                .map(CompiledValue::int),
            PrimKind::BufEq => self.emit_buf_eq(args, env, cur_fn).map(CompiledValue::int),
            PrimKind::ScanByte => self
                .emit_scan_byte(args, env, cur_fn)
                .map(CompiledValue::int),
            PrimKind::ParseI64 => self
                .emit_parse_i64(args, env, cur_fn)
                .map(CompiledValue::int),
            PrimKind::FmtI64 => self.emit_fmt_i64(args, env, cur_fn).map(CompiledValue::int),
            PrimKind::I64Get => self.emit_i64_get(args, env, cur_fn).map(CompiledValue::int),
            PrimKind::I64Set => self.emit_i64_set(args, env, cur_fn).map(CompiledValue::int),
            PrimKind::I64Swap => self
                .emit_i64_swap(args, env, cur_fn)
                .map(CompiledValue::int),
            PrimKind::I64Copy => self
                .emit_i64_copy(args, env, cur_fn)
                .map(CompiledValue::int),
            PrimKind::LineIndex => self
                .emit_line_index(args, env, cur_fn)
                .map(CompiledValue::int),
            PrimKind::TokenIndex => self
                .emit_token_index(args, env, cur_fn)
                .map(CompiledValue::int),
            PrimKind::TokenIndexAny => self
                .emit_token_index_any(args, env, cur_fn)
                .map(CompiledValue::int),
            PrimKind::RangeStart => self
                .emit_range_start(args, env, cur_fn)
                .map(CompiledValue::int),
            PrimKind::RangeLen => self
                .emit_range_len(args, env, cur_fn)
                .map(CompiledValue::int),
            PrimKind::SortI64 => self
                .emit_sort_i64(args, env, cur_fn)
                .map(CompiledValue::int),
            PrimKind::SortRangesByBytes => self
                .emit_sort_ranges_by_bytes(args, env, cur_fn)
                .map(CompiledValue::int),
            PrimKind::StableSortPairsI64 => self
                .emit_stable_sort_pairs_i64(args, env, cur_fn)
                .map(CompiledValue::int),
            PrimKind::LowerBoundI64 => self
                .emit_lower_bound_i64(args, env, cur_fn)
                .map(CompiledValue::int),
            PrimKind::CountEqualRanges => self
                .emit_count_equal_ranges(args, env, cur_fn)
                .map(CompiledValue::int),
            PrimKind::DedupAdjacentRanges => self
                .emit_dedup_adjacent_ranges(args, env, cur_fn)
                .map(CompiledValue::int),
            PrimKind::StdinSlurp => self
                .emit_stdin_slurp(args, env, cur_fn)
                .map(CompiledValue::int),
            PrimKind::WriteRange => self
                .emit_write_range(args, env, cur_fn)
                .map(CompiledValue::int),
            PrimKind::BufRev => self.emit_buf_rev(args, env, cur_fn).map(CompiledValue::int),
            PrimKind::AsciiTolower => self
                .emit_ascii_case_shift(args, env, cur_fn, AsciiCase::Lower)
                .map(CompiledValue::int),
            PrimKind::AsciiToupper => self
                .emit_ascii_case_shift(args, env, cur_fn, AsciiCase::Upper)
                .map(CompiledValue::int),
            PrimKind::AsciiIsAlpha => self
                .emit_ascii_is_alpha(args, env, cur_fn)
                .map(CompiledValue::int),
            PrimKind::AsciiIsDigit => self
                .emit_ascii_is_digit(args, env, cur_fn)
                .map(CompiledValue::int),
            PrimKind::AsciiIsSpace => self
                .emit_ascii_is_space(args, env, cur_fn)
                .map(CompiledValue::int),
            PrimKind::Utf8Decode => self
                .emit_utf8_decode(args, env, cur_fn)
                .map(CompiledValue::int),
            PrimKind::Utf8Encode => self
                .emit_utf8_encode(args, env, cur_fn)
                .map(CompiledValue::int),
            PrimKind::Utf8Len => self
                .emit_utf8_len(args, env, cur_fn)
                .map(CompiledValue::int),
            PrimKind::Map => self.emit_map_i64(args, env, cur_fn).map(CompiledValue::int),
            PrimKind::Fold => self
                .emit_fold_i64(args, env, cur_fn)
                .map(CompiledValue::int),
            PrimKind::ForEach => self
                .emit_for_each_i64(args, env, cur_fn)
                .map(CompiledValue::int),
            PrimKind::Loop => self.emit_loop(args, env, cur_fn),
            PrimKind::LoopStep => self.emit_loop_directive(args, env, cur_fn, 0),
            PrimKind::LoopExit => self.emit_loop_directive(args, env, cur_fn, 1),
            PrimKind::StateLoad => self.emit_state_load(args, env, cur_fn),
            PrimKind::StateStore => self
                .emit_state_store(args, env, cur_fn)
                .map(CompiledValue::int),
            PrimKind::StateAllocVec => self
                .emit_state_alloc_vec(args, env, cur_fn)
                .map(CompiledValue::int),
            PrimKind::StateFreeVec => self
                .emit_state_free_vec(args, env, cur_fn)
                .map(CompiledValue::int),
            PrimKind::StateSlice => Err(CodegenError::Unsupported(
                "@state-slice must appear as the direct RHS of a `let` binding",
            )),
        }
    }

    fn emit_fixed_primitive(
        &mut self,
        prim: FixedPrim,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<CompiledValue<'ctx>> {
        match prim {
            FixedPrim::FromIntWrap { dst } => {
                let value = self.compile_expr(args[0], env, cur_fn)?;
                self.normalize_fixed_int(value, dst).map(CompiledValue::int)
            }
            FixedPrim::Cast { src, dst, kind } => {
                let value = self.compile_expr(args[0], env, cur_fn)?;
                let value = match kind {
                    FixedCastKind::Trunc => value,
                    FixedCastKind::SignExtend => self.normalize_fixed_int(value, src)?,
                    FixedCastKind::ZeroExtend => self.mask_to_width(value, src.width)?,
                };
                self.normalize_fixed_int(value, dst).map(CompiledValue::int)
            }
            FixedPrim::Arith { ty, op, mode } => {
                let a = self.compile_expr(args[0], env, cur_fn)?;
                let b = self.compile_expr(args[1], env, cur_fn)?;
                match mode {
                    FixedArithMode::Wrap => self
                        .emit_fixed_arith_wrap(ty, op, a, b)
                        .map(CompiledValue::int),
                    FixedArithMode::Check => {
                        let (value, overflow) = self.emit_fixed_arith_checked(ty, op, a, b)?;
                        let ok = self.bool_not(overflow, "fixed_ok")?;
                        self.fixed_checked_record(ok, value)
                    }
                    FixedArithMode::Saturate => self
                        .emit_fixed_arith_saturating(ty, op, a, b)
                        .map(CompiledValue::int),
                }
            }
            FixedPrim::Bit { ty, op } => match op {
                FixedBitOp::Not => {
                    let value = self.compile_expr(args[0], env, cur_fn)?;
                    let masked = self.mask_to_width(value, ty.width)?;
                    let inverted = self
                        .builder
                        .build_not(masked, "fixed_not")
                        .map_err(|e| CodegenError::Llvm(e.to_string()))?;
                    self.normalize_fixed_int(inverted, ty)
                        .map(CompiledValue::int)
                }
                FixedBitOp::And | FixedBitOp::Or | FixedBitOp::Xor => {
                    let a_raw = self.compile_expr(args[0], env, cur_fn)?;
                    let b_raw = self.compile_expr(args[1], env, cur_fn)?;
                    let a = self.mask_to_width(a_raw, ty.width)?;
                    let b = self.mask_to_width(b_raw, ty.width)?;
                    let value = match op {
                        FixedBitOp::And => self.builder.build_and(a, b, "fixed_and"),
                        FixedBitOp::Or => self.builder.build_or(a, b, "fixed_or"),
                        FixedBitOp::Xor => self.builder.build_xor(a, b, "fixed_xor"),
                        FixedBitOp::Not => unreachable!(),
                    }
                    .map_err(|e| CodegenError::Llvm(e.to_string()))?;
                    self.normalize_fixed_int(value, ty).map(CompiledValue::int)
                }
            },
            FixedPrim::Shift { ty, op } => {
                let value = self.compile_expr(args[0], env, cur_fn)?;
                let count = self.compile_expr(args[1], env, cur_fn)?;
                let shifted = match op {
                    FixedShiftOp::Shl => self.emit_fixed_shift(ty, value, count, false)?,
                    FixedShiftOp::Shr => self.emit_fixed_shift(ty, value, count, true)?,
                    FixedShiftOp::Rotl => self.emit_fixed_rotate(ty, value, count, true)?,
                    FixedShiftOp::Rotr => self.emit_fixed_rotate(ty, value, count, false)?,
                };
                Ok(CompiledValue::int(shifted))
            }
            FixedPrim::MaskLow { ty } => {
                let count = self.compile_expr(args[0], env, cur_fn)?;
                self.emit_fixed_mask_low(ty, count).map(CompiledValue::int)
            }
            FixedPrim::Bytes { ty, endian } => {
                let mut bytes = Vec::with_capacity(args.len());
                for arg in args {
                    bytes.push(self.compile_expr(arg, env, cur_fn)?);
                }
                self.emit_fixed_from_bytes(ty, endian, &bytes)
                    .map(CompiledValue::int)
            }
            FixedPrim::ByteSwap { ty } => {
                let value = self.compile_expr(args[0], env, cur_fn)?;
                self.emit_fixed_bswap(ty, value).map(CompiledValue::int)
            }
        }
    }

    fn fixed_checked_record(
        &mut self,
        ok: IntValue<'ctx>,
        value: IntValue<'ctx>,
    ) -> Result<CompiledValue<'ctx>> {
        let ty = ValueTy::Record(vec![
            ("ok".to_string(), ValueTy::Int),
            ("value".to_string(), ValueTy::Int),
        ]);
        let struct_ty = self.llvm_struct_type(&ty)?;
        let mut aggregate = struct_ty.get_undef();
        aggregate = self
            .builder
            .build_insert_value(aggregate, ok, 0, "checked_ok")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_struct_value();
        aggregate = self
            .builder
            .build_insert_value(aggregate, value, 1, "checked_value")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_struct_value();
        Ok(CompiledValue {
            ty,
            value: aggregate.into(),
        })
    }

    fn emit_fixed_arith_wrap(
        &mut self,
        ty: FixedIntTy,
        op: FixedArithOp,
        a: IntValue<'ctx>,
        b: IntValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let a = self.normalize_fixed_int(a, ty)?;
        let b = self.normalize_fixed_int(b, ty)?;
        let raw = match op {
            FixedArithOp::Add => self.builder.build_int_add(a, b, "fixed_add"),
            FixedArithOp::Sub => self.builder.build_int_sub(a, b, "fixed_sub"),
            FixedArithOp::Mul => self.builder.build_int_mul(a, b, "fixed_mul"),
        }
        .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.normalize_fixed_int(raw, ty)
    }

    fn emit_fixed_arith_checked(
        &mut self,
        ty: FixedIntTy,
        op: FixedArithOp,
        a: IntValue<'ctx>,
        b: IntValue<'ctx>,
    ) -> Result<(IntValue<'ctx>, IntValue<'ctx>)> {
        if matches!(op, FixedArithOp::Mul) {
            return Err(CodegenError::Unsupported(
                "checked fixed-width multiplication",
            ));
        }
        let a = self.normalize_fixed_int(a, ty)?;
        let b = self.normalize_fixed_int(b, ty)?;
        let raw = match op {
            FixedArithOp::Add => self.builder.build_int_add(a, b, "fixed_add_check"),
            FixedArithOp::Sub => self.builder.build_int_sub(a, b, "fixed_sub_check"),
            FixedArithOp::Mul => unreachable!(),
        }
        .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let value = self.normalize_fixed_int(raw, ty)?;
        let overflow = if ty.sign == IntSign::Unsigned {
            match op {
                FixedArithOp::Add => {
                    if ty.width == 64 {
                        self.builder
                            .build_int_compare(IntPredicate::ULT, value, a, "uadd_overflow")
                    } else {
                        let max = self.i64_const(fixed_mask(ty.width));
                        self.builder
                            .build_int_compare(IntPredicate::UGT, raw, max, "uadd_overflow")
                    }
                }
                FixedArithOp::Sub => {
                    self.builder
                        .build_int_compare(IntPredicate::ULT, a, b, "usub_overflow")
                }
                FixedArithOp::Mul => unreachable!(),
            }
            .map_err(|e| CodegenError::Llvm(e.to_string()))
        } else {
            let sign_bits = match op {
                FixedArithOp::Add => {
                    let a_xor_result = self
                        .builder
                        .build_xor(a, value, "sadd_axr")
                        .map_err(|e| CodegenError::Llvm(e.to_string()))?;
                    let b_xor_result = self
                        .builder
                        .build_xor(b, value, "sadd_bxr")
                        .map_err(|e| CodegenError::Llvm(e.to_string()))?;
                    self.builder
                        .build_and(a_xor_result, b_xor_result, "sadd_over_bits")
                }
                FixedArithOp::Sub => {
                    let a_xor_b = self
                        .builder
                        .build_xor(a, b, "ssub_axb")
                        .map_err(|e| CodegenError::Llvm(e.to_string()))?;
                    let a_xor_result = self
                        .builder
                        .build_xor(a, value, "ssub_axr")
                        .map_err(|e| CodegenError::Llvm(e.to_string()))?;
                    self.builder
                        .build_and(a_xor_b, a_xor_result, "ssub_over_bits")
                }
                FixedArithOp::Mul => unreachable!(),
            }
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
            self.sign_bit_set(sign_bits, ty.width, "signed_overflow")
        }?;
        Ok((value, overflow))
    }

    fn emit_fixed_arith_saturating(
        &mut self,
        ty: FixedIntTy,
        op: FixedArithOp,
        a: IntValue<'ctx>,
        b: IntValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let (wrapped, overflow) = self.emit_fixed_arith_checked(ty, op, a, b)?;
        let saturation = if ty.sign == IntSign::Unsigned {
            match op {
                FixedArithOp::Add => self.i64_const(fixed_mask(ty.width)),
                FixedArithOp::Sub => self.context.i64_type().const_zero(),
                FixedArithOp::Mul => unreachable!(),
            }
        } else {
            let a_norm = self.normalize_fixed_int(a, ty)?;
            let zero = self.context.i64_type().const_zero();
            let a_nonnegative = self
                .builder
                .build_int_compare(IntPredicate::SGE, a_norm, zero, "sat_pos")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
            let max = self.i64_signed_const(fixed_signed_max(ty.width));
            let min = self.i64_signed_const(fixed_signed_min(ty.width));
            self.builder
                .build_select(a_nonnegative, max, min, "sat_bound")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?
                .into_int_value()
        };
        Ok(self
            .builder
            .build_select(overflow, saturation, wrapped, "fixed_sat")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_int_value())
    }

    fn emit_fixed_shift(
        &mut self,
        ty: FixedIntTy,
        value: IntValue<'ctx>,
        count: IntValue<'ctx>,
        right: bool,
    ) -> Result<IntValue<'ctx>> {
        let value = self.normalize_fixed_int(value, ty)?;
        let zero = self.context.i64_type().const_zero();
        let width = self.i64_const(u64::from(ty.width));
        let count_neg = self
            .builder
            .build_int_compare(IntPredicate::SLT, count, zero, "shift_neg")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let count_ge_width = self
            .builder
            .build_int_compare(IntPredicate::SGE, count, width, "shift_too_wide")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let invalid = self
            .builder
            .build_or(count_neg, count_ge_width, "shift_invalid")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let safe_count = self
            .builder
            .build_select(invalid, zero, count, "shift_count")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_int_value();
        let shifted = if right {
            self.builder.build_right_shift(
                value,
                safe_count,
                ty.sign == IntSign::Signed,
                "fixed_shr",
            )
        } else {
            self.builder
                .build_left_shift(value, safe_count, "fixed_shl")
        }
        .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let fallback = if right && ty.sign == IntSign::Signed {
            let negative = self
                .builder
                .build_int_compare(IntPredicate::SLT, value, zero, "shr_neg")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
            self.builder
                .build_select(
                    negative,
                    self.i64_signed_const(-1),
                    zero,
                    "shr_invalid_fill",
                )
                .map_err(|e| CodegenError::Llvm(e.to_string()))?
                .into_int_value()
        } else {
            zero
        };
        let selected = self
            .builder
            .build_select(invalid, fallback, shifted, "fixed_shift")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_int_value();
        self.normalize_fixed_int(selected, ty)
    }

    fn emit_fixed_rotate(
        &mut self,
        ty: FixedIntTy,
        value: IntValue<'ctx>,
        count: IntValue<'ctx>,
        left: bool,
    ) -> Result<IntValue<'ctx>> {
        let value = self.mask_to_width(value, ty.width)?;
        let count_mask = self.i64_const(u64::from(ty.width - 1));
        let count = self
            .builder
            .build_and(count, count_mask, "rot_count")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let neg_count = self
            .builder
            .build_int_sub(self.context.i64_type().const_zero(), count, "rot_neg")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let inv_count = self
            .builder
            .build_and(neg_count, count_mask, "rot_inv_count")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let (left_count, right_count) = if left {
            (count, inv_count)
        } else {
            (inv_count, count)
        };
        let left_part = self
            .builder
            .build_left_shift(value, left_count, "rot_left")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let right_part = self
            .builder
            .build_right_shift(value, right_count, false, "rot_right")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let combined = self
            .builder
            .build_or(left_part, right_part, "rot_combined")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.normalize_fixed_int(combined, ty)
    }

    fn emit_fixed_mask_low(
        &mut self,
        ty: FixedIntTy,
        count: IntValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let zero = self.context.i64_type().const_zero();
        let one = self.i64_const(1);
        let width_minus_one = self.i64_const(u64::from(ty.width - 1));
        let count_le_zero = self
            .builder
            .build_int_compare(IntPredicate::SLE, count, zero, "mask_empty")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let count_ge_width = self
            .builder
            .build_int_compare(
                IntPredicate::SGE,
                count,
                self.i64_const(u64::from(ty.width)),
                "mask_full",
            )
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let clamped = self
            .builder
            .build_select(count_ge_width, width_minus_one, count, "mask_count_hi")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_int_value();
        let clamped = self
            .builder
            .build_select(count_le_zero, zero, clamped, "mask_count")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_int_value();
        let shifted = self
            .builder
            .build_left_shift(one, clamped, "mask_shift")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let partial = self
            .builder
            .build_int_sub(shifted, one, "mask_partial")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let full = self.i64_const(fixed_mask(ty.width));
        let selected = self
            .builder
            .build_select(count_ge_width, full, partial, "mask_selected")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_int_value();
        let selected = self
            .builder
            .build_select(count_le_zero, zero, selected, "mask_zeroed")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_int_value();
        self.normalize_fixed_int(selected, ty)
    }

    fn emit_fixed_from_bytes(
        &mut self,
        ty: FixedIntTy,
        endian: FixedEndian,
        bytes: &[IntValue<'ctx>],
    ) -> Result<IntValue<'ctx>> {
        let mut value = self.context.i64_type().const_zero();
        for (i, byte) in bytes.iter().enumerate() {
            let src_index = match endian {
                FixedEndian::Big => i,
                FixedEndian::Little => bytes.len() - 1 - i,
            };
            let shift = 8 * (bytes.len() - 1 - src_index);
            let byte = self.mask_to_width(*byte, 8)?;
            let shifted = if shift == 0 {
                byte
            } else {
                self.builder
                    .build_left_shift(byte, self.i64_const(shift as u64), "byte_shift")
                    .map_err(|e| CodegenError::Llvm(e.to_string()))?
            };
            value = self
                .builder
                .build_or(value, shifted, "byte_acc")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        }
        self.normalize_fixed_int(value, ty)
    }

    fn emit_fixed_bswap(
        &mut self,
        ty: FixedIntTy,
        value: IntValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let value = self.mask_to_width(value, ty.width)?;
        let byte_count = ty.width / 8;
        let mut out = self.context.i64_type().const_zero();
        for i in 0..byte_count {
            let shift_in = 8 * i;
            let shift_out = 8 * (byte_count - 1 - i);
            let shifted = if shift_in == 0 {
                value
            } else {
                self.builder
                    .build_right_shift(
                        value,
                        self.i64_const(u64::from(shift_in)),
                        false,
                        "bswap_in",
                    )
                    .map_err(|e| CodegenError::Llvm(e.to_string()))?
            };
            let byte = self.mask_to_width(shifted, 8)?;
            let shifted_out = if shift_out == 0 {
                byte
            } else {
                self.builder
                    .build_left_shift(byte, self.i64_const(u64::from(shift_out)), "bswap_out")
                    .map_err(|e| CodegenError::Llvm(e.to_string()))?
            };
            out = self
                .builder
                .build_or(out, shifted_out, "bswap_acc")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        }
        self.normalize_fixed_int(out, ty)
    }

    fn normalize_fixed_int(
        &mut self,
        value: IntValue<'ctx>,
        ty: FixedIntTy,
    ) -> Result<IntValue<'ctx>> {
        let masked = self.mask_to_width(value, ty.width)?;
        if ty.sign == IntSign::Signed && ty.width < 64 {
            let shift = self.i64_const(u64::from(64 - ty.width));
            let shifted_left = self
                .builder
                .build_left_shift(masked, shift, "sext_left")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
            self.builder
                .build_right_shift(shifted_left, shift, true, "sext_right")
                .map_err(|e| CodegenError::Llvm(e.to_string()))
        } else {
            Ok(masked)
        }
    }

    fn mask_to_width(&mut self, value: IntValue<'ctx>, width: u16) -> Result<IntValue<'ctx>> {
        if width == 64 {
            Ok(value)
        } else {
            self.builder
                .build_and(value, self.i64_const(fixed_mask(width)), "fixed_mask")
                .map_err(|e| CodegenError::Llvm(e.to_string()))
        }
    }

    fn sign_bit_set(
        &mut self,
        value: IntValue<'ctx>,
        width: u16,
        name: &str,
    ) -> Result<IntValue<'ctx>> {
        let masked = self
            .builder
            .build_and(value, self.i64_const(fixed_sign_bit(width)), "sign_bit")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_int_compare(
                IntPredicate::NE,
                masked,
                self.context.i64_type().const_zero(),
                name,
            )
            .map_err(|e| CodegenError::Llvm(e.to_string()))
    }

    fn bool_not(&mut self, value: IntValue<'ctx>, name: &str) -> Result<IntValue<'ctx>> {
        let not = self
            .builder
            .build_not(value, name)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_int_z_extend(not, self.context.i64_type(), "bool_zext")
            .map_err(|e| CodegenError::Llvm(e.to_string()))
    }

    fn i64_const(&self, value: u64) -> IntValue<'ctx> {
        self.context.i64_type().const_int(value, false)
    }

    fn i64_signed_const(&self, value: i64) -> IntValue<'ctx> {
        self.context.i64_type().const_int(value as u64, true)
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

    /// Lower a read-only or writable byte-buffer argument.
    /// Accepts either a string literal (→ private global) or a `Var` resolving
    /// to a byte-buffer `Binding::Ptr` (→ stack buffer from `@buf-alloc`, ADR 0038).
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
            // `Var` may resolve to a byte-buffer pointer produced by `@buf-alloc` (ADR 0038).
            Node::Var { index } => {
                let binding = lookup_var(env, *index)?;
                match binding {
                    Binding::Ptr {
                        ptr,
                        kind: PtrKind::Buf,
                    } => Ok(*ptr),
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

    /// Resolve a buffer-like primitive argument to a `PointerValue`.
    fn compile_ptr_arg<'a>(
        &self,
        node: &Node,
        env: &'a [Binding<'ctx>],
        expected: PtrKind,
        expected_name: &'static str,
    ) -> Result<PointerValue<'ctx>> {
        match node {
            Node::Var { index } => {
                let binding = lookup_var(env, *index)?;
                match binding {
                    Binding::Ptr { ptr, kind } if *kind == expected => Ok(*ptr),
                    _ => Err(CodegenError::Unsupported(expected_name)),
                }
            }
            _ => Err(CodegenError::Unsupported(expected_name)),
        }
    }

    fn compile_buf_ptr_arg(
        &self,
        node: &Node,
        env: &[Binding<'ctx>],
    ) -> Result<PointerValue<'ctx>> {
        self.compile_ptr_arg(
            node,
            env,
            PtrKind::Buf,
            "buffer argument must be a variable referencing a byte buffer binding",
        )
    }

    fn compile_i64_vec_arg(
        &self,
        node: &Node,
        env: &[Binding<'ctx>],
    ) -> Result<PointerValue<'ctx>> {
        self.compile_ptr_arg(
            node,
            env,
            PtrKind::I64Vec,
            "i64 vector argument must be a variable referencing an @i64-alloc binding",
        )
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

    /// Declare (or retrieve) the `llvm.memmove.p0.p0.i64` intrinsic (ADR 0061).
    fn llvm_memmove(&self) -> FunctionValue<'ctx> {
        let name = "llvm.memmove.p0.p0.i64";
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

    /// GEP to `vec[index]` (element type `i64`).
    fn i64_ptr_at(
        &mut self,
        vec_ptr: PointerValue<'ctx>,
        index: IntValue<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>> {
        let i64_t = self.context.i64_type();
        unsafe { self.builder.build_gep(i64_t, vec_ptr, &[index], name) }
            .map_err(|e| CodegenError::Llvm(e.to_string()))
    }

    fn load_i64(
        &mut self,
        vec_ptr: PointerValue<'ctx>,
        index: IntValue<'ctx>,
        name: &str,
    ) -> Result<IntValue<'ctx>> {
        let i64_t = self.context.i64_type();
        let ptr = self.i64_ptr_at(vec_ptr, index, name)?;
        let value = self
            .builder
            .build_load(i64_t, ptr, name)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        Ok(value.into_int_value())
    }

    fn store_i64(
        &mut self,
        vec_ptr: PointerValue<'ctx>,
        index: IntValue<'ctx>,
        value: IntValue<'ctx>,
    ) -> Result<()> {
        let ptr = self.i64_ptr_at(vec_ptr, index, "i64_store_ptr")?;
        self.builder
            .build_store(ptr, value)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        Ok(())
    }

    fn store_range_pair(
        &mut self,
        table: PointerValue<'ctx>,
        row: IntValue<'ctx>,
        start: IntValue<'ctx>,
        len: IntValue<'ctx>,
    ) -> Result<()> {
        let i64_t = self.context.i64_type();
        let two64 = i64_t.const_int(2, false);
        let one64 = i64_t.const_int(1, false);
        let base = self
            .builder
            .build_int_mul(row, two64, "range_base")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let len_index = self
            .builder
            .build_int_add(base, one64, "range_len_index")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.store_i64(table, base, start)?;
        self.store_i64(table, len_index, len)?;
        Ok(())
    }

    fn load_range_pair(
        &mut self,
        table: PointerValue<'ctx>,
        row: IntValue<'ctx>,
        prefix: &str,
    ) -> Result<(IntValue<'ctx>, IntValue<'ctx>)> {
        let i64_t = self.context.i64_type();
        let two64 = i64_t.const_int(2, false);
        let one64 = i64_t.const_int(1, false);
        let base = self
            .builder
            .build_int_mul(row, two64, &format!("{prefix}_base"))
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let len_index = self
            .builder
            .build_int_add(base, one64, &format!("{prefix}_len_index"))
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let start = self.load_i64(table, base, &format!("{prefix}_start"))?;
        let len = self.load_i64(table, len_index, &format!("{prefix}_len"))?;
        Ok((start, len))
    }

    fn store_range_count_triple(
        &mut self,
        out: PointerValue<'ctx>,
        row: IntValue<'ctx>,
        start: IntValue<'ctx>,
        len: IntValue<'ctx>,
        count: IntValue<'ctx>,
    ) -> Result<()> {
        let i64_t = self.context.i64_type();
        let three64 = i64_t.const_int(3, false);
        let one64 = i64_t.const_int(1, false);
        let two64 = i64_t.const_int(2, false);
        let base = self
            .builder
            .build_int_mul(row, three64, "range_count_base")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let len_index = self
            .builder
            .build_int_add(base, one64, "range_count_len_index")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let count_index = self
            .builder
            .build_int_add(base, two64, "range_count_count_index")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.store_i64(out, base, start)?;
        self.store_i64(out, len_index, len)?;
        self.store_i64(out, count_index, count)?;
        Ok(())
    }

    fn emit_byte_in_delims(
        &mut self,
        byte: IntValue<'ctx>,
        delims: PointerValue<'ctx>,
        delim_count: IntValue<'ctx>,
        cur_fn: FunctionValue<'ctx>,
        prefix: &str,
    ) -> Result<IntValue<'ctx>> {
        let i64_t = self.context.i64_type();
        let i8_t = self.context.i8_type();
        let bool_t = self.context.bool_type();
        let zero64 = i64_t.const_zero();
        let one64 = i64_t.const_int(1, false);
        let false_val = bool_t.const_int(0, false);
        let true_val = bool_t.const_int(1, false);

        let entry_bb = self.builder.get_insert_block().unwrap();
        let hdr_bb = self
            .context
            .append_basic_block(cur_fn, &format!("{prefix}_delim_hdr"));
        let check_bb = self
            .context
            .append_basic_block(cur_fn, &format!("{prefix}_delim_chk"));
        let next_bb = self
            .context
            .append_basic_block(cur_fn, &format!("{prefix}_delim_next"));
        let found_bb = self
            .context
            .append_basic_block(cur_fn, &format!("{prefix}_delim_found"));
        let merge_bb = self
            .context
            .append_basic_block(cur_fn, &format!("{prefix}_delim_merge"));

        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(hdr_bb);
        let k_phi = self
            .builder
            .build_phi(i64_t, &format!("{prefix}_delim_k"))
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let k_val = k_phi.as_basic_value().into_int_value();
        let done = self
            .builder
            .build_int_compare(IntPredicate::SGE, k_val, delim_count, "delim_done")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(done, merge_bb, check_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(check_bb);
        let delim_ptr = self.ptr_at(delims, k_val, "delim_ptr")?;
        let delim_byte = self
            .builder
            .build_load(i8_t, delim_ptr, "delim_byte")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_int_value();
        let matches = self
            .builder
            .build_int_compare(IntPredicate::EQ, delim_byte, byte, "delim_matches")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(matches, found_bb, next_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(next_bb);
        let k_next = self
            .builder
            .build_int_add(k_val, one64, "delim_k_next")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        k_phi.add_incoming(&[
            (&zero64 as &dyn BasicValue<'ctx>, entry_bb),
            (&k_next as &dyn BasicValue<'ctx>, next_bb),
        ]);

        self.builder.position_at_end(found_bb);
        self.builder
            .build_unconditional_branch(merge_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(merge_bb);
        let found_phi = self
            .builder
            .build_phi(bool_t, &format!("{prefix}_delim_ret"))
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        found_phi.add_incoming(&[
            (&false_val as &dyn BasicValue<'ctx>, hdr_bb),
            (&true_val as &dyn BasicValue<'ctx>, found_bb),
        ]);
        Ok(found_phi.as_basic_value().into_int_value())
    }

    fn emit_range_bytes_gt_key(
        &mut self,
        text: PointerValue<'ctx>,
        table: PointerValue<'ctx>,
        row: IntValue<'ctx>,
        key: (IntValue<'ctx>, IntValue<'ctx>),
        cur_fn: FunctionValue<'ctx>,
        prefix: &str,
    ) -> Result<IntValue<'ctx>> {
        let i64_t = self.context.i64_type();
        let bool_t = self.context.bool_type();
        let zero64 = i64_t.const_zero();
        let one64 = i64_t.const_int(1, false);
        let false_val = bool_t.const_int(0, false);
        let true_val = bool_t.const_int(1, false);

        let (key_start, key_len) = key;
        let (start, len) = self.load_range_pair(table, row, prefix)?;

        let entry_bb = self.builder.get_insert_block().unwrap();
        let hdr_bb = self
            .context
            .append_basic_block(cur_fn, &format!("{prefix}_cmp_hdr"));
        let check_bb = self
            .context
            .append_basic_block(cur_fn, &format!("{prefix}_cmp_chk"));
        let lt_check_bb = self
            .context
            .append_basic_block(cur_fn, &format!("{prefix}_cmp_lt"));
        let cont_bb = self
            .context
            .append_basic_block(cur_fn, &format!("{prefix}_cmp_cont"));
        let gt_bb = self
            .context
            .append_basic_block(cur_fn, &format!("{prefix}_cmp_gt"));
        let le_bb = self
            .context
            .append_basic_block(cur_fn, &format!("{prefix}_cmp_le"));
        let prefix_done_bb = self
            .context
            .append_basic_block(cur_fn, &format!("{prefix}_cmp_prefix"));
        let merge_bb = self
            .context
            .append_basic_block(cur_fn, &format!("{prefix}_cmp_merge"));

        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(hdr_bb);
        let k_phi = self
            .builder
            .build_phi(i64_t, &format!("{prefix}_cmp_k"))
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let k_val = k_phi.as_basic_value().into_int_value();
        let in_left = self
            .builder
            .build_int_compare(IntPredicate::SLT, k_val, len, "range_cmp_in_left")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let in_key = self
            .builder
            .build_int_compare(IntPredicate::SLT, k_val, key_len, "range_cmp_in_key")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let in_both = self
            .builder
            .build_and(in_left, in_key, "range_cmp_in_both")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(in_both, check_bb, prefix_done_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(check_bb);
        let left_off = self
            .builder
            .build_int_add(start, k_val, "range_cmp_left_off")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let key_off = self
            .builder
            .build_int_add(key_start, k_val, "range_cmp_key_off")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let left_byte = self.load_byte(text, left_off, "range_cmp_left_byte")?;
        let key_byte = self.load_byte(text, key_off, "range_cmp_key_byte")?;
        let left_gt = self
            .builder
            .build_int_compare(IntPredicate::UGT, left_byte, key_byte, "range_cmp_gt")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(left_gt, gt_bb, lt_check_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(lt_check_bb);
        let left_lt = self
            .builder
            .build_int_compare(IntPredicate::ULT, left_byte, key_byte, "range_cmp_lt")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(left_lt, le_bb, cont_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(cont_bb);
        let k_next = self
            .builder
            .build_int_add(k_val, one64, "range_cmp_k_next")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        k_phi.add_incoming(&[
            (&zero64 as &dyn BasicValue<'ctx>, entry_bb),
            (&k_next as &dyn BasicValue<'ctx>, cont_bb),
        ]);

        self.builder.position_at_end(gt_bb);
        self.builder
            .build_unconditional_branch(merge_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(le_bb);
        self.builder
            .build_unconditional_branch(merge_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(prefix_done_bb);
        let left_longer = self
            .builder
            .build_int_compare(IntPredicate::SGT, len, key_len, "range_cmp_left_longer")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(merge_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(merge_bb);
        let result_phi = self
            .builder
            .build_phi(bool_t, &format!("{prefix}_cmp_result"))
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        result_phi.add_incoming(&[
            (&true_val as &dyn BasicValue<'ctx>, gt_bb),
            (&false_val as &dyn BasicValue<'ctx>, le_bb),
            (&left_longer as &dyn BasicValue<'ctx>, prefix_done_bb),
        ]);
        Ok(result_phi.as_basic_value().into_int_value())
    }

    fn emit_range_bytes_eq_key(
        &mut self,
        text: PointerValue<'ctx>,
        table: PointerValue<'ctx>,
        row: IntValue<'ctx>,
        key: (IntValue<'ctx>, IntValue<'ctx>),
        cur_fn: FunctionValue<'ctx>,
        prefix: &str,
    ) -> Result<IntValue<'ctx>> {
        let i64_t = self.context.i64_type();
        let bool_t = self.context.bool_type();
        let zero64 = i64_t.const_zero();
        let one64 = i64_t.const_int(1, false);
        let false_val = bool_t.const_int(0, false);
        let true_val = bool_t.const_int(1, false);

        let (key_start, key_len) = key;
        let (start, len) = self.load_range_pair(table, row, prefix)?;

        let entry_bb = self.builder.get_insert_block().unwrap();
        let hdr_bb = self
            .context
            .append_basic_block(cur_fn, &format!("{prefix}_eq_hdr"));
        let check_bb = self
            .context
            .append_basic_block(cur_fn, &format!("{prefix}_eq_chk"));
        let cont_bb = self
            .context
            .append_basic_block(cur_fn, &format!("{prefix}_eq_cont"));
        let equal_bb = self
            .context
            .append_basic_block(cur_fn, &format!("{prefix}_eq_true"));
        let false_bb = self
            .context
            .append_basic_block(cur_fn, &format!("{prefix}_eq_false"));
        let merge_bb = self
            .context
            .append_basic_block(cur_fn, &format!("{prefix}_eq_merge"));

        let len_eq = self
            .builder
            .build_int_compare(IntPredicate::EQ, len, key_len, "range_eq_len")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(len_eq, hdr_bb, false_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(hdr_bb);
        let k_phi = self
            .builder
            .build_phi(i64_t, &format!("{prefix}_eq_k"))
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let k_val = k_phi.as_basic_value().into_int_value();
        let done = self
            .builder
            .build_int_compare(IntPredicate::SGE, k_val, len, "range_eq_done")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(done, equal_bb, check_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(check_bb);
        let left_off = self
            .builder
            .build_int_add(start, k_val, "range_eq_left_off")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let key_off = self
            .builder
            .build_int_add(key_start, k_val, "range_eq_key_off")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let left_byte = self.load_byte(text, left_off, "range_eq_left_byte")?;
        let key_byte = self.load_byte(text, key_off, "range_eq_key_byte")?;
        let bytes_eq = self
            .builder
            .build_int_compare(IntPredicate::EQ, left_byte, key_byte, "range_eq_bytes")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(bytes_eq, cont_bb, false_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(cont_bb);
        let k_next = self
            .builder
            .build_int_add(k_val, one64, "range_eq_k_next")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        k_phi.add_incoming(&[
            (&zero64 as &dyn BasicValue<'ctx>, entry_bb),
            (&k_next as &dyn BasicValue<'ctx>, cont_bb),
        ]);

        self.builder.position_at_end(equal_bb);
        self.builder
            .build_unconditional_branch(merge_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(false_bb);
        self.builder
            .build_unconditional_branch(merge_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(merge_bb);
        let result_phi = self
            .builder
            .build_phi(bool_t, &format!("{prefix}_eq_result"))
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        result_phi.add_incoming(&[
            (&true_val as &dyn BasicValue<'ctx>, equal_bb),
            (&false_val as &dyn BasicValue<'ctx>, false_bb),
        ]);
        Ok(result_phi.as_basic_value().into_int_value())
    }

    // ── Phase 3 emit functions (ADR 0047, ADR 0061, ADR 0062, ADR 0063, ADR 0064) ─

    /// `@buf-get buf off` → load `buf[off]` as `i64`.
    fn emit_buf_get(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let buf = self.compile_buf_ptr_arg(args[0], env)?;
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
        let buf = self.compile_buf_ptr_arg(args[0], env)?;
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
        let dst = self.compile_buf_ptr_arg(args[0], env)?;
        let dst_off = self.compile_expr(args[1], env, cur_fn)?;
        let src = self.compile_buf_ptr_arg(args[2], env)?;
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

    /// `@i64-get vec index` → load `vec[index]` as `i64`.
    fn emit_i64_get(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let vec = self.compile_i64_vec_arg(args[0], env)?;
        let index = self.compile_expr(args[1], env, cur_fn)?;
        self.load_i64(vec, index, "i64_get")
    }

    /// `@i64-set vec index value` → store `value` at `vec[index]`; return 0.
    fn emit_i64_set(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let vec = self.compile_i64_vec_arg(args[0], env)?;
        let index = self.compile_expr(args[1], env, cur_fn)?;
        let value = self.compile_expr(args[2], env, cur_fn)?;
        self.store_i64(vec, index, value)?;
        Ok(self.context.i64_type().const_zero())
    }

    /// `@i64-swap vec i j` → swap `vec[i]` and `vec[j]`; return 0.
    fn emit_i64_swap(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let vec = self.compile_i64_vec_arg(args[0], env)?;
        let i = self.compile_expr(args[1], env, cur_fn)?;
        let j = self.compile_expr(args[2], env, cur_fn)?;
        let i_ptr = self.i64_ptr_at(vec, i, "i64_swap_i")?;
        let j_ptr = self.i64_ptr_at(vec, j, "i64_swap_j")?;
        let i64_t = self.context.i64_type();
        let a = self
            .builder
            .build_load(i64_t, i_ptr, "i64_swap_a")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_int_value();
        let b = self
            .builder
            .build_load(i64_t, j_ptr, "i64_swap_b")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_int_value();
        self.builder
            .build_store(i_ptr, b)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_store(j_ptr, a)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        Ok(i64_t.const_zero())
    }

    /// `@i64-copy dst dst-index src src-index count` → overlap-safe element copy.
    fn emit_i64_copy(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let dst = self.compile_i64_vec_arg(args[0], env)?;
        let dst_index = self.compile_expr(args[1], env, cur_fn)?;
        let src = self.compile_i64_vec_arg(args[2], env)?;
        let src_index = self.compile_expr(args[3], env, cur_fn)?;
        let count = self.compile_expr(args[4], env, cur_fn)?;

        let dst_ptr = self.i64_ptr_at(dst, dst_index, "i64_cp_dst")?;
        let src_ptr = self.i64_ptr_at(src, src_index, "i64_cp_src")?;
        let i64_t = self.context.i64_type();
        let elem_size = i64_t.const_int(8, false);
        let byte_len = self
            .builder
            .build_int_mul(count, elem_size, "i64_cp_bytes")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let false_val = self.context.bool_type().const_int(0, false);

        let memmove = self.llvm_memmove();
        self.builder
            .build_call(
                memmove,
                &[
                    BasicMetadataValueEnum::PointerValue(dst_ptr),
                    BasicMetadataValueEnum::PointerValue(src_ptr),
                    BasicMetadataValueEnum::IntValue(byte_len),
                    BasicMetadataValueEnum::IntValue(false_val),
                ],
                "i64_memmove",
            )
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        Ok(i64_t.const_zero())
    }

    // ── Stage 7 typed mutable vector helpers (ADR 0085) ─────────────────────

    fn llvm_int_type_for_width(&self, width: u16) -> inkwell::types::IntType<'ctx> {
        match width {
            8 => self.context.i8_type(),
            16 => self.context.i16_type(),
            32 => self.context.i32_type(),
            64 => self.context.i64_type(),
            _ => unreachable!("ADR 0084 fixed-width integer widths are 8/16/32/64"),
        }
    }

    fn llvm_trap(&self) -> FunctionValue<'ctx> {
        let name = "llvm.trap";
        if let Some(f) = self.module.get_function(name) {
            return f;
        }
        let void_t = self.context.void_type();
        let ty = void_t.fn_type(&[], false);
        self.module.add_function(name, ty, None)
    }

    /// Emit `if cond { llvm.trap(); unreachable; }` and continue at a fresh
    /// "ok" block. `cond` is an `i1` that is true when the access is
    /// out-of-bounds.
    fn emit_bounds_trap(
        &mut self,
        cur_fn: FunctionValue<'ctx>,
        cond: IntValue<'ctx>,
        name: &str,
    ) -> Result<()> {
        let trap_bb = self
            .context
            .append_basic_block(cur_fn, &format!("{name}_trap"));
        let ok_bb = self
            .context
            .append_basic_block(cur_fn, &format!("{name}_ok"));
        self.builder
            .build_conditional_branch(cond, trap_bb, ok_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder.position_at_end(trap_bb);
        let trap = self.llvm_trap();
        self.builder
            .build_call(trap, &[], "vec_trap")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unreachable()
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder.position_at_end(ok_bb);
        Ok(())
    }

    /// Resolve a `<ty>vec` argument to its (pointer, length, type) triple
    /// from the current binding stack. Anti-escape: the only legal source
    /// of a vec handle is a `Var` bound to a `Binding::VecHandle`.
    fn resolve_vec_arg(
        &self,
        node: &Node,
        env: &[Binding<'ctx>],
        expected: FixedIntTy,
    ) -> Result<(PointerValue<'ctx>, IntValue<'ctx>)> {
        match node {
            Node::Var { index } => match lookup_var(env, *index)? {
                Binding::VecHandle { ptr, len, ty } if *ty == expected => Ok((*ptr, *len)),
                Binding::VecHandle { .. } => Err(CodegenError::Unsupported(
                    "vec primitive received wrong vec element type",
                )),
                _ => Err(CodegenError::Unsupported(
                    "typed vec argument must reference an @<ty>vec-alloc binding",
                )),
            },
            _ => Err(CodegenError::Unsupported(
                "typed vec argument must reference an @<ty>vec-alloc binding",
            )),
        }
    }

    /// GEP to `vec[index]` using the storage element type. Returns the
    /// element pointer.
    fn vec_element_ptr(
        &mut self,
        base: PointerValue<'ctx>,
        index: IntValue<'ctx>,
        elem_ty: FixedIntTy,
        name: &str,
    ) -> Result<PointerValue<'ctx>> {
        let llvm_ty = self.llvm_int_type_for_width(elem_ty.width);
        unsafe { self.builder.build_gep(llvm_ty, base, &[index], name) }
            .map_err(|e| CodegenError::Llvm(e.to_string()))
    }

    fn emit_vec_primitive(
        &mut self,
        prim: VecPrim,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<CompiledValue<'ctx>> {
        match prim {
            VecPrim::Vec {
                op: VecOp::Alloc, ..
            } => Err(CodegenError::Unsupported(
                "@<ty>vec-alloc must appear as the direct RHS of a `let` binding",
            )),
            VecPrim::Vec { ty, op: VecOp::Len } => {
                let (_, len) = self.resolve_vec_arg(args[0], env, ty)?;
                Ok(CompiledValue::int(len))
            }
            VecPrim::Vec { ty, op: VecOp::Get } => self
                .emit_vec_get(ty, args, env, cur_fn)
                .map(CompiledValue::int),
            VecPrim::Vec { ty, op: VecOp::Set } => self
                .emit_vec_set(ty, args, env, cur_fn)
                .map(CompiledValue::int),
            VecPrim::U8Vec(U8VecOp::Fill) => self
                .emit_u8vec_fill(args, env, cur_fn)
                .map(CompiledValue::int),
            VecPrim::U8Vec(U8VecOp::Copy) => self
                .emit_u8vec_copy(args, env, cur_fn)
                .map(CompiledValue::int),
            VecPrim::U8Vec(U8VecOp::Slice) => Err(CodegenError::Unsupported(
                "@u8vec-slice must appear as the direct RHS of a `let` binding",
            )),
            VecPrim::U8Vec(U8VecOp::Eq) => self
                .emit_u8vec_eq(args, env, cur_fn)
                .map(CompiledValue::int),
            VecPrim::U8Vec(U8VecOp::Scan) => self
                .emit_u8vec_scan(args, env, cur_fn)
                .map(CompiledValue::int),
            VecPrim::U8VecBus(U8VecBusOp::Load { ty, endian }) => self
                .emit_u8vec_load(ty, endian, args, env, cur_fn)
                .map(CompiledValue::int),
            VecPrim::U8VecBus(U8VecBusOp::Store { ty, endian }) => self
                .emit_u8vec_store(ty, endian, args, env, cur_fn)
                .map(CompiledValue::int),
        }
    }

    /// `@<ty>vec-get v i` → bounds-check, load element, extend to i64.
    fn emit_vec_get(
        &mut self,
        elem_ty: FixedIntTy,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let (ptr, len) = self.resolve_vec_arg(args[0], env, elem_ty)?;
        let idx = self.compile_expr(args[1], env, cur_fn)?;
        let oob = self
            .builder
            .build_int_compare(IntPredicate::UGE, idx, len, "vec_get_oob")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.emit_bounds_trap(cur_fn, oob, "vec_get")?;
        let elem_ptr = self.vec_element_ptr(ptr, idx, elem_ty, "vec_get_ptr")?;
        let llvm_ty = self.llvm_int_type_for_width(elem_ty.width);
        let loaded = self
            .builder
            .build_load(llvm_ty, elem_ptr, "vec_get")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_int_value();
        if elem_ty.width == 64 {
            return Ok(loaded);
        }
        let i64_t = self.context.i64_type();
        let widened = if elem_ty.sign == IntSign::Signed {
            self.builder.build_int_s_extend(loaded, i64_t, "vec_get_sx")
        } else {
            self.builder.build_int_z_extend(loaded, i64_t, "vec_get_zx")
        }
        .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        Ok(widened)
    }

    /// `@<ty>vec-set v i x` → bounds-check, truncate, store; return 0.
    fn emit_vec_set(
        &mut self,
        elem_ty: FixedIntTy,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let (ptr, len) = self.resolve_vec_arg(args[0], env, elem_ty)?;
        let idx = self.compile_expr(args[1], env, cur_fn)?;
        let val = self.compile_expr(args[2], env, cur_fn)?;
        let oob = self
            .builder
            .build_int_compare(IntPredicate::UGE, idx, len, "vec_set_oob")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.emit_bounds_trap(cur_fn, oob, "vec_set")?;
        let elem_ptr = self.vec_element_ptr(ptr, idx, elem_ty, "vec_set_ptr")?;
        let llvm_ty = self.llvm_int_type_for_width(elem_ty.width);
        let narrow = if elem_ty.width == 64 {
            val
        } else {
            self.builder
                .build_int_truncate(val, llvm_ty, "vec_set_tr")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?
        };
        self.builder
            .build_store(elem_ptr, narrow)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        Ok(self.context.i64_type().const_zero())
    }

    /// `@u8vec-fill v off len byte` → bounds-check then `llvm.memset`.
    fn emit_u8vec_fill(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let u8_ty = FixedIntTy::new(IntSign::Unsigned, 8);
        let (ptr, vlen) = self.resolve_vec_arg(args[0], env, u8_ty)?;
        let off = self.compile_expr(args[1], env, cur_fn)?;
        let len = self.compile_expr(args[2], env, cur_fn)?;
        let byte = self.compile_expr(args[3], env, cur_fn)?;
        self.check_range(cur_fn, off, len, vlen, "u8vec_fill")?;
        let dst = self.ptr_at(ptr, off, "u8vf_dst")?;
        let i8_t = self.context.i8_type();
        let byte_i8 = self
            .builder
            .build_int_truncate(byte, i8_t, "u8vf_byte")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let memset = self.llvm_memset();
        let false_val = self.context.bool_type().const_int(0, false);
        self.builder
            .build_call(
                memset,
                &[
                    BasicMetadataValueEnum::PointerValue(dst),
                    BasicMetadataValueEnum::IntValue(byte_i8),
                    BasicMetadataValueEnum::IntValue(len),
                    BasicMetadataValueEnum::IntValue(false_val),
                ],
                "u8vf_memset",
            )
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        Ok(self.context.i64_type().const_zero())
    }

    fn llvm_memset(&self) -> FunctionValue<'ctx> {
        let name = "llvm.memset.p0.i64";
        if let Some(f) = self.module.get_function(name) {
            return f;
        }
        let ptr_t = self.context.ptr_type(AddressSpace::default());
        let i8_t = self.context.i8_type();
        let i64_t = self.context.i64_type();
        let i1_t = self.context.bool_type();
        let void_t = self.context.void_type();
        let fn_ty = void_t.fn_type(
            &[
                BasicMetadataTypeEnum::PointerType(ptr_t),
                BasicMetadataTypeEnum::IntType(i8_t),
                BasicMetadataTypeEnum::IntType(i64_t),
                BasicMetadataTypeEnum::IntType(i1_t),
            ],
            false,
        );
        self.module.add_function(name, fn_ty, None)
    }

    /// Bounds-check `off + len <= vec_len` (also rejects negative off and
    /// negative len via unsigned comparison wrapping). Trap on failure.
    fn check_range(
        &mut self,
        cur_fn: FunctionValue<'ctx>,
        off: IntValue<'ctx>,
        len: IntValue<'ctx>,
        vec_len: IntValue<'ctx>,
        name: &str,
    ) -> Result<()> {
        let end = self
            .builder
            .build_int_add(off, len, &format!("{name}_end"))
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        // off < 0 (as i64) becomes huge as unsigned, so `end u_gt vec_len`
        // catches it. We also explicitly catch end < off (overflow wrap) by
        // requiring end >= off.
        let oob1 = self
            .builder
            .build_int_compare(IntPredicate::UGT, end, vec_len, &format!("{name}_oob"))
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let overflow = self
            .builder
            .build_int_compare(IntPredicate::ULT, end, off, &format!("{name}_ovf"))
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let cond = self
            .builder
            .build_or(oob1, overflow, &format!("{name}_bad"))
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.emit_bounds_trap(cur_fn, cond, name)
    }

    /// `@u8vec-copy dst dst-off src src-off len` → overlap-safe element copy.
    fn emit_u8vec_copy(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let u8_ty = FixedIntTy::new(IntSign::Unsigned, 8);
        let (dst_ptr, dst_len) = self.resolve_vec_arg(args[0], env, u8_ty)?;
        let dst_off = self.compile_expr(args[1], env, cur_fn)?;
        let (src_ptr, src_len) = self.resolve_vec_arg(args[2], env, u8_ty)?;
        let src_off = self.compile_expr(args[3], env, cur_fn)?;
        let len = self.compile_expr(args[4], env, cur_fn)?;
        self.check_range(cur_fn, dst_off, len, dst_len, "u8vec_copy_dst")?;
        self.check_range(cur_fn, src_off, len, src_len, "u8vec_copy_src")?;
        let dst = self.ptr_at(dst_ptr, dst_off, "u8vc_dst")?;
        let src = self.ptr_at(src_ptr, src_off, "u8vc_src")?;
        let false_val = self.context.bool_type().const_int(0, false);
        let memmove = self.llvm_memmove();
        self.builder
            .build_call(
                memmove,
                &[
                    BasicMetadataValueEnum::PointerValue(dst),
                    BasicMetadataValueEnum::PointerValue(src),
                    BasicMetadataValueEnum::IntValue(len),
                    BasicMetadataValueEnum::IntValue(false_val),
                ],
                "u8vc_memmove",
            )
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        Ok(self.context.i64_type().const_zero())
    }

    /// `@u8vec-eq a a-off b b-off len` → byte-for-byte equality, returns
    /// `1` if equal, `0` otherwise. Bounds-checked.
    fn emit_u8vec_eq(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let u8_ty = FixedIntTy::new(IntSign::Unsigned, 8);
        let (a_ptr, a_len) = self.resolve_vec_arg(args[0], env, u8_ty)?;
        let a_off = self.compile_expr(args[1], env, cur_fn)?;
        let (b_ptr, b_len) = self.resolve_vec_arg(args[2], env, u8_ty)?;
        let b_off = self.compile_expr(args[3], env, cur_fn)?;
        let len = self.compile_expr(args[4], env, cur_fn)?;
        self.check_range(cur_fn, a_off, len, a_len, "u8vec_eq_a")?;
        self.check_range(cur_fn, b_off, len, b_len, "u8vec_eq_b")?;

        let i64_t = self.context.i64_type();
        let zero64 = i64_t.const_zero();
        let one64 = i64_t.const_int(1, false);

        let entry_bb = self.builder.get_insert_block().unwrap();
        let hdr_bb = self.context.append_basic_block(cur_fn, "u8eq_hdr");
        let body_bb = self.context.append_basic_block(cur_fn, "u8eq_body");
        let diff_bb = self.context.append_basic_block(cur_fn, "u8eq_diff");
        let cont_bb = self.context.append_basic_block(cur_fn, "u8eq_cont");
        let exit_bb = self.context.append_basic_block(cur_fn, "u8eq_exit");

        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder.position_at_end(hdr_bb);
        let i_phi = self
            .builder
            .build_phi(i64_t, "u8eq_i")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let i_val = i_phi.as_basic_value().into_int_value();
        let more = self
            .builder
            .build_int_compare(IntPredicate::SLT, i_val, len, "u8eq_more")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(more, body_bb, exit_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(body_bb);
        let a_idx = self
            .builder
            .build_int_add(a_off, i_val, "u8eq_a_idx")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let b_idx = self
            .builder
            .build_int_add(b_off, i_val, "u8eq_b_idx")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let a_b = self.load_byte(a_ptr, a_idx, "u8eq_a_b")?;
        let b_b = self.load_byte(b_ptr, b_idx, "u8eq_b_b")?;
        let eq = self
            .builder
            .build_int_compare(IntPredicate::EQ, a_b, b_b, "u8eq_eq")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(eq, cont_bb, diff_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(diff_bb);
        self.builder
            .build_unconditional_branch(exit_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(cont_bb);
        let next_i = self
            .builder
            .build_int_add(i_val, one64, "u8eq_next")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let cont_end = self.builder.get_insert_block().unwrap();
        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        i_phi.add_incoming(&[
            (&zero64 as &dyn BasicValue<'ctx>, entry_bb),
            (&next_i as &dyn BasicValue<'ctx>, cont_end),
        ]);

        self.builder.position_at_end(exit_bb);
        let result = self
            .builder
            .build_phi(i64_t, "u8eq_res")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        result.add_incoming(&[
            (&one64 as &dyn BasicValue<'ctx>, hdr_bb),
            (&zero64 as &dyn BasicValue<'ctx>, diff_bb),
        ]);
        Ok(result.as_basic_value().into_int_value())
    }

    /// `@u8vec-scan v off len byte` → first index of `byte` in
    /// `[off, off+len)` measured as an absolute index, or `off+len` if not
    /// found. Bounds-checked.
    fn emit_u8vec_scan(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let u8_ty = FixedIntTy::new(IntSign::Unsigned, 8);
        let (ptr, vlen) = self.resolve_vec_arg(args[0], env, u8_ty)?;
        let off = self.compile_expr(args[1], env, cur_fn)?;
        let len = self.compile_expr(args[2], env, cur_fn)?;
        let target = self.compile_expr(args[3], env, cur_fn)?;
        self.check_range(cur_fn, off, len, vlen, "u8vec_scan")?;

        let i64_t = self.context.i64_type();
        let one64 = i64_t.const_int(1, false);
        let end = self
            .builder
            .build_int_add(off, len, "u8sc_end")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        let entry_bb = self.builder.get_insert_block().unwrap();
        let hdr_bb = self.context.append_basic_block(cur_fn, "u8sc_hdr");
        let body_bb = self.context.append_basic_block(cur_fn, "u8sc_body");
        let hit_bb = self.context.append_basic_block(cur_fn, "u8sc_hit");
        let cont_bb = self.context.append_basic_block(cur_fn, "u8sc_cont");
        let exit_bb = self.context.append_basic_block(cur_fn, "u8sc_exit");

        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder.position_at_end(hdr_bb);
        let i_phi = self
            .builder
            .build_phi(i64_t, "u8sc_i")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let i_val = i_phi.as_basic_value().into_int_value();
        let more = self
            .builder
            .build_int_compare(IntPredicate::SLT, i_val, end, "u8sc_more")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(more, body_bb, exit_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(body_bb);
        let b = self.load_byte(ptr, i_val, "u8sc_b")?;
        let eq = self
            .builder
            .build_int_compare(IntPredicate::EQ, b, target, "u8sc_eq")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(eq, hit_bb, cont_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(hit_bb);
        self.builder
            .build_unconditional_branch(exit_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(cont_bb);
        let next_i = self
            .builder
            .build_int_add(i_val, one64, "u8sc_next")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let cont_end = self.builder.get_insert_block().unwrap();
        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        i_phi.add_incoming(&[
            (&off as &dyn BasicValue<'ctx>, entry_bb),
            (&next_i as &dyn BasicValue<'ctx>, cont_end),
        ]);

        self.builder.position_at_end(exit_bb);
        let res = self
            .builder
            .build_phi(i64_t, "u8sc_res")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        res.add_incoming(&[
            (&end as &dyn BasicValue<'ctx>, hdr_bb),
            (&i_val as &dyn BasicValue<'ctx>, hit_bb),
        ]);
        Ok(res.as_basic_value().into_int_value())
    }

    /// `@u8vec-load-<W>-<endian> v off` → assemble W bytes from `v[off..]`
    /// in the requested byte order into a zero-extended i64 value.
    fn emit_u8vec_load(
        &mut self,
        ty: FixedIntTy,
        endian: FixedEndian,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let u8_ty = FixedIntTy::new(IntSign::Unsigned, 8);
        let (ptr, vlen) = self.resolve_vec_arg(args[0], env, u8_ty)?;
        let off = self.compile_expr(args[1], env, cur_fn)?;
        let i64_t = self.context.i64_type();
        let width_bytes = i64_t.const_int(u64::from(ty.width / 8), false);
        self.check_range(cur_fn, off, width_bytes, vlen, "u8vec_load")?;

        let one64 = i64_t.const_int(1, false);
        let mut acc = i64_t.const_zero();
        let bytes = (ty.width / 8) as i64;
        for i in 0..bytes {
            let byte_off = self
                .builder
                .build_int_add(off, i64_t.const_int(i as u64, false), "u8vl_off")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
            let b = self.load_byte(ptr, byte_off, "u8vl_b")?;
            let shift_amount = match endian {
                FixedEndian::Little => i64_t.const_int((i as u64) * 8, false),
                FixedEndian::Big => i64_t.const_int(((bytes - 1 - i) as u64) * 8, false),
            };
            let shifted = self
                .builder
                .build_left_shift(b, shift_amount, "u8vl_sh")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
            acc = self
                .builder
                .build_or(acc, shifted, "u8vl_acc")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        }
        // Suppress unused-variable warning for one64 if the loop didn't use it.
        let _ = one64;
        // Result is already i64; for u16/u32 it's mask-low; for u64 it's all bits.
        self.normalize_fixed_int(acc, ty)
    }

    /// `@u8vec-store-<W>-<endian> v off x` → decompose `x` into W bytes and
    /// write them starting at `v[off]`.
    fn emit_u8vec_store(
        &mut self,
        ty: FixedIntTy,
        endian: FixedEndian,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let u8_ty = FixedIntTy::new(IntSign::Unsigned, 8);
        let (ptr, vlen) = self.resolve_vec_arg(args[0], env, u8_ty)?;
        let off = self.compile_expr(args[1], env, cur_fn)?;
        let val = self.compile_expr(args[2], env, cur_fn)?;
        let i64_t = self.context.i64_type();
        let width_bytes = i64_t.const_int(u64::from(ty.width / 8), false);
        self.check_range(cur_fn, off, width_bytes, vlen, "u8vec_store")?;

        let bytes = (ty.width / 8) as i64;
        for i in 0..bytes {
            let shift_amount = match endian {
                FixedEndian::Little => i64_t.const_int((i as u64) * 8, false),
                FixedEndian::Big => i64_t.const_int(((bytes - 1 - i) as u64) * 8, false),
            };
            let byte_val = self
                .builder
                .build_right_shift(val, shift_amount, false, "u8vs_sh")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
            let byte_off = self
                .builder
                .build_int_add(off, i64_t.const_int(i as u64, false), "u8vs_off")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
            self.store_byte(ptr, byte_off, byte_val)?;
        }
        Ok(i64_t.const_zero())
    }

    /// `@map src count f out` → for each `i`, `out[i] = f(src[i])`; return 0.
    fn emit_map_i64(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let src = self.compile_i64_vec_arg(args[0], env)?;
        let count = self.compile_expr(args[1], env, cur_fn)?;
        let callback = self.compile_value_expr(args[2], env, cur_fn)?;
        let out = self.compile_i64_vec_arg(args[3], env)?;

        let i64_t = self.context.i64_type();
        let zero64 = i64_t.const_zero();
        let one64 = i64_t.const_int(1, false);

        let entry_bb = self.builder.get_insert_block().unwrap();
        let hdr_bb = self.context.append_basic_block(cur_fn, "map_i64_hdr");
        let body_bb = self.context.append_basic_block(cur_fn, "map_i64_body");
        let ret_bb = self.context.append_basic_block(cur_fn, "map_i64_ret");

        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(hdr_bb);
        let i_phi = self
            .builder
            .build_phi(i64_t, "map_i64_i")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let i_val = i_phi.as_basic_value().into_int_value();
        let more = self
            .builder
            .build_int_compare(IntPredicate::SLT, i_val, count, "map_i64_more")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(more, body_bb, ret_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(body_bb);
        let elem = self.load_i64(src, i_val, "map_i64_elem")?;
        let mapped = self
            .call_closure_value(callback.clone(), &CompiledValue::int(elem))?
            .into_int()?;
        self.store_i64(out, i_val, mapped)?;
        let i_next = self
            .builder
            .build_int_add(i_val, one64, "map_i64_i_next")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        i_phi.add_incoming(&[
            (&zero64 as &dyn BasicValue<'ctx>, entry_bb),
            (&i_next as &dyn BasicValue<'ctx>, body_bb),
        ]);

        self.builder.position_at_end(ret_bb);
        Ok(zero64)
    }

    /// `@fold src count init f` → accumulator-first fold: `f acc src[i]`.
    fn emit_fold_i64(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let src = self.compile_i64_vec_arg(args[0], env)?;
        let count = self.compile_expr(args[1], env, cur_fn)?;
        let init = self.compile_expr(args[2], env, cur_fn)?;
        let callback = self.compile_value_expr(args[3], env, cur_fn)?;

        let i64_t = self.context.i64_type();
        let zero64 = i64_t.const_zero();
        let one64 = i64_t.const_int(1, false);

        let entry_bb = self.builder.get_insert_block().unwrap();
        let hdr_bb = self.context.append_basic_block(cur_fn, "fold_i64_hdr");
        let body_bb = self.context.append_basic_block(cur_fn, "fold_i64_body");
        let ret_bb = self.context.append_basic_block(cur_fn, "fold_i64_ret");

        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(hdr_bb);
        let i_phi = self
            .builder
            .build_phi(i64_t, "fold_i64_i")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let acc_phi = self
            .builder
            .build_phi(i64_t, "fold_i64_acc")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let i_val = i_phi.as_basic_value().into_int_value();
        let acc_val = acc_phi.as_basic_value().into_int_value();
        let more = self
            .builder
            .build_int_compare(IntPredicate::SLT, i_val, count, "fold_i64_more")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(more, body_bb, ret_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(body_bb);
        let elem = self.load_i64(src, i_val, "fold_i64_elem")?;
        let partial = self.call_closure_value(callback.clone(), &CompiledValue::int(acc_val))?;
        let acc_next = self
            .call_closure_value(partial, &CompiledValue::int(elem))?
            .into_int()?;
        let i_next = self
            .builder
            .build_int_add(i_val, one64, "fold_i64_i_next")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        i_phi.add_incoming(&[
            (&zero64 as &dyn BasicValue<'ctx>, entry_bb),
            (&i_next as &dyn BasicValue<'ctx>, body_bb),
        ]);
        acc_phi.add_incoming(&[
            (&init as &dyn BasicValue<'ctx>, entry_bb),
            (&acc_next as &dyn BasicValue<'ctx>, body_bb),
        ]);

        self.builder.position_at_end(ret_bb);
        Ok(acc_val)
    }

    /// `@for-each src count f` → call `f(src[i])` for each element; return 0.
    fn emit_for_each_i64(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let src = self.compile_i64_vec_arg(args[0], env)?;
        let count = self.compile_expr(args[1], env, cur_fn)?;
        let callback = self.compile_value_expr(args[2], env, cur_fn)?;

        let i64_t = self.context.i64_type();
        let zero64 = i64_t.const_zero();
        let one64 = i64_t.const_int(1, false);

        let entry_bb = self.builder.get_insert_block().unwrap();
        let hdr_bb = self.context.append_basic_block(cur_fn, "foreach_i64_hdr");
        let body_bb = self.context.append_basic_block(cur_fn, "foreach_i64_body");
        let ret_bb = self.context.append_basic_block(cur_fn, "foreach_i64_ret");

        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(hdr_bb);
        let i_phi = self
            .builder
            .build_phi(i64_t, "foreach_i64_i")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let i_val = i_phi.as_basic_value().into_int_value();
        let more = self
            .builder
            .build_int_compare(IntPredicate::SLT, i_val, count, "foreach_i64_more")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(more, body_bb, ret_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(body_bb);
        let elem = self.load_i64(src, i_val, "foreach_i64_elem")?;
        let _ = self.call_closure_value(callback.clone(), &CompiledValue::int(elem))?;
        let i_next = self
            .builder
            .build_int_add(i_val, one64, "foreach_i64_i_next")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        i_phi.add_incoming(&[
            (&zero64 as &dyn BasicValue<'ctx>, entry_bb),
            (&i_next as &dyn BasicValue<'ctx>, body_bb),
        ]);

        self.builder.position_at_end(ret_bb);
        Ok(zero64)
    }

    /// `@loop init step` → bounded-stack iteration (ADR 0093).
    ///
    /// Lowers as a labeled basic-block loop with a PHI on the state value.
    /// Immediate step lambdas lower inline as direct callbacks (ADR 0096);
    /// non-immediate callback values still use the closure ABI. The back-edge
    /// is an LLVM `br`, not a function call, so the loop's stack is bounded
    /// regardless of iteration count.
    fn emit_loop(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<CompiledValue<'ctx>> {
        let init = self.compile_value_expr(args[0], env, cur_fn)?;
        let state_ty = init.ty.clone();
        // The step callback's type is `S -> {tag : Int, value : S}`.  Pass
        // the expected closure type explicitly so that a bare `lambda s. ...`
        // body sees the correct state type for `s` (and any record
        // projections on it) without needing an annotation.
        let directive_ty = ValueTy::Record(vec![
            ("tag".to_string(), ValueTy::Int),
            ("value".to_string(), state_ty.clone()),
        ]);
        let direct_step_body = direct_loop_callback_body(args[1]);
        let step_fn_ty = ValueTy::Fn(Box::new(state_ty.clone()), Box::new(directive_ty.clone()));
        let step = if direct_step_body.is_some() {
            None
        } else {
            let step = match args[1] {
                Node::Lam { .. } | Node::Ann { .. } => {
                    self.compile_closure_value(args[1], Some(step_fn_ty.clone()), env, cur_fn)?
                }
                _ => self.compile_value_expr(args[1], env, cur_fn)?,
            };

            let ValueTy::Fn(step_arg_ty, step_ret_ty) = step.ty.clone() else {
                return Err(CodegenError::AppNonFunction);
            };
            if step_arg_ty.as_ref() != &state_ty {
                return Err(CodegenError::ValueTypeMismatch {
                    expected: state_ty.to_string(),
                    actual: step_arg_ty.to_string(),
                });
            }
            let ValueTy::Record(ret_fields) = step_ret_ty.as_ref() else {
                return Err(CodegenError::Unsupported(
                    "@loop step callback must return a {tag, value} record",
                ));
            };
            if ret_fields.len() != 2
                || ret_fields[0].0 != "tag"
                || ret_fields[0].1 != ValueTy::Int
                || ret_fields[1].0 != "value"
                || ret_fields[1].1 != state_ty
            {
                return Err(CodegenError::Unsupported(
                    "@loop step callback must return {tag : Int, value : S}",
                ));
            }
            Some(step)
        };

        let entry_bb = self.builder.get_insert_block().unwrap();
        let hdr_bb = self.context.append_basic_block(cur_fn, "loop_hdr");
        let cont_bb = self.context.append_basic_block(cur_fn, "loop_cont");
        let exit_bb = self.context.append_basic_block(cur_fn, "loop_exit");

        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(hdr_bb);
        let state_llvm_ty = self.llvm_type(&state_ty)?;
        let state_phi = self
            .builder
            .build_phi(state_llvm_ty, "loop_state")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let state_val = state_phi.as_basic_value();
        let budget_phi = if let Some(loop_step_budget) = self.options.loop_step_budget {
            let budget_ok_bb = self.context.append_basic_block(cur_fn, "loop_budget_ok");
            let budget_phi = self
                .builder
                .build_phi(self.context.i64_type(), "loop_budget")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
            let remaining = budget_phi.as_basic_value().into_int_value();
            let has_budget = self
                .builder
                .build_int_compare(
                    IntPredicate::NE,
                    remaining,
                    self.context.i64_type().const_zero(),
                    "loop_has_budget",
                )
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
            let budget_exhausted_bb = self
                .context
                .append_basic_block(cur_fn, "loop_budget_exhausted");
            self.builder
                .build_conditional_branch(has_budget, budget_ok_bb, budget_exhausted_bb)
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
            self.builder.position_at_end(budget_exhausted_bb);
            self.emit_loop_budget_exceeded(cur_fn)?;
            budget_phi.add_incoming(&[(
                &self.context.i64_type().const_int(loop_step_budget, false)
                    as &dyn BasicValue<'ctx>,
                entry_bb,
            )]);
            self.builder.position_at_end(budget_ok_bb);
            Some(budget_phi)
        } else {
            None
        };

        let state_compiled = CompiledValue {
            ty: state_ty.clone(),
            value: state_val,
        };
        let rec = if let Some(body) = direct_step_body {
            let mut body_env = Vec::with_capacity(env.len() + 1);
            body_env.push(Binding::Value(state_compiled));
            body_env.extend_from_slice(env);
            let rec = self.compile_value_expr(body, &body_env, cur_fn)?;
            if rec.ty != directive_ty {
                return Err(CodegenError::ValueTypeMismatch {
                    expected: directive_ty.to_string(),
                    actual: rec.ty.to_string(),
                });
            }
            rec
        } else {
            self.call_closure_value(
                step.as_ref()
                    .expect("non-direct loop callback was compiled above")
                    .clone(),
                &state_compiled,
            )?
        };
        let rec_struct = rec.value.into_struct_value();
        let tag = self
            .builder
            .build_extract_value(rec_struct, 0, "loop_tag")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_int_value();
        let next_value = self
            .builder
            .build_extract_value(rec_struct, 1, "loop_value")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        let zero64 = self.context.i64_type().const_zero();
        let is_step = self
            .builder
            .build_int_compare(IntPredicate::EQ, tag, zero64, "loop_is_step")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(is_step, cont_bb, exit_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(cont_bb);
        if let Some(budget_phi) = &budget_phi {
            let budget_next = self
                .builder
                .build_int_sub(
                    budget_phi.as_basic_value().into_int_value(),
                    self.context.i64_type().const_int(1, false),
                    "loop_budget_next",
                )
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
            budget_phi.add_incoming(&[(&budget_next as &dyn BasicValue<'ctx>, cont_bb)]);
        }
        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        state_phi.add_incoming(&[
            (&init.value as &dyn BasicValue<'ctx>, entry_bb),
            (&next_value as &dyn BasicValue<'ctx>, cont_bb),
        ]);

        self.builder.position_at_end(exit_bb);
        Ok(CompiledValue {
            ty: state_ty,
            value: next_value,
        })
    }

    fn emit_loop_budget_exceeded(&mut self, _cur_fn: FunctionValue<'ctx>) -> Result<()> {
        let f = self.libc_exit();
        let code = self
            .context
            .i32_type()
            .const_int(TEST_STEP_BUDGET_EXIT_CODE, false);
        self.builder
            .build_call(
                f,
                &[BasicMetadataValueEnum::IntValue(code)],
                "loop_budget_exit",
            )
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unreachable()
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        Ok(())
    }

    /// `@loop-step value` / `@loop-exit value` → build the loop-directive
    /// record `{ tag, value }` (ADR 0093). `tag` is 0 for step, 1 for exit.
    fn emit_loop_directive(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
        tag: u64,
    ) -> Result<CompiledValue<'ctx>> {
        let value = self.compile_value_expr(args[0], env, cur_fn)?;
        let value_ty = value.ty.clone();
        let ty = ValueTy::Record(vec![
            ("tag".to_string(), ValueTy::Int),
            ("value".to_string(), value_ty),
        ]);
        let struct_ty = self.llvm_struct_type(&ty)?;
        let tag_val = self.context.i64_type().const_int(tag, false);
        let mut aggregate = struct_ty.get_undef();
        aggregate = self
            .builder
            .build_insert_value(aggregate, tag_val, 0, "loop_dir_tag")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_struct_value();
        aggregate = self
            .builder
            .build_insert_value(aggregate, value.value, 1, "loop_dir_value")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_struct_value();
        Ok(CompiledValue {
            ty,
            value: aggregate.into(),
        })
    }

    /// `@sort-i64 vec count` → stable insertion sort of `vec[0..count)`; return 0.
    fn emit_sort_i64(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let vec = self.compile_i64_vec_arg(args[0], env)?;
        let count = self.compile_expr(args[1], env, cur_fn)?;

        let i64_t = self.context.i64_type();
        let zero64 = i64_t.const_zero();
        let one64 = i64_t.const_int(1, false);

        let entry_bb = self.builder.get_insert_block().unwrap();
        let outer_hdr_bb = self
            .context
            .append_basic_block(cur_fn, "sort_i64_outer_hdr");
        let outer_body_bb = self
            .context
            .append_basic_block(cur_fn, "sort_i64_outer_body");
        let inner_hdr_bb = self
            .context
            .append_basic_block(cur_fn, "sort_i64_inner_hdr");
        let inner_check_bb = self
            .context
            .append_basic_block(cur_fn, "sort_i64_inner_chk");
        let shift_bb = self.context.append_basic_block(cur_fn, "sort_i64_shift");
        let inner_done_bb = self
            .context
            .append_basic_block(cur_fn, "sort_i64_inner_done");
        let outer_cont_bb = self
            .context
            .append_basic_block(cur_fn, "sort_i64_outer_cont");
        let ret_bb = self.context.append_basic_block(cur_fn, "sort_i64_ret");

        self.builder
            .build_unconditional_branch(outer_hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(outer_hdr_bb);
        let i_phi = self
            .builder
            .build_phi(i64_t, "sort_i64_i")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let i_val = i_phi.as_basic_value().into_int_value();
        let more = self
            .builder
            .build_int_compare(IntPredicate::SLT, i_val, count, "sort_i64_more")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(more, outer_body_bb, ret_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(outer_body_bb);
        let key = self.load_i64(vec, i_val, "sort_i64_key")?;
        self.builder
            .build_unconditional_branch(inner_hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(inner_hdr_bb);
        let j_phi = self
            .builder
            .build_phi(i64_t, "sort_i64_j")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let j_val = j_phi.as_basic_value().into_int_value();
        let has_prev = self
            .builder
            .build_int_compare(IntPredicate::SGT, j_val, zero64, "sort_i64_has_prev")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(has_prev, inner_check_bb, inner_done_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(inner_check_bb);
        let prev_idx = self
            .builder
            .build_int_sub(j_val, one64, "sort_i64_prev_idx")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let prev = self.load_i64(vec, prev_idx, "sort_i64_prev")?;
        let should_shift = self
            .builder
            .build_int_compare(IntPredicate::SGT, prev, key, "sort_i64_should_shift")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(should_shift, shift_bb, inner_done_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(shift_bb);
        self.store_i64(vec, j_val, prev)?;
        self.builder
            .build_unconditional_branch(inner_hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        j_phi.add_incoming(&[
            (&i_val as &dyn BasicValue<'ctx>, outer_body_bb),
            (&prev_idx as &dyn BasicValue<'ctx>, shift_bb),
        ]);

        self.builder.position_at_end(inner_done_bb);
        self.store_i64(vec, j_val, key)?;
        self.builder
            .build_unconditional_branch(outer_cont_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(outer_cont_bb);
        let i_next = self
            .builder
            .build_int_add(i_val, one64, "sort_i64_i_next")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(outer_hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        i_phi.add_incoming(&[
            (&one64 as &dyn BasicValue<'ctx>, entry_bb),
            (&i_next as &dyn BasicValue<'ctx>, outer_cont_bb),
        ]);

        self.builder.position_at_end(ret_bb);
        Ok(zero64)
    }

    /// `@stable-sort-pairs-i64 keys values count` → stable key sort; return 0.
    fn emit_stable_sort_pairs_i64(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let keys = self.compile_i64_vec_arg(args[0], env)?;
        let values = self.compile_i64_vec_arg(args[1], env)?;
        let count = self.compile_expr(args[2], env, cur_fn)?;

        let i64_t = self.context.i64_type();
        let zero64 = i64_t.const_zero();
        let one64 = i64_t.const_int(1, false);

        let entry_bb = self.builder.get_insert_block().unwrap();
        let outer_hdr_bb = self
            .context
            .append_basic_block(cur_fn, "sort_pair_outer_hdr");
        let outer_body_bb = self
            .context
            .append_basic_block(cur_fn, "sort_pair_outer_body");
        let inner_hdr_bb = self
            .context
            .append_basic_block(cur_fn, "sort_pair_inner_hdr");
        let inner_check_bb = self
            .context
            .append_basic_block(cur_fn, "sort_pair_inner_chk");
        let shift_bb = self.context.append_basic_block(cur_fn, "sort_pair_shift");
        let inner_done_bb = self
            .context
            .append_basic_block(cur_fn, "sort_pair_inner_done");
        let outer_cont_bb = self
            .context
            .append_basic_block(cur_fn, "sort_pair_outer_cont");
        let ret_bb = self.context.append_basic_block(cur_fn, "sort_pair_ret");

        self.builder
            .build_unconditional_branch(outer_hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(outer_hdr_bb);
        let i_phi = self
            .builder
            .build_phi(i64_t, "sort_pair_i")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let i_val = i_phi.as_basic_value().into_int_value();
        let more = self
            .builder
            .build_int_compare(IntPredicate::SLT, i_val, count, "sort_pair_more")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(more, outer_body_bb, ret_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(outer_body_bb);
        let key = self.load_i64(keys, i_val, "sort_pair_key")?;
        let value = self.load_i64(values, i_val, "sort_pair_value")?;
        self.builder
            .build_unconditional_branch(inner_hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(inner_hdr_bb);
        let j_phi = self
            .builder
            .build_phi(i64_t, "sort_pair_j")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let j_val = j_phi.as_basic_value().into_int_value();
        let has_prev = self
            .builder
            .build_int_compare(IntPredicate::SGT, j_val, zero64, "sort_pair_has_prev")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(has_prev, inner_check_bb, inner_done_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(inner_check_bb);
        let prev_idx = self
            .builder
            .build_int_sub(j_val, one64, "sort_pair_prev_idx")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let prev_key = self.load_i64(keys, prev_idx, "sort_pair_prev_key")?;
        let should_shift = self
            .builder
            .build_int_compare(IntPredicate::SGT, prev_key, key, "sort_pair_should_shift")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(should_shift, shift_bb, inner_done_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(shift_bb);
        let prev_value = self.load_i64(values, prev_idx, "sort_pair_prev_value")?;
        self.store_i64(keys, j_val, prev_key)?;
        self.store_i64(values, j_val, prev_value)?;
        self.builder
            .build_unconditional_branch(inner_hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        j_phi.add_incoming(&[
            (&i_val as &dyn BasicValue<'ctx>, outer_body_bb),
            (&prev_idx as &dyn BasicValue<'ctx>, shift_bb),
        ]);

        self.builder.position_at_end(inner_done_bb);
        self.store_i64(keys, j_val, key)?;
        self.store_i64(values, j_val, value)?;
        self.builder
            .build_unconditional_branch(outer_cont_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(outer_cont_bb);
        let i_next = self
            .builder
            .build_int_add(i_val, one64, "sort_pair_i_next")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(outer_hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        i_phi.add_incoming(&[
            (&one64 as &dyn BasicValue<'ctx>, entry_bb),
            (&i_next as &dyn BasicValue<'ctx>, outer_cont_bb),
        ]);

        self.builder.position_at_end(ret_bb);
        Ok(zero64)
    }

    /// `@sort-ranges-by-bytes text table count` → stable byte-lexicographic range sort; return 0.
    fn emit_sort_ranges_by_bytes(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let text = self.compile_buf_ptr_arg(args[0], env)?;
        let table = self.compile_i64_vec_arg(args[1], env)?;
        let count = self.compile_expr(args[2], env, cur_fn)?;

        let i64_t = self.context.i64_type();
        let zero64 = i64_t.const_zero();
        let one64 = i64_t.const_int(1, false);

        let entry_bb = self.builder.get_insert_block().unwrap();
        let outer_hdr_bb = self
            .context
            .append_basic_block(cur_fn, "sort_ranges_outer_hdr");
        let outer_body_bb = self
            .context
            .append_basic_block(cur_fn, "sort_ranges_outer_body");
        let inner_hdr_bb = self
            .context
            .append_basic_block(cur_fn, "sort_ranges_inner_hdr");
        let inner_check_bb = self
            .context
            .append_basic_block(cur_fn, "sort_ranges_inner_chk");
        let shift_bb = self.context.append_basic_block(cur_fn, "sort_ranges_shift");
        let inner_done_bb = self
            .context
            .append_basic_block(cur_fn, "sort_ranges_inner_done");
        let outer_cont_bb = self
            .context
            .append_basic_block(cur_fn, "sort_ranges_outer_cont");
        let ret_bb = self.context.append_basic_block(cur_fn, "sort_ranges_ret");

        self.builder
            .build_unconditional_branch(outer_hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(outer_hdr_bb);
        let i_phi = self
            .builder
            .build_phi(i64_t, "sort_ranges_i")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let i_val = i_phi.as_basic_value().into_int_value();
        let more = self
            .builder
            .build_int_compare(IntPredicate::SLT, i_val, count, "sort_ranges_more")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(more, outer_body_bb, ret_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(outer_body_bb);
        let (key_start, key_len) = self.load_range_pair(table, i_val, "sort_ranges_key")?;
        self.builder
            .build_unconditional_branch(inner_hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(inner_hdr_bb);
        let j_phi = self
            .builder
            .build_phi(i64_t, "sort_ranges_j")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let j_val = j_phi.as_basic_value().into_int_value();
        let has_prev = self
            .builder
            .build_int_compare(IntPredicate::SGT, j_val, zero64, "sort_ranges_has_prev")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(has_prev, inner_check_bb, inner_done_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(inner_check_bb);
        let prev_idx = self
            .builder
            .build_int_sub(j_val, one64, "sort_ranges_prev_idx")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let should_shift = self.emit_range_bytes_gt_key(
            text,
            table,
            prev_idx,
            (key_start, key_len),
            cur_fn,
            "sort_ranges_prev",
        )?;
        self.builder
            .build_conditional_branch(should_shift, shift_bb, inner_done_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(shift_bb);
        let (prev_start, prev_len) = self.load_range_pair(table, prev_idx, "sort_ranges_shift")?;
        self.store_range_pair(table, j_val, prev_start, prev_len)?;
        self.builder
            .build_unconditional_branch(inner_hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        j_phi.add_incoming(&[
            (&i_val as &dyn BasicValue<'ctx>, outer_body_bb),
            (&prev_idx as &dyn BasicValue<'ctx>, shift_bb),
        ]);

        self.builder.position_at_end(inner_done_bb);
        self.store_range_pair(table, j_val, key_start, key_len)?;
        self.builder
            .build_unconditional_branch(outer_cont_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(outer_cont_bb);
        let i_next = self
            .builder
            .build_int_add(i_val, one64, "sort_ranges_i_next")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(outer_hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        i_phi.add_incoming(&[
            (&one64 as &dyn BasicValue<'ctx>, entry_bb),
            (&i_next as &dyn BasicValue<'ctx>, outer_cont_bb),
        ]);

        self.builder.position_at_end(ret_bb);
        Ok(zero64)
    }

    /// `@lower-bound-i64 vec count value` → first sorted index with `vec[i] >= value`.
    fn emit_lower_bound_i64(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let vec = self.compile_i64_vec_arg(args[0], env)?;
        let count = self.compile_expr(args[1], env, cur_fn)?;
        let value = self.compile_expr(args[2], env, cur_fn)?;

        let i64_t = self.context.i64_type();
        let zero64 = i64_t.const_zero();
        let one64 = i64_t.const_int(1, false);
        let two64 = i64_t.const_int(2, false);

        let entry_bb = self.builder.get_insert_block().unwrap();
        let hdr_bb = self.context.append_basic_block(cur_fn, "lower_bound_hdr");
        let check_bb = self.context.append_basic_block(cur_fn, "lower_bound_chk");
        let move_low_bb = self
            .context
            .append_basic_block(cur_fn, "lower_bound_move_low");
        let move_high_bb = self
            .context
            .append_basic_block(cur_fn, "lower_bound_move_high");
        let ret_bb = self.context.append_basic_block(cur_fn, "lower_bound_ret");

        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(hdr_bb);
        let lo_phi = self
            .builder
            .build_phi(i64_t, "lower_bound_lo")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let hi_phi = self
            .builder
            .build_phi(i64_t, "lower_bound_hi")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let lo_val = lo_phi.as_basic_value().into_int_value();
        let hi_val = hi_phi.as_basic_value().into_int_value();
        let done = self
            .builder
            .build_int_compare(IntPredicate::SGE, lo_val, hi_val, "lower_bound_done")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(done, ret_bb, check_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(check_bb);
        let span = self
            .builder
            .build_int_add(lo_val, hi_val, "lower_bound_span")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let mid = self
            .builder
            .build_int_signed_div(span, two64, "lower_bound_mid")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let mid_val = self.load_i64(vec, mid, "lower_bound_mid_val")?;
        let is_less = self
            .builder
            .build_int_compare(IntPredicate::SLT, mid_val, value, "lower_bound_is_less")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(is_less, move_low_bb, move_high_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(move_low_bb);
        let lo_next = self
            .builder
            .build_int_add(mid, one64, "lower_bound_lo_next")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(move_high_bb);
        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        lo_phi.add_incoming(&[
            (&zero64 as &dyn BasicValue<'ctx>, entry_bb),
            (&lo_next as &dyn BasicValue<'ctx>, move_low_bb),
            (&lo_val as &dyn BasicValue<'ctx>, move_high_bb),
        ]);
        hi_phi.add_incoming(&[
            (&count as &dyn BasicValue<'ctx>, entry_bb),
            (&hi_val as &dyn BasicValue<'ctx>, move_low_bb),
            (&mid as &dyn BasicValue<'ctx>, move_high_bb),
        ]);

        self.builder.position_at_end(ret_bb);
        Ok(lo_val)
    }

    /// `@count-equal-ranges text table count out` → triples `(start, len, run_count)`.
    fn emit_count_equal_ranges(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let text = self.compile_buf_ptr_arg(args[0], env)?;
        let table = self.compile_i64_vec_arg(args[1], env)?;
        let count = self.compile_expr(args[2], env, cur_fn)?;
        let out = self.compile_i64_vec_arg(args[3], env)?;

        let i64_t = self.context.i64_type();
        let zero64 = i64_t.const_zero();
        let one64 = i64_t.const_int(1, false);

        let entry_bb = self.builder.get_insert_block().unwrap();
        let outer_hdr_bb = self
            .context
            .append_basic_block(cur_fn, "count_ranges_outer_hdr");
        let outer_body_bb = self
            .context
            .append_basic_block(cur_fn, "count_ranges_outer_body");
        let inner_hdr_bb = self
            .context
            .append_basic_block(cur_fn, "count_ranges_inner_hdr");
        let inner_check_bb = self
            .context
            .append_basic_block(cur_fn, "count_ranges_inner_chk");
        let inner_cont_bb = self
            .context
            .append_basic_block(cur_fn, "count_ranges_inner_cont");
        let emit_bb = self.context.append_basic_block(cur_fn, "count_ranges_emit");
        let ret_bb = self.context.append_basic_block(cur_fn, "count_ranges_ret");

        self.builder
            .build_unconditional_branch(outer_hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(outer_hdr_bb);
        let i_phi = self
            .builder
            .build_phi(i64_t, "count_ranges_i")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let out_count_phi = self
            .builder
            .build_phi(i64_t, "count_ranges_out_count")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let i_val = i_phi.as_basic_value().into_int_value();
        let out_count_val = out_count_phi.as_basic_value().into_int_value();
        let done = self
            .builder
            .build_int_compare(IntPredicate::SGE, i_val, count, "count_ranges_done")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(done, ret_bb, outer_body_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(outer_body_bb);
        let (key_start, key_len) = self.load_range_pair(table, i_val, "count_ranges_key")?;
        let first_j = self
            .builder
            .build_int_add(i_val, one64, "count_ranges_first_j")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(inner_hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(inner_hdr_bb);
        let j_phi = self
            .builder
            .build_phi(i64_t, "count_ranges_j")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let j_val = j_phi.as_basic_value().into_int_value();
        let inner_done = self
            .builder
            .build_int_compare(IntPredicate::SGE, j_val, count, "count_ranges_inner_done")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(inner_done, emit_bb, inner_check_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(inner_check_bb);
        let equal = self.emit_range_bytes_eq_key(
            text,
            table,
            j_val,
            (key_start, key_len),
            cur_fn,
            "count_ranges_eq",
        )?;
        self.builder
            .build_conditional_branch(equal, inner_cont_bb, emit_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(inner_cont_bb);
        let j_next = self
            .builder
            .build_int_add(j_val, one64, "count_ranges_j_next")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(inner_hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        j_phi.add_incoming(&[
            (&first_j as &dyn BasicValue<'ctx>, outer_body_bb),
            (&j_next as &dyn BasicValue<'ctx>, inner_cont_bb),
        ]);

        self.builder.position_at_end(emit_bb);
        let run_count = self
            .builder
            .build_int_sub(j_val, i_val, "count_ranges_run_count")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.store_range_count_triple(out, out_count_val, key_start, key_len, run_count)?;
        let out_count_next = self
            .builder
            .build_int_add(out_count_val, one64, "count_ranges_out_next")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(outer_hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        i_phi.add_incoming(&[
            (&zero64 as &dyn BasicValue<'ctx>, entry_bb),
            (&j_val as &dyn BasicValue<'ctx>, emit_bb),
        ]);
        out_count_phi.add_incoming(&[
            (&zero64 as &dyn BasicValue<'ctx>, entry_bb),
            (&out_count_next as &dyn BasicValue<'ctx>, emit_bb),
        ]);

        self.builder.position_at_end(ret_bb);
        Ok(out_count_val)
    }

    /// `@dedup-adjacent-ranges text table count out` → adjacent unique start/length pairs.
    fn emit_dedup_adjacent_ranges(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let text = self.compile_buf_ptr_arg(args[0], env)?;
        let table = self.compile_i64_vec_arg(args[1], env)?;
        let count = self.compile_expr(args[2], env, cur_fn)?;
        let out = self.compile_i64_vec_arg(args[3], env)?;

        let i64_t = self.context.i64_type();
        let zero64 = i64_t.const_zero();
        let one64 = i64_t.const_int(1, false);

        let entry_bb = self.builder.get_insert_block().unwrap();
        let hdr_bb = self.context.append_basic_block(cur_fn, "dedup_ranges_hdr");
        let body_bb = self.context.append_basic_block(cur_fn, "dedup_ranges_body");
        let compare_bb = self
            .context
            .append_basic_block(cur_fn, "dedup_ranges_compare");
        let copy_bb = self.context.append_basic_block(cur_fn, "dedup_ranges_copy");
        let skip_bb = self.context.append_basic_block(cur_fn, "dedup_ranges_skip");
        let ret_bb = self.context.append_basic_block(cur_fn, "dedup_ranges_ret");

        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(hdr_bb);
        let i_phi = self
            .builder
            .build_phi(i64_t, "dedup_ranges_i")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let out_count_phi = self
            .builder
            .build_phi(i64_t, "dedup_ranges_out_count")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let i_val = i_phi.as_basic_value().into_int_value();
        let out_count_val = out_count_phi.as_basic_value().into_int_value();
        let done = self
            .builder
            .build_int_compare(IntPredicate::SGE, i_val, count, "dedup_ranges_done")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(done, ret_bb, body_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(body_bb);
        let has_output = self
            .builder
            .build_int_compare(
                IntPredicate::SGT,
                out_count_val,
                zero64,
                "dedup_ranges_has_output",
            )
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(has_output, compare_bb, copy_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(compare_bb);
        let last_out = self
            .builder
            .build_int_sub(out_count_val, one64, "dedup_ranges_last_out")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let (key_start, key_len) = self.load_range_pair(out, last_out, "dedup_ranges_key")?;
        let equal = self.emit_range_bytes_eq_key(
            text,
            table,
            i_val,
            (key_start, key_len),
            cur_fn,
            "dedup_ranges_eq",
        )?;
        self.builder
            .build_conditional_branch(equal, skip_bb, copy_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(copy_bb);
        let (start, len) = self.load_range_pair(table, i_val, "dedup_ranges_copy")?;
        self.store_range_pair(out, out_count_val, start, len)?;
        let out_count_next = self
            .builder
            .build_int_add(out_count_val, one64, "dedup_ranges_out_next")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let i_next_copy = self
            .builder
            .build_int_add(i_val, one64, "dedup_ranges_i_next_copy")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(skip_bb);
        let i_next_skip = self
            .builder
            .build_int_add(i_val, one64, "dedup_ranges_i_next_skip")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        i_phi.add_incoming(&[
            (&zero64 as &dyn BasicValue<'ctx>, entry_bb),
            (&i_next_copy as &dyn BasicValue<'ctx>, copy_bb),
            (&i_next_skip as &dyn BasicValue<'ctx>, skip_bb),
        ]);
        out_count_phi.add_incoming(&[
            (&zero64 as &dyn BasicValue<'ctx>, entry_bb),
            (&out_count_next as &dyn BasicValue<'ctx>, copy_bb),
            (&out_count_val as &dyn BasicValue<'ctx>, skip_bb),
        ]);

        self.builder.position_at_end(ret_bb);
        Ok(out_count_val)
    }

    /// `@stdin-slurp buf cap` → repeatedly `read(0, buf+total, cap-total)` until
    /// EOF or `cap` bytes have been written; returns total bytes read (ADR 0067).
    fn emit_stdin_slurp(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let buf = self.compile_buf_ptr_arg(args[0], env)?;
        let cap = self.compile_expr(args[1], env, cur_fn)?;

        let i64_t = self.context.i64_type();
        let i32_t = self.context.i32_type();
        let zero64 = i64_t.const_zero();

        let entry_bb = self.builder.get_insert_block().unwrap();
        let hdr_bb = self.context.append_basic_block(cur_fn, "slurp_hdr");
        let body_bb = self.context.append_basic_block(cur_fn, "slurp_body");
        let cont_bb = self.context.append_basic_block(cur_fn, "slurp_cont");
        let exit_bb = self.context.append_basic_block(cur_fn, "slurp_exit");

        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // header: total = phi(0, total + n)
        self.builder.position_at_end(hdr_bb);
        let total_phi = self
            .builder
            .build_phi(i64_t, "slurp_total")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let total_val = total_phi.as_basic_value().into_int_value();
        let room = self
            .builder
            .build_int_compare(IntPredicate::SLT, total_val, cap, "slurp_room")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(room, body_bb, exit_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // body: n = read(0, buf + total, cap - total)
        self.builder.position_at_end(body_bb);
        let dst_ptr = self.ptr_at(buf, total_val, "slurp_dst")?;
        let remaining = self
            .builder
            .build_int_sub(cap, total_val, "slurp_rem")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let zero_fd = i32_t.const_zero();
        let read_fn = self.libc_read();
        let n_call = self
            .builder
            .build_call(
                read_fn,
                &[
                    BasicMetadataValueEnum::IntValue(zero_fd),
                    BasicMetadataValueEnum::PointerValue(dst_ptr),
                    BasicMetadataValueEnum::IntValue(remaining),
                ],
                "slurp_n",
            )
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let n = n_call
            .try_as_basic_value()
            .basic()
            .ok_or_else(|| CodegenError::Llvm("read returned no value".into()))?
            .into_int_value();
        let progress = self
            .builder
            .build_int_compare(IntPredicate::SGT, n, zero64, "slurp_progress")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(progress, cont_bb, exit_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // cont: total += n; loop
        self.builder.position_at_end(cont_bb);
        let total_next = self
            .builder
            .build_int_add(total_val, n, "slurp_total_next")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        total_phi.add_incoming(&[
            (&zero64 as &dyn BasicValue<'ctx>, entry_bb),
            (&total_next as &dyn BasicValue<'ctx>, cont_bb),
        ]);

        // exit: return whichever total was current when we stopped.
        // Both predecessors (hdr full-cap, body short-read) carry total_val.
        self.builder.position_at_end(exit_bb);
        let res_phi = self
            .builder
            .build_phi(i64_t, "slurp_res")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        res_phi.add_incoming(&[
            (&total_val as &dyn BasicValue<'ctx>, hdr_bb),
            (&total_val as &dyn BasicValue<'ctx>, body_bb),
        ]);
        Ok(res_phi.as_basic_value().into_int_value())
    }

    /// `@write-range fd buf off len` → repeatedly `write(fd, buf+off+written,
    /// len-written)` until `len` bytes have been emitted or a write returns
    /// `<= 0`; returns 0 (ADR 0067).
    fn emit_write_range(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let fd = self.compile_expr(args[0], env, cur_fn)?;
        let buf = self.compile_buf_ptr_arg(args[1], env)?;
        let off = self.compile_expr(args[2], env, cur_fn)?;
        let len = self.compile_expr(args[3], env, cur_fn)?;

        let i64_t = self.context.i64_type();
        let zero64 = i64_t.const_zero();

        let fd_i32 = self
            .builder
            .build_int_truncate(fd, self.context.i32_type(), "wrng_fd_i32")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        let entry_bb = self.builder.get_insert_block().unwrap();
        let hdr_bb = self.context.append_basic_block(cur_fn, "wrng_hdr");
        let body_bb = self.context.append_basic_block(cur_fn, "wrng_body");
        let cont_bb = self.context.append_basic_block(cur_fn, "wrng_cont");
        let exit_bb = self.context.append_basic_block(cur_fn, "wrng_exit");

        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(hdr_bb);
        let written_phi = self
            .builder
            .build_phi(i64_t, "wrng_written")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let written_val = written_phi.as_basic_value().into_int_value();
        let more = self
            .builder
            .build_int_compare(IntPredicate::SLT, written_val, len, "wrng_more")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(more, body_bb, exit_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // body: cur = off + written; remaining = len - written; n = write(...)
        self.builder.position_at_end(body_bb);
        let cur_off = self
            .builder
            .build_int_add(off, written_val, "wrng_cur_off")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let src_ptr = self.ptr_at(buf, cur_off, "wrng_src")?;
        let remaining = self
            .builder
            .build_int_sub(len, written_val, "wrng_rem")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let write_fn = self.libc_write();
        let n_call = self
            .builder
            .build_call(
                write_fn,
                &[
                    BasicMetadataValueEnum::IntValue(fd_i32),
                    BasicMetadataValueEnum::PointerValue(src_ptr),
                    BasicMetadataValueEnum::IntValue(remaining),
                ],
                "wrng_n",
            )
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let n = n_call
            .try_as_basic_value()
            .basic()
            .ok_or_else(|| CodegenError::Llvm("write returned no value".into()))?
            .into_int_value();
        let progress = self
            .builder
            .build_int_compare(IntPredicate::SGT, n, zero64, "wrng_progress")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(progress, cont_bb, exit_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(cont_bb);
        let written_next = self
            .builder
            .build_int_add(written_val, n, "wrng_written_next")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        written_phi.add_incoming(&[
            (&zero64 as &dyn BasicValue<'ctx>, entry_bb),
            (&written_next as &dyn BasicValue<'ctx>, cont_bb),
        ]);

        self.builder.position_at_end(exit_bb);
        Ok(zero64)
    }

    /// `@buf-rev buf off len` → reverse `buf[off..off+len)` in place; return 0 (ADR 0067).
    fn emit_buf_rev(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let buf = self.compile_buf_ptr_arg(args[0], env)?;
        let off = self.compile_expr(args[1], env, cur_fn)?;
        let len = self.compile_expr(args[2], env, cur_fn)?;

        let i64_t = self.context.i64_type();
        let one64 = i64_t.const_int(1, false);

        // j_init = off + len - 1
        let off_plus_len = self
            .builder
            .build_int_add(off, len, "rev_end")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let j_init = self
            .builder
            .build_int_sub(off_plus_len, one64, "rev_j_init")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        let entry_bb = self.builder.get_insert_block().unwrap();
        let hdr_bb = self.context.append_basic_block(cur_fn, "rev_hdr");
        let body_bb = self.context.append_basic_block(cur_fn, "rev_body");
        let exit_bb = self.context.append_basic_block(cur_fn, "rev_exit");

        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // header: i = phi(off, i+1); j = phi(j_init, j-1); loop while i < j
        self.builder.position_at_end(hdr_bb);
        let i_phi = self
            .builder
            .build_phi(i64_t, "rev_i")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let j_phi = self
            .builder
            .build_phi(i64_t, "rev_j")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let i_val = i_phi.as_basic_value().into_int_value();
        let j_val = j_phi.as_basic_value().into_int_value();
        let cont = self
            .builder
            .build_int_compare(IntPredicate::SLT, i_val, j_val, "rev_cont")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(cont, body_bb, exit_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // body: swap buf[i] and buf[j]
        self.builder.position_at_end(body_bb);
        let a = self.load_byte(buf, i_val, "rev_a")?;
        let b = self.load_byte(buf, j_val, "rev_b")?;
        self.store_byte(buf, i_val, b)?;
        self.store_byte(buf, j_val, a)?;
        let i_next = self
            .builder
            .build_int_add(i_val, one64, "rev_i_next")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let j_next = self
            .builder
            .build_int_sub(j_val, one64, "rev_j_next")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        i_phi.add_incoming(&[
            (&off as &dyn BasicValue<'ctx>, entry_bb),
            (&i_next as &dyn BasicValue<'ctx>, body_bb),
        ]);
        j_phi.add_incoming(&[
            (&j_init as &dyn BasicValue<'ctx>, entry_bb),
            (&j_next as &dyn BasicValue<'ctx>, body_bb),
        ]);

        self.builder.position_at_end(exit_bb);
        Ok(i64_t.const_zero())
    }

    /// `@ascii-tolower b` / `@ascii-toupper b` → if `b` is in the source
    /// case range, return the shifted byte; otherwise return `b` unchanged
    /// (ADR 0068). Pure straight-line `icmp + and + select`.
    fn emit_ascii_case_shift(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
        case: AsciiCase,
    ) -> Result<IntValue<'ctx>> {
        let b = self.compile_expr(args[0], env, cur_fn)?;
        let i64_t = self.context.i64_type();
        let (lo, hi, delta, lo_name, hi_name, in_name, shifted_name, sel_name) = match case {
            // tolower: A..=Z (65..=90) → +32
            AsciiCase::Lower => (
                i64_t.const_int(65, false),
                i64_t.const_int(90, false),
                i64_t.const_int(32, false),
                "tol_lo",
                "tol_hi",
                "tol_in",
                "tol_shift",
                "tol_res",
            ),
            // toupper: a..=z (97..=122) → -32
            AsciiCase::Upper => (
                i64_t.const_int(97, false),
                i64_t.const_int(122, false),
                i64_t.const_int(32, false),
                "tou_lo",
                "tou_hi",
                "tou_in",
                "tou_shift",
                "tou_res",
            ),
        };
        let ge_lo = self
            .builder
            .build_int_compare(IntPredicate::SGE, b, lo, lo_name)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let le_hi = self
            .builder
            .build_int_compare(IntPredicate::SLE, b, hi, hi_name)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let in_range = self
            .builder
            .build_and(ge_lo, le_hi, in_name)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let shifted = match case {
            AsciiCase::Lower => self
                .builder
                .build_int_add(b, delta, shifted_name)
                .map_err(|e| CodegenError::Llvm(e.to_string()))?,
            AsciiCase::Upper => self
                .builder
                .build_int_sub(b, delta, shifted_name)
                .map_err(|e| CodegenError::Llvm(e.to_string()))?,
        };
        Ok(self
            .builder
            .build_select(in_range, shifted, b, sel_name)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_int_value())
    }

    /// `@ascii-is-alpha b` → 1 if `b` is in 65..=90 or 97..=122, else 0
    /// (ADR 0068). Two range checks, OR'd, zero-extended.
    fn emit_ascii_is_alpha(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let b = self.compile_expr(args[0], env, cur_fn)?;
        let i64_t = self.context.i64_type();
        let upper_lo = i64_t.const_int(65, false);
        let upper_hi = i64_t.const_int(90, false);
        let lower_lo = i64_t.const_int(97, false);
        let lower_hi = i64_t.const_int(122, false);

        let ge_u = self
            .builder
            .build_int_compare(IntPredicate::SGE, b, upper_lo, "ia_uge")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let le_u = self
            .builder
            .build_int_compare(IntPredicate::SLE, b, upper_hi, "ia_ule")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let in_upper = self
            .builder
            .build_and(ge_u, le_u, "ia_upper")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        let ge_l = self
            .builder
            .build_int_compare(IntPredicate::SGE, b, lower_lo, "ia_lge")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let le_l = self
            .builder
            .build_int_compare(IntPredicate::SLE, b, lower_hi, "ia_lle")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let in_lower = self
            .builder
            .build_and(ge_l, le_l, "ia_lower")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        let any = self
            .builder
            .build_or(in_upper, in_lower, "ia_any")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_int_z_extend(any, i64_t, "ia_res")
            .map_err(|e| CodegenError::Llvm(e.to_string()))
    }

    /// `@ascii-is-digit b` → 1 if `b` is in 48..=57, else 0 (ADR 0068).
    fn emit_ascii_is_digit(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let b = self.compile_expr(args[0], env, cur_fn)?;
        let i64_t = self.context.i64_type();
        let lo = i64_t.const_int(48, false);
        let hi = i64_t.const_int(57, false);
        let ge = self
            .builder
            .build_int_compare(IntPredicate::SGE, b, lo, "id_ge")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let le = self
            .builder
            .build_int_compare(IntPredicate::SLE, b, hi, "id_le")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let in_range = self
            .builder
            .build_and(ge, le, "id_in")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_int_z_extend(in_range, i64_t, "id_res")
            .map_err(|e| CodegenError::Llvm(e.to_string()))
    }

    /// `@ascii-is-space b` → 1 if `b` is one of {9, 10, 11, 12, 13, 32}, else 0
    /// (ADR 0068). Implemented as `(b == 32) || (b >= 9 && b <= 13)`.
    fn emit_ascii_is_space(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let b = self.compile_expr(args[0], env, cur_fn)?;
        let i64_t = self.context.i64_type();
        let lo = i64_t.const_int(9, false);
        let hi = i64_t.const_int(13, false);
        let sp = i64_t.const_int(32, false);
        let ge = self
            .builder
            .build_int_compare(IntPredicate::SGE, b, lo, "is_ge")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let le = self
            .builder
            .build_int_compare(IntPredicate::SLE, b, hi, "is_le")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let in_ctrl = self
            .builder
            .build_and(ge, le, "is_ctrl")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let is_sp = self
            .builder
            .build_int_compare(IntPredicate::EQ, b, sp, "is_sp")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let any = self
            .builder
            .build_or(in_ctrl, is_sp, "is_any")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_int_z_extend(any, i64_t, "is_res")
            .map_err(|e| CodegenError::Llvm(e.to_string()))
    }

    /// `@utf8-decode buf off` → decode one UTF-8 codepoint and return packed
    /// `cp * 8 + byte_len`, or 0 on malformed input (ADR 0069).
    ///
    /// Branches on the lead byte's high bits, validates each continuation
    /// byte against `10xxxxxx`, rejects overlong encodings, surrogate
    /// codepoints (0xD800..=0xDFFF), and codepoints above 0x10FFFF.
    fn emit_utf8_decode(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let buf = self.compile_buf_ptr_arg(args[0], env)?;
        let off = self.compile_expr(args[1], env, cur_fn)?;

        let i64_t = self.context.i64_type();
        let zero64 = i64_t.const_zero();
        let one64 = i64_t.const_int(1, false);
        let two64 = i64_t.const_int(2, false);
        let three64 = i64_t.const_int(3, false);
        let four64 = i64_t.const_int(4, false);
        let eight64 = i64_t.const_int(8, false);
        let mask_3f = i64_t.const_int(0x3F, false);
        let mask_c0 = i64_t.const_int(0xC0, false);
        let cont_marker = i64_t.const_int(0x80, false);

        let lead_b0 = self.load_byte(buf, off, "ud_b0")?;

        // Block layout: dispatch ladder + per-width validate/compute + final phi.
        let ascii_bb = self.context.append_basic_block(cur_fn, "ud_ascii");
        let dispatch_2plus_bb = self.context.append_basic_block(cur_fn, "ud_2plus");
        let lead_2or_more_bb = self.context.append_basic_block(cur_fn, "ud_2orMore");
        let decode_2_bb = self.context.append_basic_block(cur_fn, "ud_dec2");
        let compute_2_bb = self.context.append_basic_block(cur_fn, "ud_cmp2");
        let accept_2_bb = self.context.append_basic_block(cur_fn, "ud_acc2");
        let lead_3or_more_bb = self.context.append_basic_block(cur_fn, "ud_3orMore");
        let decode_3_bb = self.context.append_basic_block(cur_fn, "ud_dec3");
        let compute_3_bb = self.context.append_basic_block(cur_fn, "ud_cmp3");
        let accept_3_bb = self.context.append_basic_block(cur_fn, "ud_acc3");
        let lead_4_bb = self.context.append_basic_block(cur_fn, "ud_4");
        let decode_4_bb = self.context.append_basic_block(cur_fn, "ud_dec4");
        let compute_4_bb = self.context.append_basic_block(cur_fn, "ud_cmp4");
        let accept_4_bb = self.context.append_basic_block(cur_fn, "ud_acc4");
        let malformed_bb = self.context.append_basic_block(cur_fn, "ud_malformed");
        let exit_bb = self.context.append_basic_block(cur_fn, "ud_exit");

        // entry: ASCII (b0 < 0x80)?
        let is_ascii = self
            .builder
            .build_int_compare(IntPredicate::ULT, lead_b0, cont_marker, "ud_is_ascii")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(is_ascii, ascii_bb, dispatch_2plus_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // ascii_bb: result = b0 * 8 + 1
        self.builder.position_at_end(ascii_bb);
        let result_ascii_mul = self
            .builder
            .build_int_nsw_mul(lead_b0, eight64, "ud_ascii_cp")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let result_ascii = self
            .builder
            .build_int_nsw_add(result_ascii_mul, one64, "ud_ascii_res")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(exit_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // dispatch_2plus_bb: lone continuation? (b0 < 0xC0 ⇒ in 0x80..=0xBF)
        self.builder.position_at_end(dispatch_2plus_bb);
        let is_lone_cont = self
            .builder
            .build_int_compare(IntPredicate::ULT, lead_b0, mask_c0, "ud_lone_cont")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(is_lone_cont, malformed_bb, lead_2or_more_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // lead_2or_more_bb: 2-byte? (b0 < 0xE0 ⇒ in 0xC0..=0xDF)
        self.builder.position_at_end(lead_2or_more_bb);
        let lead_3plus_threshold = i64_t.const_int(0xE0, false);
        let is_2 = self
            .builder
            .build_int_compare(IntPredicate::ULT, lead_b0, lead_3plus_threshold, "ud_is_2")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(is_2, decode_2_bb, lead_3or_more_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // decode_2_bb: read b1, validate 10xxxxxx
        self.builder.position_at_end(decode_2_bb);
        let off_plus_1 = self
            .builder
            .build_int_nsw_add(off, one64, "ud_off1")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let b1_2 = self.load_byte(buf, off_plus_1, "ud_b1_2")?;
        let cont1_2_ok = self.utf8_continuation_ok(b1_2, mask_c0, cont_marker, "ud_c1_2")?;
        self.builder
            .build_conditional_branch(cont1_2_ok, compute_2_bb, malformed_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // compute_2_bb: cp = ((b0 & 0x1F) << 6) | (b1 & 0x3F); reject overlong
        self.builder.position_at_end(compute_2_bb);
        let mask_1f = i64_t.const_int(0x1F, false);
        let six64 = i64_t.const_int(6, false);
        let b0_low_2 = self
            .builder
            .build_and(lead_b0, mask_1f, "ud_b0lo_2")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let b0_shift_2 = self
            .builder
            .build_left_shift(b0_low_2, six64, "ud_b0sh_2")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let b1_low_2 = self
            .builder
            .build_and(b1_2, mask_3f, "ud_b1lo_2")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let cp_2 = self
            .builder
            .build_or(b0_shift_2, b1_low_2, "ud_cp_2")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let min_2 = i64_t.const_int(0x80, false);
        let not_overlong_2 = self
            .builder
            .build_int_compare(IntPredicate::SGE, cp_2, min_2, "ud_not_over_2")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(not_overlong_2, accept_2_bb, malformed_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // accept_2_bb: result = cp * 8 + 2
        self.builder.position_at_end(accept_2_bb);
        let cp_2_mul = self
            .builder
            .build_int_nsw_mul(cp_2, eight64, "ud_cp2_mul")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let result_2 = self
            .builder
            .build_int_nsw_add(cp_2_mul, two64, "ud_res_2")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(exit_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // lead_3or_more_bb: 3-byte? (b0 < 0xF0 ⇒ in 0xE0..=0xEF)
        self.builder.position_at_end(lead_3or_more_bb);
        let lead_4plus_threshold = i64_t.const_int(0xF0, false);
        let is_3 = self
            .builder
            .build_int_compare(IntPredicate::ULT, lead_b0, lead_4plus_threshold, "ud_is_3")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(is_3, decode_3_bb, lead_4_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // decode_3_bb: read b1, b2 and validate both
        self.builder.position_at_end(decode_3_bb);
        let off_plus_1_b3 = self
            .builder
            .build_int_nsw_add(off, one64, "ud_off1_b3")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let off_plus_2_b3 = self
            .builder
            .build_int_nsw_add(off, two64, "ud_off2_b3")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let b1_3 = self.load_byte(buf, off_plus_1_b3, "ud_b1_3")?;
        let b2_3 = self.load_byte(buf, off_plus_2_b3, "ud_b2_3")?;
        let c1_3 = self.utf8_continuation_ok(b1_3, mask_c0, cont_marker, "ud_c1_3")?;
        let c2_3 = self.utf8_continuation_ok(b2_3, mask_c0, cont_marker, "ud_c2_3")?;
        let conts_3_ok = self
            .builder
            .build_and(c1_3, c2_3, "ud_conts_3")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(conts_3_ok, compute_3_bb, malformed_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // compute_3_bb: assemble cp; reject overlong (<0x800) and surrogates (0xD800..=0xDFFF)
        self.builder.position_at_end(compute_3_bb);
        let mask_0f = i64_t.const_int(0x0F, false);
        let twelve64 = i64_t.const_int(12, false);
        let b0_low_3 = self
            .builder
            .build_and(lead_b0, mask_0f, "ud_b0lo_3")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let b0_shift_3 = self
            .builder
            .build_left_shift(b0_low_3, twelve64, "ud_b0sh_3")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let b1_low_3 = self
            .builder
            .build_and(b1_3, mask_3f, "ud_b1lo_3")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let b1_shift_3 = self
            .builder
            .build_left_shift(b1_low_3, six64, "ud_b1sh_3")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let b2_low_3 = self
            .builder
            .build_and(b2_3, mask_3f, "ud_b2lo_3")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let cp_3_partial = self
            .builder
            .build_or(b0_shift_3, b1_shift_3, "ud_cp3_p")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let cp_3 = self
            .builder
            .build_or(cp_3_partial, b2_low_3, "ud_cp_3")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let min_3 = i64_t.const_int(0x800, false);
        let surrogate_lo = i64_t.const_int(0xD800, false);
        let surrogate_hi = i64_t.const_int(0xDFFF, false);
        let not_overlong_3 = self
            .builder
            .build_int_compare(IntPredicate::SGE, cp_3, min_3, "ud_not_over_3")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let ge_surrogate = self
            .builder
            .build_int_compare(IntPredicate::SGE, cp_3, surrogate_lo, "ud_ge_sur")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let le_surrogate = self
            .builder
            .build_int_compare(IntPredicate::SLE, cp_3, surrogate_hi, "ud_le_sur")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let is_surrogate = self
            .builder
            .build_and(ge_surrogate, le_surrogate, "ud_is_sur")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let not_surrogate = self
            .builder
            .build_not(is_surrogate, "ud_not_sur")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let ok_3 = self
            .builder
            .build_and(not_overlong_3, not_surrogate, "ud_ok_3")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(ok_3, accept_3_bb, malformed_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // accept_3_bb: result = cp * 8 + 3
        self.builder.position_at_end(accept_3_bb);
        let cp_3_mul = self
            .builder
            .build_int_nsw_mul(cp_3, eight64, "ud_cp3_mul")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let result_3 = self
            .builder
            .build_int_nsw_add(cp_3_mul, three64, "ud_res_3")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(exit_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // lead_4_bb: 4-byte? (b0 < 0xF8 ⇒ in 0xF0..=0xF7); else malformed
        self.builder.position_at_end(lead_4_bb);
        let lead_5plus_threshold = i64_t.const_int(0xF8, false);
        let is_4 = self
            .builder
            .build_int_compare(IntPredicate::ULT, lead_b0, lead_5plus_threshold, "ud_is_4")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(is_4, decode_4_bb, malformed_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // decode_4_bb: read b1, b2, b3; validate
        self.builder.position_at_end(decode_4_bb);
        let off_plus_1_b4 = self
            .builder
            .build_int_nsw_add(off, one64, "ud_off1_b4")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let off_plus_2_b4 = self
            .builder
            .build_int_nsw_add(off, two64, "ud_off2_b4")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let off_plus_3_b4 = self
            .builder
            .build_int_nsw_add(off, three64, "ud_off3_b4")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let b1_4 = self.load_byte(buf, off_plus_1_b4, "ud_b1_4")?;
        let b2_4 = self.load_byte(buf, off_plus_2_b4, "ud_b2_4")?;
        let b3_4 = self.load_byte(buf, off_plus_3_b4, "ud_b3_4")?;
        let c1_4 = self.utf8_continuation_ok(b1_4, mask_c0, cont_marker, "ud_c1_4")?;
        let c2_4 = self.utf8_continuation_ok(b2_4, mask_c0, cont_marker, "ud_c2_4")?;
        let c3_4 = self.utf8_continuation_ok(b3_4, mask_c0, cont_marker, "ud_c3_4")?;
        let c12_4 = self
            .builder
            .build_and(c1_4, c2_4, "ud_c12_4")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let conts_4_ok = self
            .builder
            .build_and(c12_4, c3_4, "ud_conts_4")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(conts_4_ok, compute_4_bb, malformed_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // compute_4_bb: assemble cp; require 0x10000 <= cp <= 0x10FFFF
        self.builder.position_at_end(compute_4_bb);
        let mask_07 = i64_t.const_int(0x07, false);
        let eighteen64 = i64_t.const_int(18, false);
        let b0_low_4 = self
            .builder
            .build_and(lead_b0, mask_07, "ud_b0lo_4")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let b0_shift_4 = self
            .builder
            .build_left_shift(b0_low_4, eighteen64, "ud_b0sh_4")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let b1_low_4 = self
            .builder
            .build_and(b1_4, mask_3f, "ud_b1lo_4")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let b1_shift_4 = self
            .builder
            .build_left_shift(b1_low_4, twelve64, "ud_b1sh_4")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let b2_low_4 = self
            .builder
            .build_and(b2_4, mask_3f, "ud_b2lo_4")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let b2_shift_4 = self
            .builder
            .build_left_shift(b2_low_4, six64, "ud_b2sh_4")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let b3_low_4 = self
            .builder
            .build_and(b3_4, mask_3f, "ud_b3lo_4")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let cp_4_p1 = self
            .builder
            .build_or(b0_shift_4, b1_shift_4, "ud_cp4_p1")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let cp_4_p2 = self
            .builder
            .build_or(cp_4_p1, b2_shift_4, "ud_cp4_p2")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let cp_4 = self
            .builder
            .build_or(cp_4_p2, b3_low_4, "ud_cp_4")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let min_4 = i64_t.const_int(0x10000, false);
        let max_cp = i64_t.const_int(0x10FFFF, false);
        let ge_min_4 = self
            .builder
            .build_int_compare(IntPredicate::SGE, cp_4, min_4, "ud_ge_min_4")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let le_max_4 = self
            .builder
            .build_int_compare(IntPredicate::SLE, cp_4, max_cp, "ud_le_max_4")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let in_range_4 = self
            .builder
            .build_and(ge_min_4, le_max_4, "ud_in_range_4")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(in_range_4, accept_4_bb, malformed_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // accept_4_bb: result = cp * 8 + 4
        self.builder.position_at_end(accept_4_bb);
        let cp_4_mul = self
            .builder
            .build_int_nsw_mul(cp_4, eight64, "ud_cp4_mul")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let result_4 = self
            .builder
            .build_int_nsw_add(cp_4_mul, four64, "ud_res_4")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(exit_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // malformed_bb: result = 0
        self.builder.position_at_end(malformed_bb);
        self.builder
            .build_unconditional_branch(exit_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // exit_bb: phi over the five accept paths + malformed sentinel
        self.builder.position_at_end(exit_bb);
        let res_phi = self
            .builder
            .build_phi(i64_t, "ud_res")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        res_phi.add_incoming(&[
            (&result_ascii as &dyn BasicValue<'ctx>, ascii_bb),
            (&result_2 as &dyn BasicValue<'ctx>, accept_2_bb),
            (&result_3 as &dyn BasicValue<'ctx>, accept_3_bb),
            (&result_4 as &dyn BasicValue<'ctx>, accept_4_bb),
            (&zero64 as &dyn BasicValue<'ctx>, malformed_bb),
        ]);
        Ok(res_phi.as_basic_value().into_int_value())
    }

    /// Helper for UTF-8 continuation-byte validation: `(b & 0xC0) == 0x80`.
    fn utf8_continuation_ok(
        &mut self,
        b: IntValue<'ctx>,
        mask_c0: IntValue<'ctx>,
        cont_marker: IntValue<'ctx>,
        name: &str,
    ) -> Result<IntValue<'ctx>> {
        let masked = self
            .builder
            .build_and(b, mask_c0, name)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_int_compare(IntPredicate::EQ, masked, cont_marker, name)
            .map_err(|e| CodegenError::Llvm(e.to_string()))
    }

    /// `@utf8-encode buf off cp` → write the UTF-8 encoding of `cp` to
    /// `buf[off..off+n)` and return `n`. Returns 0 without writing for
    /// invalid codepoints (negative, surrogate, > 0x10FFFF). (ADR 0069)
    fn emit_utf8_encode(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let buf = self.compile_buf_ptr_arg(args[0], env)?;
        let off = self.compile_expr(args[1], env, cur_fn)?;
        let cp = self.compile_expr(args[2], env, cur_fn)?;

        let i64_t = self.context.i64_type();
        let zero64 = i64_t.const_zero();
        let one64 = i64_t.const_int(1, false);
        let two64 = i64_t.const_int(2, false);
        let three64 = i64_t.const_int(3, false);
        let four64 = i64_t.const_int(4, false);
        let mask_3f = i64_t.const_int(0x3F, false);
        let cont_marker = i64_t.const_int(0x80, false);

        let valid_bb = self.context.append_basic_block(cur_fn, "ue_valid");
        let reject_bb = self.context.append_basic_block(cur_fn, "ue_reject");
        let try_2_bb = self.context.append_basic_block(cur_fn, "ue_try2");
        let try_3_bb = self.context.append_basic_block(cur_fn, "ue_try3");
        let enc1_bb = self.context.append_basic_block(cur_fn, "ue_enc1");
        let enc2_bb = self.context.append_basic_block(cur_fn, "ue_enc2");
        let enc3_bb = self.context.append_basic_block(cur_fn, "ue_enc3");
        let enc4_bb = self.context.append_basic_block(cur_fn, "ue_enc4");
        let exit_bb = self.context.append_basic_block(cur_fn, "ue_exit");

        // Validity gate: cp ≥ 0 && cp ≤ 0x10FFFF && !(0xD800 ≤ cp ≤ 0xDFFF)
        let max_cp = i64_t.const_int(0x10FFFF, false);
        let surrogate_lo = i64_t.const_int(0xD800, false);
        let surrogate_hi = i64_t.const_int(0xDFFF, false);
        let cp_neg = self
            .builder
            .build_int_compare(IntPredicate::SLT, cp, zero64, "ue_neg")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let cp_too_high = self
            .builder
            .build_int_compare(IntPredicate::SGT, cp, max_cp, "ue_too_high")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let ge_sur = self
            .builder
            .build_int_compare(IntPredicate::SGE, cp, surrogate_lo, "ue_ge_sur")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let le_sur = self
            .builder
            .build_int_compare(IntPredicate::SLE, cp, surrogate_hi, "ue_le_sur")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let is_sur = self
            .builder
            .build_and(ge_sur, le_sur, "ue_is_sur")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let invalid_low = self
            .builder
            .build_or(cp_neg, cp_too_high, "ue_inv_lo")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let invalid = self
            .builder
            .build_or(invalid_low, is_sur, "ue_invalid")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(invalid, reject_bb, valid_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // valid_bb: ladder on cp size.
        self.builder.position_at_end(valid_bb);
        let thresh_2 = i64_t.const_int(0x80, false);
        let cp_lt_2 = self
            .builder
            .build_int_compare(IntPredicate::SLT, cp, thresh_2, "ue_lt_80")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(cp_lt_2, enc1_bb, try_2_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // try_2_bb
        self.builder.position_at_end(try_2_bb);
        let thresh_3 = i64_t.const_int(0x800, false);
        let cp_lt_3 = self
            .builder
            .build_int_compare(IntPredicate::SLT, cp, thresh_3, "ue_lt_800")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(cp_lt_3, enc2_bb, try_3_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // try_3_bb
        self.builder.position_at_end(try_3_bb);
        let thresh_4 = i64_t.const_int(0x10000, false);
        let cp_lt_4 = self
            .builder
            .build_int_compare(IntPredicate::SLT, cp, thresh_4, "ue_lt_10000")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(cp_lt_4, enc3_bb, enc4_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // enc1_bb: buf[off] = cp; return 1
        self.builder.position_at_end(enc1_bb);
        self.store_byte(buf, off, cp)?;
        self.builder
            .build_unconditional_branch(exit_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // enc2_bb
        self.builder.position_at_end(enc2_bb);
        let six64 = i64_t.const_int(6, false);
        let lead2_pre = self
            .builder
            .build_right_shift(cp, six64, false, "ue2_pre")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let mask_c0_marker = i64_t.const_int(0xC0, false);
        let lead2 = self
            .builder
            .build_or(lead2_pre, mask_c0_marker, "ue2_lead")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let cont2_low = self
            .builder
            .build_and(cp, mask_3f, "ue2_clo")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let cont2 = self
            .builder
            .build_or(cont2_low, cont_marker, "ue2_cont")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let off_plus_1_e2 = self
            .builder
            .build_int_nsw_add(off, one64, "ue2_off1")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.store_byte(buf, off, lead2)?;
        self.store_byte(buf, off_plus_1_e2, cont2)?;
        self.builder
            .build_unconditional_branch(exit_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // enc3_bb
        self.builder.position_at_end(enc3_bb);
        let twelve64 = i64_t.const_int(12, false);
        let lead3_pre = self
            .builder
            .build_right_shift(cp, twelve64, false, "ue3_pre")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let mask_e0_marker = i64_t.const_int(0xE0, false);
        let lead3 = self
            .builder
            .build_or(lead3_pre, mask_e0_marker, "ue3_lead")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let mid3_pre = self
            .builder
            .build_right_shift(cp, six64, false, "ue3_midpre")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let mid3_low = self
            .builder
            .build_and(mid3_pre, mask_3f, "ue3_midlo")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let mid3 = self
            .builder
            .build_or(mid3_low, cont_marker, "ue3_mid")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let cont3_low = self
            .builder
            .build_and(cp, mask_3f, "ue3_clo")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let cont3 = self
            .builder
            .build_or(cont3_low, cont_marker, "ue3_cont")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let off_plus_1_e3 = self
            .builder
            .build_int_nsw_add(off, one64, "ue3_off1")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let off_plus_2_e3 = self
            .builder
            .build_int_nsw_add(off, two64, "ue3_off2")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.store_byte(buf, off, lead3)?;
        self.store_byte(buf, off_plus_1_e3, mid3)?;
        self.store_byte(buf, off_plus_2_e3, cont3)?;
        self.builder
            .build_unconditional_branch(exit_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // enc4_bb
        self.builder.position_at_end(enc4_bb);
        let eighteen64 = i64_t.const_int(18, false);
        let lead4_pre = self
            .builder
            .build_right_shift(cp, eighteen64, false, "ue4_pre")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let mask_f0_marker = i64_t.const_int(0xF0, false);
        let lead4 = self
            .builder
            .build_or(lead4_pre, mask_f0_marker, "ue4_lead")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let mid_a_pre = self
            .builder
            .build_right_shift(cp, twelve64, false, "ue4_apre")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let mid_a_low = self
            .builder
            .build_and(mid_a_pre, mask_3f, "ue4_alo")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let mid_a = self
            .builder
            .build_or(mid_a_low, cont_marker, "ue4_a")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let mid_b_pre = self
            .builder
            .build_right_shift(cp, six64, false, "ue4_bpre")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let mid_b_low = self
            .builder
            .build_and(mid_b_pre, mask_3f, "ue4_blo")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let mid_b = self
            .builder
            .build_or(mid_b_low, cont_marker, "ue4_b")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let cont4_low = self
            .builder
            .build_and(cp, mask_3f, "ue4_clo")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let cont4 = self
            .builder
            .build_or(cont4_low, cont_marker, "ue4_c")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let off_plus_1_e4 = self
            .builder
            .build_int_nsw_add(off, one64, "ue4_off1")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let off_plus_2_e4 = self
            .builder
            .build_int_nsw_add(off, two64, "ue4_off2")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let off_plus_3_e4 = self
            .builder
            .build_int_nsw_add(off, three64, "ue4_off3")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.store_byte(buf, off, lead4)?;
        self.store_byte(buf, off_plus_1_e4, mid_a)?;
        self.store_byte(buf, off_plus_2_e4, mid_b)?;
        self.store_byte(buf, off_plus_3_e4, cont4)?;
        self.builder
            .build_unconditional_branch(exit_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // reject_bb
        self.builder.position_at_end(reject_bb);
        self.builder
            .build_unconditional_branch(exit_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        // exit_bb
        self.builder.position_at_end(exit_bb);
        let res_phi = self
            .builder
            .build_phi(i64_t, "ue_res")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        res_phi.add_incoming(&[
            (&one64 as &dyn BasicValue<'ctx>, enc1_bb),
            (&two64 as &dyn BasicValue<'ctx>, enc2_bb),
            (&three64 as &dyn BasicValue<'ctx>, enc3_bb),
            (&four64 as &dyn BasicValue<'ctx>, enc4_bb),
            (&zero64 as &dyn BasicValue<'ctx>, reject_bb),
        ]);
        Ok(res_phi.as_basic_value().into_int_value())
    }

    /// `@utf8-len cp` → byte length (1..=4) for valid codepoints, 0 otherwise.
    fn emit_utf8_len(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let cp = self.compile_expr(args[0], env, cur_fn)?;
        let i64_t = self.context.i64_type();
        let zero64 = i64_t.const_zero();
        let one64 = i64_t.const_int(1, false);
        let two64 = i64_t.const_int(2, false);
        let three64 = i64_t.const_int(3, false);
        let four64 = i64_t.const_int(4, false);

        let valid_bb = self.context.append_basic_block(cur_fn, "ul_valid");
        let try_2_bb = self.context.append_basic_block(cur_fn, "ul_try2");
        let try_3_bb = self.context.append_basic_block(cur_fn, "ul_try3");
        let len_1_bb = self.context.append_basic_block(cur_fn, "ul_len1");
        let len_2_bb = self.context.append_basic_block(cur_fn, "ul_len2");
        let len_3_bb = self.context.append_basic_block(cur_fn, "ul_len3");
        let len_4_bb = self.context.append_basic_block(cur_fn, "ul_len4");
        let reject_bb = self.context.append_basic_block(cur_fn, "ul_reject");
        let exit_bb = self.context.append_basic_block(cur_fn, "ul_exit");

        let max_cp = i64_t.const_int(0x10FFFF, false);
        let surrogate_lo = i64_t.const_int(0xD800, false);
        let surrogate_hi = i64_t.const_int(0xDFFF, false);
        let cp_neg = self
            .builder
            .build_int_compare(IntPredicate::SLT, cp, zero64, "ul_neg")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let cp_too_high = self
            .builder
            .build_int_compare(IntPredicate::SGT, cp, max_cp, "ul_too_high")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let ge_sur = self
            .builder
            .build_int_compare(IntPredicate::SGE, cp, surrogate_lo, "ul_ge_sur")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let le_sur = self
            .builder
            .build_int_compare(IntPredicate::SLE, cp, surrogate_hi, "ul_le_sur")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let is_sur = self
            .builder
            .build_and(ge_sur, le_sur, "ul_is_sur")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let invalid_low = self
            .builder
            .build_or(cp_neg, cp_too_high, "ul_inv_lo")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let invalid = self
            .builder
            .build_or(invalid_low, is_sur, "ul_invalid")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(invalid, reject_bb, valid_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(valid_bb);
        let thresh_2 = i64_t.const_int(0x80, false);
        let cp_lt_2 = self
            .builder
            .build_int_compare(IntPredicate::SLT, cp, thresh_2, "ul_lt_80")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(cp_lt_2, len_1_bb, try_2_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(try_2_bb);
        let thresh_3 = i64_t.const_int(0x800, false);
        let cp_lt_3 = self
            .builder
            .build_int_compare(IntPredicate::SLT, cp, thresh_3, "ul_lt_800")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(cp_lt_3, len_2_bb, try_3_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(try_3_bb);
        let thresh_4 = i64_t.const_int(0x10000, false);
        let cp_lt_4 = self
            .builder
            .build_int_compare(IntPredicate::SLT, cp, thresh_4, "ul_lt_10000")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(cp_lt_4, len_3_bb, len_4_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        for bb in [len_1_bb, len_2_bb, len_3_bb, len_4_bb, reject_bb] {
            self.builder.position_at_end(bb);
            self.builder
                .build_unconditional_branch(exit_bb)
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        }

        self.builder.position_at_end(exit_bb);
        let res_phi = self
            .builder
            .build_phi(i64_t, "ul_res")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        res_phi.add_incoming(&[
            (&one64 as &dyn BasicValue<'ctx>, len_1_bb),
            (&two64 as &dyn BasicValue<'ctx>, len_2_bb),
            (&three64 as &dyn BasicValue<'ctx>, len_3_bb),
            (&four64 as &dyn BasicValue<'ctx>, len_4_bb),
            (&zero64 as &dyn BasicValue<'ctx>, reject_bb),
        ]);
        Ok(res_phi.as_basic_value().into_int_value())
    }

    /// `@range-start table index` → load `table[2 * index]`.
    fn emit_range_start(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let table = self.compile_i64_vec_arg(args[0], env)?;
        let index = self.compile_expr(args[1], env, cur_fn)?;
        let two64 = self.context.i64_type().const_int(2, false);
        let field_index = self
            .builder
            .build_int_mul(index, two64, "range_start_index")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.load_i64(table, field_index, "range_start")
    }

    /// `@range-len table index` → load `table[2 * index + 1]`.
    fn emit_range_len(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let table = self.compile_i64_vec_arg(args[0], env)?;
        let index = self.compile_expr(args[1], env, cur_fn)?;
        let i64_t = self.context.i64_type();
        let two64 = i64_t.const_int(2, false);
        let one64 = i64_t.const_int(1, false);
        let base = self
            .builder
            .build_int_mul(index, two64, "range_len_base")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let field_index = self
            .builder
            .build_int_add(base, one64, "range_len_index")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.load_i64(table, field_index, "range_len")
    }

    /// `@line-index text len table` → write LF-delimited start/length pairs; return row count.
    fn emit_line_index(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let text = self.compile_buf_ptr_arg(args[0], env)?;
        let len = self.compile_expr(args[1], env, cur_fn)?;
        let table = self.compile_i64_vec_arg(args[2], env)?;

        let i64_t = self.context.i64_type();
        let zero64 = i64_t.const_zero();
        let one64 = i64_t.const_int(1, false);
        let lf64 = i64_t.const_int(10, false);

        let entry_bb = self.builder.get_insert_block().unwrap();
        let hdr_bb = self.context.append_basic_block(cur_fn, "li_hdr");
        let check_bb = self.context.append_basic_block(cur_fn, "li_chk");
        let emit_newline_bb = self.context.append_basic_block(cur_fn, "li_emit_nl");
        let cont_bb = self.context.append_basic_block(cur_fn, "li_cont");
        let final_bb = self.context.append_basic_block(cur_fn, "li_final");
        let emit_final_bb = self.context.append_basic_block(cur_fn, "li_emit_final");
        let ret_bb = self.context.append_basic_block(cur_fn, "li_ret");

        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(hdr_bb);
        let i_phi = self
            .builder
            .build_phi(i64_t, "li_i")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let start_phi = self
            .builder
            .build_phi(i64_t, "li_start")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let count_phi = self
            .builder
            .build_phi(i64_t, "li_count")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let i_val = i_phi.as_basic_value().into_int_value();
        let start_val = start_phi.as_basic_value().into_int_value();
        let count_val = count_phi.as_basic_value().into_int_value();
        let past_end = self
            .builder
            .build_int_compare(IntPredicate::SGE, i_val, len, "li_done")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(past_end, final_bb, check_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(check_bb);
        let byte = self.load_byte(text, i_val, "li_byte")?;
        let is_lf = self
            .builder
            .build_int_compare(IntPredicate::EQ, byte, lf64, "li_is_lf")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(is_lf, emit_newline_bb, cont_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(cont_bb);
        let i_next_cont = self
            .builder
            .build_int_add(i_val, one64, "li_i_next")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(emit_newline_bb);
        let line_len = self
            .builder
            .build_int_sub(i_val, start_val, "li_line_len")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.store_range_pair(table, count_val, start_val, line_len)?;
        let count_next = self
            .builder
            .build_int_add(count_val, one64, "li_count_next")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let start_next = self
            .builder
            .build_int_add(i_val, one64, "li_start_next")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let i_next_newline = start_next;
        self.builder
            .build_unconditional_branch(hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        i_phi.add_incoming(&[
            (&zero64 as &dyn BasicValue<'ctx>, entry_bb),
            (&i_next_cont as &dyn BasicValue<'ctx>, cont_bb),
            (&i_next_newline as &dyn BasicValue<'ctx>, emit_newline_bb),
        ]);
        start_phi.add_incoming(&[
            (&zero64 as &dyn BasicValue<'ctx>, entry_bb),
            (&start_val as &dyn BasicValue<'ctx>, cont_bb),
            (&start_next as &dyn BasicValue<'ctx>, emit_newline_bb),
        ]);
        count_phi.add_incoming(&[
            (&zero64 as &dyn BasicValue<'ctx>, entry_bb),
            (&count_val as &dyn BasicValue<'ctx>, cont_bb),
            (&count_next as &dyn BasicValue<'ctx>, emit_newline_bb),
        ]);

        self.builder.position_at_end(final_bb);
        let has_final = self
            .builder
            .build_int_compare(IntPredicate::SLT, start_val, len, "li_has_final")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(has_final, emit_final_bb, ret_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(emit_final_bb);
        let final_len = self
            .builder
            .build_int_sub(len, start_val, "li_final_len")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.store_range_pair(table, count_val, start_val, final_len)?;
        let final_count = self
            .builder
            .build_int_add(count_val, one64, "li_final_count")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(ret_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(ret_bb);
        let ret_phi = self
            .builder
            .build_phi(i64_t, "li_ret")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        ret_phi.add_incoming(&[
            (&count_val as &dyn BasicValue<'ctx>, final_bb),
            (&final_count as &dyn BasicValue<'ctx>, emit_final_bb),
        ]);
        Ok(ret_phi.as_basic_value().into_int_value())
    }

    /// `@token-index text off len delim table` → write non-empty token ranges; return count.
    fn emit_token_index(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let text = self.compile_buf_ptr_arg(args[0], env)?;
        let off = self.compile_expr(args[1], env, cur_fn)?;
        let len = self.compile_expr(args[2], env, cur_fn)?;
        let delim = self.compile_expr(args[3], env, cur_fn)?;
        let table = self.compile_i64_vec_arg(args[4], env)?;

        let i64_t = self.context.i64_type();
        let i8_t = self.context.i8_type();
        let zero64 = i64_t.const_zero();
        let one64 = i64_t.const_int(1, false);
        let delim8 = self
            .builder
            .build_int_truncate(delim, i8_t, "ti_delim8")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let end = self
            .builder
            .build_int_add(off, len, "ti_end")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        let entry_bb = self.builder.get_insert_block().unwrap();
        let skip_hdr_bb = self.context.append_basic_block(cur_fn, "ti_skip_hdr");
        let skip_check_bb = self.context.append_basic_block(cur_fn, "ti_skip_chk");
        let skip_delim_bb = self.context.append_basic_block(cur_fn, "ti_skip_delim");
        let scan_hdr_bb = self.context.append_basic_block(cur_fn, "ti_scan_hdr");
        let scan_check_bb = self.context.append_basic_block(cur_fn, "ti_scan_chk");
        let scan_cont_bb = self.context.append_basic_block(cur_fn, "ti_scan_cont");
        let emit_delim_bb = self.context.append_basic_block(cur_fn, "ti_emit_delim");
        let emit_eof_bb = self.context.append_basic_block(cur_fn, "ti_emit_eof");
        let ret_bb = self.context.append_basic_block(cur_fn, "ti_ret");

        self.builder
            .build_unconditional_branch(skip_hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(skip_hdr_bb);
        let i_phi = self
            .builder
            .build_phi(i64_t, "ti_i")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let count_phi = self
            .builder
            .build_phi(i64_t, "ti_count")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let i_val = i_phi.as_basic_value().into_int_value();
        let count_val = count_phi.as_basic_value().into_int_value();
        let past_end = self
            .builder
            .build_int_compare(IntPredicate::SGE, i_val, end, "ti_done")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(past_end, ret_bb, skip_check_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(skip_check_bb);
        let skip_ptr = self.ptr_at(text, i_val, "ti_skip_ptr")?;
        let skip_byte = self
            .builder
            .build_load(i8_t, skip_ptr, "ti_skip_byte")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_int_value();
        let skip_is_delim = self
            .builder
            .build_int_compare(IntPredicate::EQ, skip_byte, delim8, "ti_skip_is_delim")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(skip_is_delim, skip_delim_bb, scan_hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(skip_delim_bb);
        let i_after_delim = self
            .builder
            .build_int_add(i_val, one64, "ti_i_after_delim")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(skip_hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(scan_hdr_bb);
        let j_phi = self
            .builder
            .build_phi(i64_t, "ti_j")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let j_val = j_phi.as_basic_value().into_int_value();
        let token_reaches_end = self
            .builder
            .build_int_compare(IntPredicate::SGE, j_val, end, "ti_token_done")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(token_reaches_end, emit_eof_bb, scan_check_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(scan_check_bb);
        let scan_ptr = self.ptr_at(text, j_val, "ti_scan_ptr")?;
        let scan_byte = self
            .builder
            .build_load(i8_t, scan_ptr, "ti_scan_byte")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_int_value();
        let scan_is_delim = self
            .builder
            .build_int_compare(IntPredicate::EQ, scan_byte, delim8, "ti_scan_is_delim")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(scan_is_delim, emit_delim_bb, scan_cont_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(scan_cont_bb);
        let j_next = self
            .builder
            .build_int_add(j_val, one64, "ti_j_next")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(scan_hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        j_phi.add_incoming(&[
            (&i_val as &dyn BasicValue<'ctx>, skip_check_bb),
            (&j_next as &dyn BasicValue<'ctx>, scan_cont_bb),
        ]);

        self.builder.position_at_end(emit_delim_bb);
        let token_len = self
            .builder
            .build_int_sub(j_val, i_val, "ti_token_len")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.store_range_pair(table, count_val, i_val, token_len)?;
        let count_next = self
            .builder
            .build_int_add(count_val, one64, "ti_count_next")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let i_after_token = self
            .builder
            .build_int_add(j_val, one64, "ti_i_after_token")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(skip_hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        i_phi.add_incoming(&[
            (&off as &dyn BasicValue<'ctx>, entry_bb),
            (&i_after_delim as &dyn BasicValue<'ctx>, skip_delim_bb),
            (&i_after_token as &dyn BasicValue<'ctx>, emit_delim_bb),
        ]);
        count_phi.add_incoming(&[
            (&zero64 as &dyn BasicValue<'ctx>, entry_bb),
            (&count_val as &dyn BasicValue<'ctx>, skip_delim_bb),
            (&count_next as &dyn BasicValue<'ctx>, emit_delim_bb),
        ]);

        self.builder.position_at_end(emit_eof_bb);
        let final_len = self
            .builder
            .build_int_sub(end, i_val, "ti_final_len")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.store_range_pair(table, count_val, i_val, final_len)?;
        let final_count = self
            .builder
            .build_int_add(count_val, one64, "ti_final_count")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(ret_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(ret_bb);
        let ret_phi = self
            .builder
            .build_phi(i64_t, "ti_ret")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        ret_phi.add_incoming(&[
            (&count_val as &dyn BasicValue<'ctx>, skip_hdr_bb),
            (&final_count as &dyn BasicValue<'ctx>, emit_eof_bb),
        ]);
        Ok(ret_phi.as_basic_value().into_int_value())
    }

    /// `@token-index-any text off len delims delim-count table` → write non-empty token ranges; return count.
    fn emit_token_index_any(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let text = self.compile_buf_ptr_arg(args[0], env)?;
        let off = self.compile_expr(args[1], env, cur_fn)?;
        let len = self.compile_expr(args[2], env, cur_fn)?;
        let delims =
            self.compile_buffer_arg(args[3], "token_index_any_delims", true, env, cur_fn)?;
        let delim_count = self.compile_expr(args[4], env, cur_fn)?;
        let table = self.compile_i64_vec_arg(args[5], env)?;

        let i64_t = self.context.i64_type();
        let i8_t = self.context.i8_type();
        let zero64 = i64_t.const_zero();
        let one64 = i64_t.const_int(1, false);
        let end = self
            .builder
            .build_int_add(off, len, "tia_end")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        let entry_bb = self.builder.get_insert_block().unwrap();
        let skip_hdr_bb = self.context.append_basic_block(cur_fn, "tia_skip_hdr");
        let skip_check_bb = self.context.append_basic_block(cur_fn, "tia_skip_chk");
        let skip_delim_bb = self.context.append_basic_block(cur_fn, "tia_skip_delim");
        let scan_hdr_bb = self.context.append_basic_block(cur_fn, "tia_scan_hdr");
        let scan_check_bb = self.context.append_basic_block(cur_fn, "tia_scan_chk");
        let scan_cont_bb = self.context.append_basic_block(cur_fn, "tia_scan_cont");
        let emit_delim_bb = self.context.append_basic_block(cur_fn, "tia_emit_delim");
        let emit_eof_bb = self.context.append_basic_block(cur_fn, "tia_emit_eof");
        let ret_bb = self.context.append_basic_block(cur_fn, "tia_ret");

        self.builder
            .build_unconditional_branch(skip_hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(skip_hdr_bb);
        let i_phi = self
            .builder
            .build_phi(i64_t, "tia_i")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let count_phi = self
            .builder
            .build_phi(i64_t, "tia_count")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let i_val = i_phi.as_basic_value().into_int_value();
        let count_val = count_phi.as_basic_value().into_int_value();
        let past_end = self
            .builder
            .build_int_compare(IntPredicate::SGE, i_val, end, "tia_done")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(past_end, ret_bb, skip_check_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(skip_check_bb);
        let skip_ptr = self.ptr_at(text, i_val, "tia_skip_ptr")?;
        let skip_byte = self
            .builder
            .build_load(i8_t, skip_ptr, "tia_skip_byte")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_int_value();
        let skip_is_delim =
            self.emit_byte_in_delims(skip_byte, delims, delim_count, cur_fn, "tia_skip")?;
        let skip_non_delim_bb = self.builder.get_insert_block().unwrap();
        self.builder
            .build_conditional_branch(skip_is_delim, skip_delim_bb, scan_hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(skip_delim_bb);
        let i_after_delim = self
            .builder
            .build_int_add(i_val, one64, "tia_i_after_delim")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(skip_hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(scan_hdr_bb);
        let j_phi = self
            .builder
            .build_phi(i64_t, "tia_j")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let j_val = j_phi.as_basic_value().into_int_value();
        let token_reaches_end = self
            .builder
            .build_int_compare(IntPredicate::SGE, j_val, end, "tia_token_done")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(token_reaches_end, emit_eof_bb, scan_check_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(scan_check_bb);
        let scan_ptr = self.ptr_at(text, j_val, "tia_scan_ptr")?;
        let scan_byte = self
            .builder
            .build_load(i8_t, scan_ptr, "tia_scan_byte")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_int_value();
        let scan_is_delim =
            self.emit_byte_in_delims(scan_byte, delims, delim_count, cur_fn, "tia_scan")?;
        self.builder
            .build_conditional_branch(scan_is_delim, emit_delim_bb, scan_cont_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(scan_cont_bb);
        let j_next = self
            .builder
            .build_int_add(j_val, one64, "tia_j_next")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(scan_hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        j_phi.add_incoming(&[
            (&i_val as &dyn BasicValue<'ctx>, skip_non_delim_bb),
            (&j_next as &dyn BasicValue<'ctx>, scan_cont_bb),
        ]);

        self.builder.position_at_end(emit_delim_bb);
        let token_len = self
            .builder
            .build_int_sub(j_val, i_val, "tia_token_len")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.store_range_pair(table, count_val, i_val, token_len)?;
        let count_next = self
            .builder
            .build_int_add(count_val, one64, "tia_count_next")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let i_after_token = self
            .builder
            .build_int_add(j_val, one64, "tia_i_after_token")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(skip_hdr_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        i_phi.add_incoming(&[
            (&off as &dyn BasicValue<'ctx>, entry_bb),
            (&i_after_delim as &dyn BasicValue<'ctx>, skip_delim_bb),
            (&i_after_token as &dyn BasicValue<'ctx>, emit_delim_bb),
        ]);
        count_phi.add_incoming(&[
            (&zero64 as &dyn BasicValue<'ctx>, entry_bb),
            (&count_val as &dyn BasicValue<'ctx>, skip_delim_bb),
            (&count_next as &dyn BasicValue<'ctx>, emit_delim_bb),
        ]);

        self.builder.position_at_end(emit_eof_bb);
        let final_len = self
            .builder
            .build_int_sub(end, i_val, "tia_final_len")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.store_range_pair(table, count_val, i_val, final_len)?;
        let final_count = self
            .builder
            .build_int_add(count_val, one64, "tia_final_count")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(ret_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(ret_bb);
        let ret_phi = self
            .builder
            .build_phi(i64_t, "tia_ret")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        ret_phi.add_incoming(&[
            (&count_val as &dyn BasicValue<'ctx>, skip_hdr_bb),
            (&final_count as &dyn BasicValue<'ctx>, emit_eof_bb),
        ]);
        Ok(ret_phi.as_basic_value().into_int_value())
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
        let a = self.compile_buf_ptr_arg(args[0], env)?;
        let a_off = self.compile_expr(args[1], env, cur_fn)?;
        let b = self.compile_buf_ptr_arg(args[2], env)?;
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
        let buf = self.compile_buf_ptr_arg(args[0], env)?;
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
        let buf = self.compile_buf_ptr_arg(args[0], env)?;
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
        let buf = self.compile_buf_ptr_arg(args[0], env)?;
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
        param_tys: Vec<ParamTy>,
        ret_ty: ValueTy,
        name_hint: &str,
        closure_template: Option<(&Node, ValueTy)>,
    ) -> Result<FunctionBinding<'ctx>> {
        let arity = param_tys.len();
        check_closed(lam_body, arity as u64)?;

        let fn_name = self.fresh_fn_name(name_hint);
        let fn_val = self.add_tacit_function(&fn_name, &param_tys, &ret_ty, &[])?;
        // C calling convention is LLVM's default (ADR 0027); no override needed.

        self.compile_lambda_body(fn_val, lam_body, &param_tys, &ret_ty)?;
        Ok(FunctionBinding {
            value: fn_val,
            param_tys,
            ret_ty,
            captures: Vec::new(),
            closure_template: closure_template.map(|(expr, ty)| ClosureTemplate {
                expr: Box::new(expr.clone()),
                ty,
                env: Vec::new(),
            }),
        })
    }

    fn reify_function_binding(
        &mut self,
        fn_binding: &FunctionBinding<'ctx>,
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<CompiledValue<'ctx>> {
        // A function with a call-local handle parameter is direct-call only;
        // it has no first-class function value (ADR 0098).
        if fn_binding.has_handle_param() {
            return Err(CodegenError::Unsupported(
                "function with a handle-typed parameter used as a first-class value",
            ));
        }
        if let Some(template) = &fn_binding.closure_template {
            return self.compile_closure_value(
                template.expr.as_ref(),
                Some(template.ty.clone()),
                &template.env,
                cur_fn,
            );
        }
        self.reify_direct_function_binding(fn_binding, cur_fn)
    }

    fn reify_direct_function_binding(
        &mut self,
        fn_binding: &FunctionBinding<'ctx>,
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<CompiledValue<'ctx>> {
        if fn_binding.param_tys.is_empty() {
            return Err(CodegenError::UnsupportedClosureEscape(
                "zero-argument direct function reification is not implemented",
            ));
        }

        let mut hidden_captures = Vec::with_capacity(fn_binding.captures.len());
        for (i, binding) in fn_binding.captures.iter().enumerate() {
            let value = self.capture_binding_as_value(binding, i as u64, cur_fn)?;
            hidden_captures.push(value);
        }
        self.build_direct_adapter_closure(fn_binding, Vec::new(), hidden_captures)
    }

    fn build_direct_adapter_closure(
        &mut self,
        direct: &FunctionBinding<'ctx>,
        supplied_args: Vec<CompiledValue<'ctx>>,
        hidden_captures: Vec<CompiledValue<'ctx>>,
    ) -> Result<CompiledValue<'ctx>> {
        let supplied_count = supplied_args.len();
        if supplied_count >= direct.param_tys.len() {
            return Err(CodegenError::UnsupportedClosureEscape(
                "direct function adapter has no remaining argument",
            ));
        }

        // Reachable only for all-value functions: `reify_function_binding`
        // rejects handle-parameter functions before any adapter is built.
        let fn_ty = try_nested_fn_ty(&direct.param_tys[supplied_count..], direct.ret_ty.clone())
            .ok_or(CodegenError::Unsupported(
                "function with a handle-typed parameter used as a first-class value",
            ))?;
        let mut capture_values = supplied_args;
        capture_values.extend(hidden_captures);
        let captures: Vec<CaptureValue<'ctx>> = capture_values
            .into_iter()
            .enumerate()
            .map(|(outer_index, value)| CaptureValue { outer_index, value })
            .collect();
        let capture_tys: Vec<ValueTy> = captures.iter().map(|c| c.value.ty.clone()).collect();

        let fn_name = self.fresh_fn_name("direct_closure");
        let entry = self.module.add_function(
            &fn_name,
            self.closure_entry_type(&fn_ty)?,
            Some(Linkage::Private),
        );
        self.compile_direct_adapter_entry(entry, direct, supplied_count, &captures)?;
        let env_ptr = self.allocate_closure_env(&captures, &capture_tys)?;
        let closure_value =
            self.build_closure_pair(entry.as_global_value().as_pointer_value(), env_ptr)?;
        Ok(CompiledValue {
            ty: fn_ty,
            value: closure_value,
        })
    }

    fn compile_closure_value(
        &mut self,
        node: &Node,
        expected_ty: Option<ValueTy>,
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<CompiledValue<'ctx>> {
        match node {
            Node::Ann { expr, type_ } => {
                let declared = value_ty_from_ty(&ty_from_type_node(type_)?)?;
                self.compile_closure_value(expr, Some(declared), env, cur_fn)
            }
            Node::Lam { body } => {
                let fn_ty = if let Some(ty) = expected_ty {
                    ty
                } else {
                    self.infer_value_ty(node, &binding_tys_from_env(env))?
                };
                self.compile_lambda_closure(body, fn_ty, env, cur_fn)
            }
            _ => Err(CodegenError::AppNonFunction),
        }
    }

    fn compile_lambda_closure(
        &mut self,
        body: &Node,
        fn_ty: ValueTy,
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<CompiledValue<'ctx>> {
        let ValueTy::Fn(arg_ty, ret_ty) = &fn_ty else {
            return Err(CodegenError::AppNonFunction);
        };

        let captures = self.collect_capture_values(body, env, cur_fn)?;
        let capture_tys: Vec<ValueTy> = captures.iter().map(|c| c.value.ty.clone()).collect();

        let fn_name = self.fresh_fn_name("closure");
        let fn_val = self.module.add_function(
            &fn_name,
            self.closure_entry_type(&fn_ty)?,
            Some(Linkage::Private),
        );

        self.compile_closure_entry(fn_val, body, arg_ty, ret_ty, &captures, env)?;

        let env_ptr = self.allocate_closure_env(&captures, &capture_tys)?;
        let code_ptr = fn_val.as_global_value().as_pointer_value();
        let closure_value = self.build_closure_pair(code_ptr, env_ptr)?;
        Ok(CompiledValue {
            ty: fn_ty,
            value: closure_value,
        })
    }

    fn collect_capture_values(
        &mut self,
        body: &Node,
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<Vec<CaptureValue<'ctx>>> {
        let mut free = BTreeSet::new();
        collect_free_outer_indices(body, 1, &mut free);
        let mut captures = Vec::with_capacity(free.len());
        for outer_index in free {
            let binding = env.get(outer_index).ok_or(CodegenError::FreeVarInLambda {
                index: (outer_index + 1) as u64,
            })?;
            let value = self.capture_binding_as_value(binding, (outer_index + 1) as u64, cur_fn)?;
            captures.push(CaptureValue { outer_index, value });
        }
        Ok(captures)
    }

    fn capture_binding_as_value(
        &mut self,
        binding: &Binding<'ctx>,
        index: u64,
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<CompiledValue<'ctx>> {
        match binding {
            Binding::Value(v) => Ok(v.clone()),
            Binding::Function(f) => self.reify_function_binding(f, cur_fn),
            Binding::Ptr {
                kind: PtrKind::Buf, ..
            } => Err(CodegenError::InvalidCapture { index, kind: "Buf" }),
            Binding::Ptr {
                kind: PtrKind::I64Vec,
                ..
            } => Err(CodegenError::InvalidCapture {
                index,
                kind: "I64Vec",
            }),
            Binding::VecHandle { .. } => Err(CodegenError::InvalidCapture {
                index,
                kind: "typed vec",
            }),
            Binding::Unavailable => Err(CodegenError::UnavailableCapture { index }),
        }
    }

    fn compile_direct_adapter_entry(
        &mut self,
        entry_fn: FunctionValue<'ctx>,
        direct: &FunctionBinding<'ctx>,
        supplied_count: usize,
        captures: &[CaptureValue<'ctx>],
    ) -> Result<()> {
        let saved_block = self.builder.get_insert_block();
        let entry = self.context.append_basic_block(entry_fn, "entry");
        self.builder.position_at_end(entry);

        let env_ptr = entry_fn
            .get_nth_param(0)
            .ok_or_else(|| CodegenError::Llvm("direct closure missing env param".into()))?
            .into_pointer_value();
        let arg = entry_fn
            .get_nth_param(1)
            .ok_or_else(|| CodegenError::Llvm("direct closure missing arg param".into()))?;
        let loaded_captures = self.load_linear_captures(env_ptr, captures)?;

        let hidden_count = direct.captures.len();
        let loaded_supplied = loaded_captures[..supplied_count].to_vec();
        let loaded_hidden = loaded_captures[supplied_count..supplied_count + hidden_count].to_vec();
        // All-value only: handle-parameter functions never reach adapter
        // construction (`reify_function_binding` rejects them first).
        let current_arg = CompiledValue {
            ty: direct.param_tys[supplied_count].as_value().cloned().ok_or(
                CodegenError::Unsupported(
                    "function with a handle-typed parameter used as a first-class value",
                ),
            )?,
            value: arg,
        };

        let ret = if supplied_count + 1 == direct.param_tys.len() {
            let mut call_args: Vec<BasicMetadataValueEnum<'ctx>> =
                Vec::with_capacity(direct.param_tys.len() + hidden_count);
            for supplied in &loaded_supplied {
                call_args.push(supplied.value.into());
            }
            call_args.push(current_arg.value.into());
            for capture in &loaded_hidden {
                call_args.push(capture.value.into());
            }

            self.builder
                .build_call(direct.value, &call_args, "direct_closure_call")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?
                .try_as_basic_value()
                .basic()
                .ok_or_else(|| CodegenError::Llvm("direct closure call returned no value".into()))?
        } else {
            let mut next_supplied = loaded_supplied;
            next_supplied.push(current_arg);
            self.build_direct_adapter_closure(direct, next_supplied, loaded_hidden)?
                .value
        };
        self.builder
            .build_return(Some(&ret))
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        if let Some(saved) = saved_block {
            self.builder.position_at_end(saved);
        }
        Ok(())
    }

    fn allocate_closure_env(
        &mut self,
        captures: &[CaptureValue<'ctx>],
        capture_tys: &[ValueTy],
    ) -> Result<PointerValue<'ctx>> {
        let ptr_t = self.context.ptr_type(AddressSpace::default());
        if captures.is_empty() {
            return Ok(ptr_t.const_null());
        }
        let field_types = capture_tys
            .iter()
            .map(|ty| self.llvm_type(ty))
            .collect::<Result<Vec<_>>>()?;
        let env_ty = self.context.struct_type(&field_types, false);
        let env_ptr = self
            .builder
            .build_malloc(env_ty, "closure_env")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        for (i, capture) in captures.iter().enumerate() {
            let slot = self
                .builder
                .build_struct_gep(env_ty, env_ptr, i as u32, "closure_env_slot")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
            self.builder
                .build_store(slot, capture.value.value)
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        }
        Ok(env_ptr)
    }

    fn compile_closure_entry(
        &mut self,
        fn_val: FunctionValue<'ctx>,
        body: &Node,
        arg_ty: &ValueTy,
        ret_ty: &ValueTy,
        captures: &[CaptureValue<'ctx>],
        outer_env: &[Binding<'ctx>],
    ) -> Result<()> {
        let saved_block = self.builder.get_insert_block();
        let entry = self.context.append_basic_block(fn_val, "entry");
        self.builder.position_at_end(entry);

        let env_ptr = fn_val
            .get_nth_param(0)
            .ok_or_else(|| CodegenError::Llvm("closure missing env param".into()))?
            .into_pointer_value();
        let arg = fn_val
            .get_nth_param(1)
            .ok_or_else(|| CodegenError::Llvm("closure missing arg param".into()))?;

        let loaded_captures = self.load_closure_captures(env_ptr, captures, outer_env)?;
        let mut body_env = Vec::with_capacity(1 + loaded_captures.len());
        body_env.push(Binding::Value(CompiledValue {
            ty: arg_ty.clone(),
            value: arg,
        }));
        body_env.extend(loaded_captures);

        let v = if matches!(body, Node::Lam { .. } | Node::Ann { .. })
            && matches!(ret_ty, ValueTy::Fn(_, _))
        {
            self.compile_closure_value(body, Some(ret_ty.clone()), &body_env, fn_val)?
        } else {
            self.compile_value_expr(body, &body_env, fn_val)?
        };
        if &v.ty != ret_ty {
            return Err(CodegenError::ValueTypeMismatch {
                expected: ret_ty.to_string(),
                actual: v.ty.to_string(),
            });
        }
        self.builder
            .build_return(Some(&v.value))
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        if let Some(saved) = saved_block {
            self.builder.position_at_end(saved);
        }
        Ok(())
    }

    fn load_closure_captures(
        &mut self,
        env_ptr: PointerValue<'ctx>,
        captures: &[CaptureValue<'ctx>],
        outer_env: &[Binding<'ctx>],
    ) -> Result<Vec<Binding<'ctx>>> {
        let mut loaded = Vec::with_capacity(outer_env.len());
        for binding in outer_env {
            loaded.push(self.placeholder_binding(binding)?);
        }
        if captures.is_empty() {
            return Ok(loaded);
        }

        let field_types = captures
            .iter()
            .map(|capture| self.llvm_type(&capture.value.ty))
            .collect::<Result<Vec<_>>>()?;
        let env_ty = self.context.struct_type(&field_types, false);
        for (slot_index, capture) in captures.iter().enumerate() {
            let slot = self
                .builder
                .build_struct_gep(env_ty, env_ptr, slot_index as u32, "closure_env_slot")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
            let loaded_value = self
                .builder
                .build_load(self.llvm_type(&capture.value.ty)?, slot, "closure_capture")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
            loaded[capture.outer_index] = Binding::Value(CompiledValue {
                ty: capture.value.ty.clone(),
                value: loaded_value,
            });
        }
        Ok(loaded)
    }

    fn load_linear_captures(
        &mut self,
        env_ptr: PointerValue<'ctx>,
        captures: &[CaptureValue<'ctx>],
    ) -> Result<Vec<CompiledValue<'ctx>>> {
        if captures.is_empty() {
            return Ok(Vec::new());
        }
        let field_types = captures
            .iter()
            .map(|capture| self.llvm_type(&capture.value.ty))
            .collect::<Result<Vec<_>>>()?;
        let env_ty = self.context.struct_type(&field_types, false);
        let mut loaded = Vec::with_capacity(captures.len());
        for (slot_index, capture) in captures.iter().enumerate() {
            let slot = self
                .builder
                .build_struct_gep(env_ty, env_ptr, slot_index as u32, "closure_env_slot")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
            let loaded_value = self
                .builder
                .build_load(self.llvm_type(&capture.value.ty)?, slot, "closure_capture")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
            loaded.push(CompiledValue {
                ty: capture.value.ty.clone(),
                value: loaded_value,
            });
        }
        Ok(loaded)
    }

    fn placeholder_binding(&self, binding: &Binding<'ctx>) -> Result<Binding<'ctx>> {
        match binding {
            Binding::Value(_) => Ok(Binding::Unavailable),
            Binding::Function(f) => Ok(Binding::Function(f.clone())),
            Binding::Ptr { .. } => Ok(Binding::Unavailable),
            Binding::VecHandle { .. } => Ok(Binding::Unavailable),
            Binding::Unavailable => Ok(Binding::Unavailable),
        }
    }

    fn add_tacit_function(
        &self,
        name: &str,
        param_tys: &[ParamTy],
        ret_ty: &ValueTy,
        captures: &[Binding<'ctx>],
    ) -> Result<FunctionValue<'ctx>> {
        let ptr_t = self.context.ptr_type(AddressSpace::default());
        let mut params: Vec<BasicMetadataTypeEnum<'ctx>> = Vec::with_capacity(param_tys.len());
        // A handle parameter lowers to the same word(s) as a `rec` hidden
        // handle capture — one internal handle calling convention (ADR 0098).
        for pt in param_tys {
            match pt {
                ParamTy::Value(ty) => params.push(self.llvm_type(ty)?.into()),
                ParamTy::Handle(HandleTy::Buf) | ParamTy::Handle(HandleTy::I64Vec) => {
                    params.push(BasicMetadataTypeEnum::PointerType(ptr_t));
                }
                ParamTy::Handle(HandleTy::Vec(_)) => {
                    params.push(BasicMetadataTypeEnum::PointerType(ptr_t));
                    params.push(BasicMetadataTypeEnum::IntType(self.context.i64_type()));
                }
            }
        }
        for capture in captures {
            match capture {
                Binding::Value(v) => params.push(self.llvm_type(&v.ty)?.into()),
                Binding::Ptr { .. } => params.push(BasicMetadataTypeEnum::PointerType(ptr_t)),
                Binding::VecHandle { .. } => {
                    // VecHandle: pointer + i64 length, passed as two hidden params.
                    params.push(BasicMetadataTypeEnum::PointerType(ptr_t));
                    params.push(BasicMetadataTypeEnum::IntType(self.context.i64_type()));
                }
                Binding::Function(_) | Binding::Unavailable => {}
            }
        }
        let fn_ty = self.llvm_type(ret_ty)?.fn_type(&params, false);
        Ok(self
            .module
            .add_function(name, fn_ty, Some(Linkage::Private)))
    }

    /// Bind a direct-call function's declared parameters from its LLVM
    /// parameter list (ADR 0098). Returns the per-source-parameter bindings in
    /// source order, together with the number of LLVM parameter slots
    /// consumed — a handle parameter may occupy two slots, so hidden capture
    /// parameters start after this count rather than after the arity.
    fn bind_param_slots(
        &self,
        fn_val: FunctionValue<'ctx>,
        param_tys: &[ParamTy],
    ) -> Result<(Vec<Binding<'ctx>>, u32)> {
        let nth = |idx: u32| {
            fn_val
                .get_nth_param(idx)
                .ok_or_else(|| CodegenError::Llvm(format!("lambda missing param slot {idx}")))
        };
        let mut bindings = Vec::with_capacity(param_tys.len());
        let mut slot = 0u32;
        for pt in param_tys {
            match pt {
                ParamTy::Value(ty) => {
                    let value = nth(slot)?;
                    slot += 1;
                    bindings.push(Binding::Value(CompiledValue {
                        ty: ty.clone(),
                        value,
                    }));
                }
                ParamTy::Handle(HandleTy::Buf) => {
                    let ptr = nth(slot)?.into_pointer_value();
                    slot += 1;
                    bindings.push(Binding::Ptr {
                        ptr,
                        kind: PtrKind::Buf,
                    });
                }
                ParamTy::Handle(HandleTy::I64Vec) => {
                    let ptr = nth(slot)?.into_pointer_value();
                    slot += 1;
                    bindings.push(Binding::Ptr {
                        ptr,
                        kind: PtrKind::I64Vec,
                    });
                }
                ParamTy::Handle(HandleTy::Vec(elem)) => {
                    let ptr = nth(slot)?.into_pointer_value();
                    let len = nth(slot + 1)?.into_int_value();
                    slot += 2;
                    bindings.push(Binding::VecHandle {
                        ptr,
                        len,
                        ty: *elem,
                    });
                }
            }
        }
        Ok((bindings, slot))
    }

    fn compile_lambda_body(
        &mut self,
        fn_val: FunctionValue<'ctx>,
        body: &Node,
        param_tys: &[ParamTy],
        ret_ty: &ValueTy,
    ) -> Result<()> {
        let saved_block = self.builder.get_insert_block();
        let entry = self.context.append_basic_block(fn_val, "entry");
        self.builder.position_at_end(entry);

        // Innermost binder is DeBruijn 0, so the env stacks parameters in
        // reverse source order.
        let (param_bindings, _slots) = self.bind_param_slots(fn_val, param_tys)?;
        let env: Vec<Binding<'ctx>> = param_bindings.into_iter().rev().collect();

        let v = if matches!(body, Node::Lam { .. } | Node::Ann { .. })
            && matches!(ret_ty, ValueTy::Fn(_, _))
        {
            self.compile_closure_value(body, Some(ret_ty.clone()), &env, fn_val)?
        } else {
            self.compile_value_expr(body, &env, fn_val)?
        };
        if &v.ty != ret_ty {
            return Err(CodegenError::ValueTypeMismatch {
                expected: ret_ty.to_string(),
                actual: v.ty.to_string(),
            });
        }
        self.builder
            .build_return(Some(&v.value))
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
    ) -> Result<CompiledValue<'ctx>> {
        // Every Rec member must be a Lam chain.
        let mut specs: Vec<LamSpec<'_>> = Vec::with_capacity(bindings.len());
        for (i, b) in bindings.iter().enumerate() {
            if let Some((arity, lam_body, ann_ty)) = collect_annotated_lam_chain(b) {
                let (param_tys, ret_ty) = if ann_ty.is_some() {
                    self.signature_for_lam(
                        lam_body,
                        arity,
                        ann_ty,
                        &[],
                        &binding_tys_from_env(env),
                    )?
                } else {
                    // An un-annotated rec member takes only first-class
                    // integer parameters; a handle parameter on a rec member
                    // requires an explicit signature (ADR 0098).
                    let param_tys = vec![ParamTy::Value(ValueTy::Int); arity];
                    let lambda_env: Vec<BindingTy> = vec![BindingTy::Value(ValueTy::Int); arity];
                    let ret_ty = self
                        .infer_value_ty(lam_body, &lambda_env)
                        .unwrap_or(ValueTy::Int);
                    (param_tys, ret_ty)
                };
                specs.push(LamSpec {
                    param_tys,
                    ret_ty,
                    body: lam_body,
                });
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
        for spec in &specs {
            let name = self.fresh_fn_name("rec");
            let f = self.add_tacit_function(&name, &spec.param_tys, &spec.ret_ty, env)?;
            fns.push(FunctionBinding {
                value: f,
                param_tys: spec.param_tys.clone(),
                ret_ty: spec.ret_ty.clone(),
                captures: env.to_vec(),
                closure_template: None,
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
        for (i, spec) in specs.iter().enumerate() {
            // The lambda body sees its params in reverse DeBruijn order,
            // then the rec frame, then hidden capture parameters standing in
            // for the outer env. Build the per-body env accordingly.
            self.compile_lambda_body_with_rec_env(
                &fns[i],
                spec.body,
                &spec.param_tys,
                &spec.ret_ty,
                &fns,
                env,
            )
            .map_err(|cause| CodegenError::RecGroupFailed {
                failing_index: i,
                cause: Box::new(cause),
            })?;
        }

        // Compile the rec-block body in the current scope, with the rec frame
        // on top of the existing env.
        self.compile_value_expr(body, &rec_env, cur_fn)
    }

    fn compile_lambda_body_with_rec_env(
        &mut self,
        fn_binding: &FunctionBinding<'ctx>,
        body: &Node,
        param_tys: &[ParamTy],
        ret_ty: &ValueTy,
        rec_fns: &[FunctionBinding<'ctx>],
        outer_env: &[Binding<'ctx>],
    ) -> Result<()> {
        let fn_val = fn_binding.value;
        let saved_block = self.builder.get_insert_block();
        let entry = self.context.append_basic_block(fn_val, "entry");
        self.builder.position_at_end(entry);
        // Hidden capture parameters start after every declared-parameter LLVM
        // slot — a handle parameter consumes two (ADR 0098).
        let (param_bindings, param_slots) = self.bind_param_slots(fn_val, param_tys)?;
        let captured_env = self.capture_env_from_params(fn_val, param_slots, outer_env)?;
        let mut rec_env: Vec<Binding<'ctx>> =
            Vec::with_capacity(rec_fns.len() + captured_env.len());
        for f in rec_fns {
            rec_env.push(Binding::Function(FunctionBinding {
                value: f.value,
                param_tys: f.param_tys.clone(),
                ret_ty: f.ret_ty.clone(),
                captures: captured_env.clone(),
                closure_template: None,
            }));
        }
        rec_env.extend_from_slice(&captured_env);

        let mut body_env: Vec<Binding<'ctx>> =
            Vec::with_capacity(param_bindings.len() + rec_env.len());
        body_env.extend(param_bindings.into_iter().rev());
        body_env.extend_from_slice(&rec_env);
        let v = if matches!(body, Node::Lam { .. } | Node::Ann { .. })
            && matches!(ret_ty, ValueTy::Fn(_, _))
        {
            self.compile_closure_value(body, Some(ret_ty.clone()), &body_env, fn_val)?
        } else {
            self.compile_value_expr(body, &body_env, fn_val)?
        };
        if &v.ty != ret_ty {
            return Err(CodegenError::ValueTypeMismatch {
                expected: ret_ty.to_string(),
                actual: v.ty.to_string(),
            });
        }
        self.builder
            .build_return(Some(&v.value))
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        if let Some(saved) = saved_block {
            self.builder.position_at_end(saved);
        }
        Ok(())
    }

    fn capture_env_from_params(
        &self,
        fn_val: FunctionValue<'ctx>,
        param_slots: u32,
        outer_env: &[Binding<'ctx>],
    ) -> Result<Vec<Binding<'ctx>>> {
        let mut param_index = param_slots;
        let mut captured = Vec::with_capacity(outer_env.len());
        for binding in outer_env {
            match binding {
                Binding::Value(v) => {
                    let param = fn_val.get_nth_param(param_index).ok_or_else(|| {
                        CodegenError::Llvm(format!(
                            "lambda missing captured int param {}",
                            param_index
                        ))
                    })?;
                    captured.push(Binding::Value(CompiledValue {
                        ty: v.ty.clone(),
                        value: param,
                    }));
                    param_index += 1;
                }
                Binding::Ptr { kind, .. } => {
                    let param = fn_val
                        .get_nth_param(param_index)
                        .ok_or_else(|| {
                            CodegenError::Llvm(format!(
                                "lambda missing captured ptr param {}",
                                param_index
                            ))
                        })?
                        .into_pointer_value();
                    captured.push(Binding::Ptr {
                        ptr: param,
                        kind: *kind,
                    });
                    param_index += 1;
                }
                Binding::VecHandle { ty, .. } => {
                    let ptr_param = fn_val
                        .get_nth_param(param_index)
                        .ok_or_else(|| {
                            CodegenError::Llvm(format!(
                                "lambda missing captured vec ptr param {}",
                                param_index
                            ))
                        })?
                        .into_pointer_value();
                    param_index += 1;
                    let len_param = fn_val
                        .get_nth_param(param_index)
                        .ok_or_else(|| {
                            CodegenError::Llvm(format!(
                                "lambda missing captured vec len param {}",
                                param_index
                            ))
                        })?
                        .into_int_value();
                    param_index += 1;
                    captured.push(Binding::VecHandle {
                        ptr: ptr_param,
                        len: len_param,
                        ty: *ty,
                    });
                }
                Binding::Function(f) => captured.push(Binding::Function(f.clone())),
                Binding::Unavailable => captured.push(Binding::Unavailable),
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
    ) -> Result<CompiledValue<'ctx>> {
        let scrut = self.compile_expr(scrutinee, env, cur_fn)?;

        let merge_bb = self.context.append_basic_block(cur_fn, "match_end");
        // Collect (basic-block, value) for the phi at merge.
        let mut incoming: Vec<(CompiledValue<'ctx>, BasicBlock<'ctx>)> = Vec::new();
        let mut result_ty: Option<ValueTy> = None;

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
                    let v = self.compile_value_expr(body, env, cur_fn)?;
                    remember_match_type(&mut result_ty, &v.ty)?;
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
                    let v = self.compile_value_expr(body, env, cur_fn)?;
                    remember_match_type(&mut result_ty, &v.ty)?;
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
                    let v = self.compile_value_expr(body, env, cur_fn)?;
                    remember_match_type(&mut result_ty, &v.ty)?;
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
            return Ok(CompiledValue::int(self.context.i64_type().const_zero()));
        }
        let result_ty = result_ty.unwrap_or(ValueTy::Int);
        let phi = self
            .builder
            .build_phi(self.llvm_type(&result_ty)?, "match_val")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let refs: Vec<(&dyn inkwell::values::BasicValue<'ctx>, BasicBlock<'ctx>)> = incoming
            .iter()
            .map(|(v, b)| (&v.value as &dyn inkwell::values::BasicValue<'ctx>, *b))
            .collect();
        phi.add_incoming(&refs);
        Ok(CompiledValue {
            ty: result_ty,
            value: phi.as_basic_value(),
        })
    }
}

fn collect_annotated_lam_chain(node: &Node) -> Option<(usize, &Node, Option<&Node>)> {
    match node {
        Node::Ann { expr, type_ } => {
            collect_lam_chain(expr).map(|(arity, body)| (arity, body, Some(type_.as_ref())))
        }
        other => collect_lam_chain(other).map(|(arity, body)| (arity, body, None)),
    }
}

fn direct_loop_callback_body(node: &Node) -> Option<&Node> {
    match node {
        Node::Lam { body } => Some(body.as_ref()),
        Node::Ann { expr, .. } => direct_loop_callback_body(expr),
        _ => None,
    }
}

fn function_signature_from_type_node(type_node: &Node) -> Result<(Vec<ParamTy>, ValueTy)> {
    let ty = ty_from_type_node(type_node)?;
    let mut params = Vec::new();
    let mut cur = ty;
    loop {
        match cur {
            Ty::Fn(arg, ret, _) => {
                params.push(param_ty_from_ty(&arg)?);
                cur = *ret;
            }
            // The result type goes through `value_ty_from_ty`, which rejects
            // handle types: a handle may not be *returned* (ADR 0098 — up is
            // forbidden).
            other => return Ok((params, value_ty_from_ty(&other)?)),
        }
    }
}

/// Like `function_signature_from_type_node`, but fails if any parameter is a
/// handle. Best-effort value-type inference has no `ParamTy` representation,
/// so its callers degrade gracefully (`unwrap_or`) when this fails.
fn value_signature_from_type_node(type_node: &Node) -> Result<(Vec<ValueTy>, ValueTy)> {
    let (params, ret_ty) = function_signature_from_type_node(type_node)?;
    let mut values = Vec::with_capacity(params.len());
    for param in params {
        values.push(param.as_value().cloned().ok_or(CodegenError::Unsupported(
            "handle-typed parameter in value-position type inference",
        ))?);
    }
    Ok((values, ret_ty))
}

/// Map a parameter-position `Ty` to a `ParamTy`. Handle types (`Buf`,
/// `I64Vec`, `Vec`) become `ParamTy::Handle`; everything else routes through
/// `value_ty_from_ty` (ADR 0098).
fn param_ty_from_ty(ty: &Ty) -> Result<ParamTy> {
    match ty {
        Ty::Buf => Ok(ParamTy::Handle(HandleTy::Buf)),
        Ty::I64Vec => Ok(ParamTy::Handle(HandleTy::I64Vec)),
        Ty::Vec(elem) => Ok(ParamTy::Handle(HandleTy::Vec(*elem))),
        other => Ok(ParamTy::Value(value_ty_from_ty(other)?)),
    }
}

fn nested_fn_ty(param_tys: &[ValueTy], ret_ty: ValueTy) -> ValueTy {
    param_tys.iter().rev().fold(ret_ty, |acc, param| {
        ValueTy::Fn(Box::new(param.clone()), Box::new(acc))
    })
}

/// Build a closure `ValueTy::Fn` chain from a `ParamTy` list, or `None` if any
/// parameter is a handle — a function with a handle parameter has no
/// first-class function type and cannot be reified (ADR 0098).
fn try_nested_fn_ty(param_tys: &[ParamTy], ret_ty: ValueTy) -> Option<ValueTy> {
    let mut values = Vec::with_capacity(param_tys.len());
    for param in param_tys {
        values.push(param.as_value()?.clone());
    }
    Some(nested_fn_ty(&values, ret_ty))
}

fn apply_fn_spine_ty(mut ty: ValueTy, arg_count: usize) -> Result<ValueTy> {
    for _ in 0..arg_count {
        match ty {
            ValueTy::Fn(_, ret) => ty = *ret,
            _ => return Err(CodegenError::AppNonFunction),
        }
    }
    Ok(ty)
}

fn binding_tys_from_env(env: &[Binding<'_>]) -> Vec<BindingTy> {
    env.iter()
        .map(|binding| match binding {
            Binding::Value(v) => BindingTy::Value(v.ty.clone()),
            // A handle-parameter function has no first-class function type;
            // it is recorded as a non-value `Ptr` so any value-position use
            // is rejected (ADR 0098). It is still callable in App position.
            Binding::Function(f) => match f.value_param_tys() {
                Some(param_tys) => BindingTy::Function {
                    param_tys,
                    ret_ty: f.ret_ty.clone(),
                },
                None => BindingTy::Ptr,
            },
            Binding::Ptr { .. } => BindingTy::Ptr,
            Binding::VecHandle { .. } => BindingTy::Ptr,
            Binding::Unavailable => BindingTy::Ptr,
        })
        .collect()
}

fn collect_free_outer_indices(node: &Node, depth: u64, out: &mut BTreeSet<usize>) {
    match node {
        Node::Var { index } => {
            if *index >= depth {
                out.insert((*index - depth) as usize);
            }
        }
        Node::Lam { body } => collect_free_outer_indices(body, depth + 1, out),
        Node::Let { rhs, body } => {
            collect_free_outer_indices(rhs, depth, out);
            collect_free_outer_indices(body, depth + 1, out);
        }
        Node::Rec { bindings, body } => {
            let inner = depth + bindings.len() as u64;
            for binding in bindings {
                collect_free_outer_indices(binding, inner, out);
            }
            collect_free_outer_indices(body, inner, out);
        }
        Node::Module { bindings } => {
            let inner = depth + bindings.len() as u64;
            for binding in bindings {
                collect_free_outer_indices(binding, inner, out);
            }
        }
        Node::Unit { defs, .. } | Node::Defs { defs } => {
            for def in defs {
                collect_free_outer_indices(def, depth, out);
            }
        }
        Node::Def { body, .. } => collect_free_outer_indices(body, depth, out),
        Node::App { fn_, arg } => {
            collect_free_outer_indices(fn_, depth, out);
            collect_free_outer_indices(arg, depth, out);
        }
        Node::If { cond, then, else_ } => {
            collect_free_outer_indices(cond, depth, out);
            collect_free_outer_indices(then, depth, out);
            collect_free_outer_indices(else_, depth, out);
        }
        Node::Match { scrutinee, arms } => {
            collect_free_outer_indices(scrutinee, depth, out);
            for arm in arms {
                collect_free_outer_indices(arm, depth, out);
            }
        }
        Node::Arm { pattern, body } => {
            collect_free_outer_indices(body, depth + count_pat_vars_local(pattern), out);
        }
        Node::Record { fields } => {
            for (_, value) in fields {
                collect_free_outer_indices(value, depth, out);
            }
        }
        Node::Proj { record, .. } => collect_free_outer_indices(record, depth, out),
        Node::Ctor { args, .. } => {
            for arg in args {
                collect_free_outer_indices(arg, depth, out);
            }
        }
        Node::Ann { expr, .. } => collect_free_outer_indices(expr, depth, out),
        Node::PatCtor { sub_patterns, .. } => {
            for pattern in sub_patterns {
                collect_free_outer_indices(pattern, depth, out);
            }
        }
        Node::Int { .. }
        | Node::Str { .. }
        | Node::Sym { .. }
        | Node::Hole { .. }
        | Node::PatWild
        | Node::PatVar
        | Node::PatInt { .. }
        | Node::FnTy { .. }
        | Node::TyVar { .. }
        | Node::Forall { .. }
        | Node::EffSet { .. }
        | Node::EffVar { .. }
        | Node::Imports { .. }
        | Node::Import { .. }
        | Node::HostImport { .. }
        | Node::Exports { .. }
        | Node::Export { .. }
        | Node::State { .. }
        | Node::Sig { .. }
        | Node::Ref { .. } => {}
    }
}

fn count_pat_vars_local(pattern: &Node) -> u64 {
    match pattern {
        Node::PatVar => 1,
        Node::PatCtor { sub_patterns, .. } => sub_patterns.iter().map(count_pat_vars_local).sum(),
        _ => 0,
    }
}

fn ty_from_type_node(type_node: &Node) -> Result<Ty> {
    let mut subst = Subst::default();
    let mut diags = Vec::new();
    let ty = type_from_node(type_node, &[], &[], &mut subst, &[], &mut diags);
    if let Some(diag) = diags.into_iter().find(|d| d.severity == "error") {
        return Err(CodegenError::UnsupportedValueType { ty: diag.message });
    }
    Ok(subst.apply(&ty))
}

fn value_ty_from_ty(ty: &Ty) -> Result<ValueTy> {
    match ty {
        Ty::IntLit | Ty::Int | Ty::Bool | Ty::FixedInt(_) => Ok(ValueTy::Int),
        Ty::Fn(arg, ret, _) => Ok(ValueTy::Fn(
            Box::new(value_ty_from_ty(arg)?),
            Box::new(value_ty_from_ty(ret)?),
        )),
        Ty::Record(fields) => fields
            .iter()
            .map(|(name, ty)| Ok((name.clone(), value_ty_from_ty(ty)?)))
            .collect::<Result<Vec<_>>>()
            .map(ValueTy::Record),
        Ty::Unknown | Ty::Meta(_) => Err(CodegenError::UnsupportedValueType { ty: ty.to_string() }),
        Ty::Str | Ty::Buf | Ty::I64Vec | Ty::Vec(_) | Ty::App(_, _) => {
            Err(CodegenError::UnsupportedValueType { ty: ty.to_string() })
        }
    }
}

impl<'ctx> Compiler<'ctx> {
    fn infer_value_ty(&self, node: &Node, env: &[BindingTy]) -> Result<ValueTy> {
        match node {
            Node::Int { .. } => Ok(ValueTy::Int),
            Node::Str { .. } => Err(CodegenError::UnsupportedValueType { ty: "Str".into() }),
            Node::Var { index } => match lookup_binding_ty(env, *index)? {
                BindingTy::Value(ty) => Ok(ty.clone()),
                BindingTy::Function { param_tys, ret_ty } => {
                    Ok(nested_fn_ty(param_tys, ret_ty.clone()))
                }
                BindingTy::Ptr => Err(CodegenError::UnsupportedValueType {
                    ty: "non-escapable pointer handle".into(),
                }),
            },
            Node::Let { rhs, body } => {
                if let Some((arity, lam_body, ann_ty)) = collect_annotated_lam_chain(rhs) {
                    let (param_tys, ret_ty) = if let Some(type_node) = ann_ty {
                        let sig = value_signature_from_type_node(type_node)?;
                        if sig.0.len() != arity {
                            return Err(CodegenError::FunctionArity {
                                expected: sig.0.len(),
                                got: arity,
                            });
                        }
                        sig
                    } else {
                        let param_tys = vec![ValueTy::Int; arity];
                        let lambda_env: Vec<BindingTy> = param_tys
                            .iter()
                            .rev()
                            .cloned()
                            .map(BindingTy::Value)
                            .collect();
                        let ret_ty = self
                            .infer_value_ty(lam_body, &lambda_env)
                            .unwrap_or(ValueTy::Int);
                        (param_tys, ret_ty)
                    };
                    if check_closed(lam_body, arity as u64).is_ok() {
                        let mut body_env = vec![BindingTy::Function { param_tys, ret_ty }];
                        body_env.extend_from_slice(env);
                        self.infer_value_ty(body, &body_env)
                    } else {
                        let rhs_ty = nested_fn_ty(&param_tys, ret_ty);
                        let mut body_env = vec![BindingTy::Value(rhs_ty)];
                        body_env.extend_from_slice(env);
                        self.infer_value_ty(body, &body_env)
                    }
                } else {
                    let rhs_ty = self.infer_value_ty(rhs, env)?;
                    let mut body_env = vec![BindingTy::Value(rhs_ty)];
                    body_env.extend_from_slice(env);
                    self.infer_value_ty(body, &body_env)
                }
            }
            Node::If { then, else_, .. } => {
                let then_ty = self.infer_value_ty(then, env)?;
                let else_ty = self.infer_value_ty(else_, env)?;
                if then_ty == else_ty {
                    Ok(then_ty)
                } else {
                    Err(CodegenError::ValueTypeMismatch {
                        expected: then_ty.to_string(),
                        actual: else_ty.to_string(),
                    })
                }
            }
            Node::App { .. } => {
                let (head, args) = unfold_app(node);
                match head {
                    Node::Sym { name } => {
                        // ADR 0093: @loop and @loop-step / @loop-exit have
                        // result types that depend on argument types, not a
                        // fixed scalar.  Recurse to infer init/value types.
                        match name.as_str() {
                            "loop" if args.len() == 2 => return self.infer_value_ty(args[0], env),
                            "loop-step" | "loop-exit" if args.len() == 1 => {
                                let inner = self.infer_value_ty(args[0], env)?;
                                return Ok(ValueTy::Record(vec![
                                    ("tag".to_string(), ValueTy::Int),
                                    ("value".to_string(), inner),
                                ]));
                            }
                            _ => {}
                        }
                        primitive_value_ty(name, args.len())
                    }
                    Node::Var { index } => match lookup_binding_ty(env, *index)? {
                        BindingTy::Function { param_tys, ret_ty } => {
                            apply_fn_spine_ty(nested_fn_ty(param_tys, ret_ty.clone()), args.len())
                        }
                        BindingTy::Value(ty) => apply_fn_spine_ty(ty.clone(), args.len()),
                        BindingTy::Ptr => Err(CodegenError::AppNonFunction),
                    },
                    Node::Lam { .. } | Node::Ann { .. } => {
                        let (arity, lam_body, ann_ty) = collect_annotated_lam_chain(head)
                            .ok_or(CodegenError::AppNonFunction)?;
                        let fn_ty = if let Some(type_node) = ann_ty {
                            value_ty_from_ty(&ty_from_type_node(type_node)?)?
                        } else {
                            let param_tys = vec![ValueTy::Int; arity];
                            let lambda_env: Vec<BindingTy> = param_tys
                                .iter()
                                .rev()
                                .cloned()
                                .map(BindingTy::Value)
                                .collect();
                            let ret_ty = self.infer_value_ty(lam_body, &lambda_env)?;
                            nested_fn_ty(&param_tys, ret_ty)
                        };
                        apply_fn_spine_ty(fn_ty, args.len())
                    }
                    _ => apply_fn_spine_ty(self.infer_value_ty(head, env)?, args.len()),
                }
            }
            Node::Lam { body } => {
                let param_ty = ValueTy::Int;
                let mut body_env = vec![BindingTy::Value(param_ty.clone())];
                body_env.extend_from_slice(env);
                let ret_ty = self.infer_value_ty(body, &body_env)?;
                Ok(ValueTy::Fn(Box::new(param_ty), Box::new(ret_ty)))
            }
            Node::Rec { bindings, body } => {
                let mut rec_types = Vec::with_capacity(bindings.len());
                for binding in bindings {
                    let (arity, _lam_body, ann_ty) = collect_annotated_lam_chain(binding)
                        .ok_or(CodegenError::Unsupported("rec member that is not a lambda"))?;
                    let (param_tys, ret_ty) = if let Some(type_node) = ann_ty {
                        value_signature_from_type_node(type_node)?
                    } else {
                        (vec![ValueTy::Int; arity], ValueTy::Int)
                    };
                    rec_types.push(BindingTy::Function { param_tys, ret_ty });
                }
                let mut body_env = rec_types;
                body_env.extend_from_slice(env);
                self.infer_value_ty(body, &body_env)
            }
            Node::Module { .. } => Err(CodegenError::Unsupported("module binding group")),
            Node::Unit { .. } => Err(CodegenError::Unsupported("logical unit artifact")),
            Node::Match { arms, .. } => {
                let mut result_ty = None;
                for arm in arms {
                    let Node::Arm { body, .. } = arm else {
                        return Err(CodegenError::Unsupported("match child must be arm"));
                    };
                    let arm_ty = self.infer_value_ty(body, env)?;
                    remember_match_type(&mut result_ty, &arm_ty)?;
                }
                Ok(result_ty.unwrap_or(ValueTy::Int))
            }
            Node::Arm { .. } => Err(CodegenError::Unsupported("bare arm outside match")),
            Node::Record { fields } => {
                let mut ordered: Vec<&(String, Node)> = fields.iter().collect();
                ordered.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
                ordered
                    .into_iter()
                    .map(|(name, value)| Ok((name.clone(), self.infer_value_ty(value, env)?)))
                    .collect::<Result<Vec<_>>>()
                    .map(ValueTy::Record)
            }
            Node::Proj { record, field } => {
                let rec_ty = self.infer_value_ty(record, env)?;
                let ValueTy::Record(fields) = rec_ty else {
                    return Err(CodegenError::InvalidProjection {
                        field: field.clone(),
                        actual: rec_ty.to_string(),
                    });
                };
                fields
                    .iter()
                    .find(|(name, _)| name == field)
                    .map(|(_, ty)| ty.clone())
                    .ok_or_else(|| CodegenError::MissingField {
                        field: field.clone(),
                    })
            }
            Node::Ctor { .. } => Err(CodegenError::Unsupported("ctor in expression position")),
            Node::Ann { expr, type_ } => {
                let _ = expr;
                value_ty_from_ty(&ty_from_type_node(type_)?)
            }
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
            // A bare reference to an imported definition in value position.
            // Internal package refs are expanded inline before codegen, so a
            // surviving `Ref` is a host import: recover its function type from
            // the registered host-import binding so it can be passed as a
            // first-class callback value (ADR 0098 lineage; the typed-vector
            // handle case stays unsupported because such imports are not
            // registered as reifiable bindings).
            Node::Ref { hash } => match self.host_imports.get(hash) {
                Some(binding) => binding
                    .value_param_tys()
                    .map(|param_tys| nested_fn_ty(&param_tys, binding.ret_ty.clone()))
                    .ok_or(CodegenError::Unsupported(
                        "host import with a handle-typed parameter in value position",
                    )),
                None => Err(CodegenError::Unsupported(
                    "unit artifact node in value position",
                )),
            },
            Node::Imports { .. }
            | Node::Import { .. }
            | Node::HostImport { .. }
            | Node::Exports { .. }
            | Node::Export { .. }
            | Node::Defs { .. }
            | Node::Def { .. }
            | Node::State { .. }
            | Node::Sig { .. } => Err(CodegenError::Unsupported(
                "unit artifact node in value position",
            )),
        }
    }
}

fn primitive_value_ty(name: &str, arg_count: usize) -> Result<ValueTy> {
    let Some(kind) = PrimKind::lookup(name) else {
        return Ok(ValueTy::Int);
    };
    if arg_count != kind.arity() {
        return Ok(ValueTy::Int);
    }
    match kind {
        PrimKind::Fixed(FixedPrim::Arith {
            mode: FixedArithMode::Check,
            ..
        }) => Ok(ValueTy::Record(vec![
            ("ok".to_string(), ValueTy::Int),
            ("value".to_string(), ValueTy::Int),
        ])),
        _ => Ok(ValueTy::Int),
    }
}

fn lookup_binding_ty(env: &[BindingTy], idx: u64) -> Result<&BindingTy> {
    env.get(idx as usize)
        .ok_or(CodegenError::FreeVarInLambda { index: idx })
}

fn remember_match_type(slot: &mut Option<ValueTy>, ty: &ValueTy) -> Result<()> {
    match slot {
        None => {
            *slot = Some(ty.clone());
            Ok(())
        }
        Some(existing) if existing == ty => Ok(()),
        Some(existing) => Err(CodegenError::ValueTypeMismatch {
            expected: existing.to_string(),
            actual: ty.to_string(),
        }),
    }
}

fn fixed_mask(width: u16) -> u64 {
    if width == 64 {
        u64::MAX
    } else {
        (1u64 << width) - 1
    }
}

fn fixed_sign_bit(width: u16) -> u64 {
    1u64 << (width - 1)
}

fn fixed_signed_min(width: u16) -> i64 {
    if width == 64 {
        i64::MIN
    } else {
        -(1i64 << (width - 1))
    }
}

fn fixed_signed_max(width: u16) -> i64 {
    if width == 64 {
        i64::MAX
    } else {
        (1i64 << (width - 1)) - 1
    }
}

fn lookup_var<'a, 'ctx>(env: &'a [Binding<'ctx>], idx: u64) -> Result<&'a Binding<'ctx>> {
    let i = idx as usize;
    if i >= env.len() {
        return Err(CodegenError::FreeVarInLambda { index: idx });
    }
    Ok(&env[i])
}

// =========================================================================
// Host-library emission per ADR 0088 and ADR 0092.
//
// `compile_library_to_object` accepts a `PackageLibrary` (built by the
// typechecker from a checked package + its host interface) and emits an
// LLVM object exposing one extern "C" wrapper per public export. Host
// imports are dispatched indirectly through a per-package thread-local
// pointer to the host-supplied `tacit_p_<pkg>_context` struct.
// =========================================================================

use tacit_typecheck::library::{
    LibAbiType, LibInstance, LibRecord, LibScalar, LibStateField, LibStateType, LibraryExport,
    LibraryImport, PackageLibrary,
};

const TACIT_STATUS_OK: u64 = 0;
const TACIT_STATUS_BAD_ARGUMENT: u64 = 1;
const TACIT_STATUS_OUT_OF_MEMORY: u64 = 4;

pub fn compile_library_to_object(
    spec: &PackageLibrary,
    module_name: &str,
    out_path: &Path,
) -> Result<()> {
    let context = Context::create();
    let mut compiler = Compiler::new(&context, module_name);
    compiler.compile_package_library(spec)?;
    compiler.write_object(out_path)
}

pub fn compile_library_to_ir_string(spec: &PackageLibrary, module_name: &str) -> Result<String> {
    let context = Context::create();
    let mut compiler = Compiler::new(&context, module_name);
    compiler.compile_package_library(spec)?;
    Ok(compiler.print_to_string())
}

impl<'ctx> Compiler<'ctx> {
    pub fn compile_package_library(&mut self, spec: &PackageLibrary) -> Result<()> {
        let ptr_t = self.context.ptr_type(AddressSpace::default());

        // {ptr user, ptr callbacks}. Per-package by hash.
        let ctx_struct_ty = self
            .context
            .struct_type(&[ptr_t.into(), ptr_t.into()], false);

        let callback_field_count = spec.imports.len().max(1);
        let callback_fields: Vec<BasicTypeEnum<'ctx>> =
            (0..callback_field_count).map(|_| ptr_t.into()).collect();
        let callbacks_struct_ty = self.context.struct_type(&callback_fields, false);

        let tls_name = format!("{}_current_ctx", spec.package_prefix);
        let tls_global = self.module.add_global(ptr_t, None, &tls_name);
        tls_global.set_thread_local(true);
        tls_global.set_initializer(&ptr_t.const_null());
        tls_global.set_linkage(Linkage::Internal);

        let state_runtime = if let Some(instance) = &spec.instance {
            Some(self.compile_instance_runtime(instance, &spec.package_prefix)?)
        } else {
            None
        };
        self.state_runtime = state_runtime;

        for import in &spec.imports {
            self.compile_host_import_trampoline(
                import,
                &spec.package_prefix,
                ctx_struct_ty,
                callbacks_struct_ty,
                tls_global,
            )?;
        }

        for export in &spec.exports {
            self.compile_library_export(export, ctx_struct_ty, tls_global)?;
        }

        self.state_runtime = None;

        self.module
            .verify()
            .map_err(|e| CodegenError::Llvm(e.to_string_lossy().into_owned()))?;
        Ok(())
    }

    fn abi_llvm_type(&self, ty: &LibAbiType) -> Result<BasicTypeEnum<'ctx>> {
        match ty {
            LibAbiType::Unit => Ok(self
                .context
                .struct_type(&[self.context.i8_type().into()], false)
                .into()),
            LibAbiType::Scalar(scalar) => Ok(scalar_int_type(self.context, *scalar).into()),
            LibAbiType::Record(record) => Ok(self.abi_record_struct_type(record)?.into()),
            LibAbiType::BorrowedVector(_) => Ok(self.borrowed_vec_struct_type().into()),
        }
    }

    fn borrowed_vec_struct_type(&self) -> inkwell::types::StructType<'ctx> {
        let ptr_t = self.context.ptr_type(AddressSpace::default());
        self.context
            .struct_type(&[ptr_t.into(), self.context.i64_type().into()], false)
    }

    fn compile_instance_runtime(
        &mut self,
        instance: &LibInstance,
        package_prefix: &str,
    ) -> Result<StateRuntime<'ctx>> {
        let ptr_t = self.context.ptr_type(AddressSpace::default());
        let status_t = self.context.i32_type();
        let fields = self.state_layout_fields(&instance.fields)?;
        let field_tys = fields
            .iter()
            .map(|field| self.state_field_basic_type(&field.ty))
            .collect::<Vec<_>>();
        let instance_ty = self.context.struct_type(&field_tys, false);

        let instance_tls_name = format!("{package_prefix}_current_instance");
        let instance_tls = self.module.add_global(ptr_t, None, &instance_tls_name);
        instance_tls.set_thread_local(true);
        instance_tls.set_initializer(&ptr_t.const_null());
        instance_tls.set_linkage(Linkage::Internal);

        let status_tls_name = format!("{package_prefix}_current_status");
        let status_tls = self.module.add_global(status_t, None, &status_tls_name);
        status_tls.set_thread_local(true);
        status_tls.set_initializer(&status_t.const_zero());
        status_tls.set_linkage(Linkage::Internal);

        let runtime = StateRuntime {
            instance_ty,
            instance_tls,
            status_tls,
            fields,
        };
        self.compile_instance_create(instance, &runtime)?;
        self.compile_instance_destroy(instance, &runtime)?;
        Ok(runtime)
    }

    fn state_layout_fields(&self, fields: &[LibStateField]) -> Result<Vec<StateFieldLayout<'ctx>>> {
        fields
            .iter()
            .map(|field| {
                Ok(StateFieldLayout {
                    name: field.name.clone(),
                    ty: self.state_layout_ty(&field.ty)?,
                })
            })
            .collect()
    }

    fn state_layout_ty(&self, ty: &LibStateType) -> Result<StateFieldTy<'ctx>> {
        match ty {
            LibStateType::Scalar(scalar) => Ok(StateFieldTy::Scalar(*scalar)),
            LibStateType::Vec(elem) => Ok(StateFieldTy::Vec {
                elem: *elem,
                slot_ty: self.borrowed_vec_struct_type(),
            }),
            LibStateType::Record(fields) => {
                let fields = self.state_layout_fields(fields)?;
                let field_tys = fields
                    .iter()
                    .map(|field| self.state_field_basic_type(&field.ty))
                    .collect::<Vec<_>>();
                Ok(StateFieldTy::Record {
                    struct_ty: self.context.struct_type(&field_tys, false),
                    fields,
                })
            }
        }
    }

    fn state_field_basic_type(&self, ty: &StateFieldTy<'ctx>) -> BasicTypeEnum<'ctx> {
        match ty {
            StateFieldTy::Scalar(scalar) => scalar_int_type(self.context, *scalar).into(),
            StateFieldTy::Record { struct_ty, .. } => (*struct_ty).into(),
            StateFieldTy::Vec { slot_ty, .. } => (*slot_ty).into(),
        }
    }

    fn compile_instance_create(
        &mut self,
        instance: &LibInstance,
        runtime: &StateRuntime<'ctx>,
    ) -> Result<()> {
        let ptr_t = self.context.ptr_type(AddressSpace::default());
        let i32_t = self.context.i32_type();
        let fn_ty = i32_t.fn_type(&[ptr_t.into(), ptr_t.into()], false);
        let f = self
            .module
            .add_function(&instance.create_symbol, fn_ty, Some(Linkage::External));
        let saved_block = self.builder.get_insert_block();
        let entry = self.context.append_basic_block(f, "entry");
        let bad_arg = self.context.append_basic_block(f, "bad_arg");
        let alloc = self.context.append_basic_block(f, "alloc");
        let oom = self.context.append_basic_block(f, "oom");
        let ok = self.context.append_basic_block(f, "ok");

        self.builder.position_at_end(entry);
        let ctx = f
            .get_nth_param(0)
            .ok_or_else(|| CodegenError::Llvm("create missing ctx".into()))?
            .into_pointer_value();
        let out = f
            .get_nth_param(1)
            .ok_or_else(|| CodegenError::Llvm("create missing out".into()))?
            .into_pointer_value();
        let ctx_null = self
            .builder
            .build_is_null(ctx, "ctx_null")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let out_null = self
            .builder
            .build_is_null(out, "out_null")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let invalid = self
            .builder
            .build_or(ctx_null, out_null, "create_bad_arg")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(invalid, bad_arg, alloc)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(bad_arg);
        self.builder
            .build_return(Some(&i32_t.const_int(TACIT_STATUS_BAD_ARGUMENT, false)))
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(alloc);
        let instance_ptr = self
            .builder
            .build_malloc(runtime.instance_ty, "instance")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let is_null = self
            .builder
            .build_is_null(instance_ptr, "instance_null")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(is_null, oom, ok)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(oom);
        self.builder
            .build_return(Some(&i32_t.const_int(TACIT_STATUS_OUT_OF_MEMORY, false)))
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(ok);
        self.builder
            .build_store(instance_ptr, runtime.instance_ty.const_zero())
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_store(out, instance_ptr)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_return(Some(&i32_t.const_int(TACIT_STATUS_OK, false)))
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        if let Some(saved) = saved_block {
            self.builder.position_at_end(saved);
        }
        Ok(())
    }

    fn compile_instance_destroy(
        &mut self,
        instance: &LibInstance,
        runtime: &StateRuntime<'ctx>,
    ) -> Result<()> {
        let ptr_t = self.context.ptr_type(AddressSpace::default());
        let i32_t = self.context.i32_type();
        let fn_ty = i32_t.fn_type(&[ptr_t.into(), ptr_t.into()], false);
        let f = self
            .module
            .add_function(&instance.destroy_symbol, fn_ty, Some(Linkage::External));
        let saved_block = self.builder.get_insert_block();
        let entry = self.context.append_basic_block(f, "entry");
        let bad_arg = self.context.append_basic_block(f, "bad_arg");
        let run = self.context.append_basic_block(f, "run");

        self.builder.position_at_end(entry);
        let instance_ptr = f
            .get_nth_param(1)
            .ok_or_else(|| CodegenError::Llvm("destroy missing instance".into()))?
            .into_pointer_value();
        let instance_null = self
            .builder
            .build_is_null(instance_ptr, "instance_null")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(instance_null, bad_arg, run)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(bad_arg);
        self.builder
            .build_return(Some(&i32_t.const_int(TACIT_STATUS_BAD_ARGUMENT, false)))
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(run);
        self.emit_free_state_vec_fields(
            instance_ptr,
            runtime.instance_ty,
            &runtime.fields,
            f,
            "destroy",
        )?;
        self.builder
            .build_free(instance_ptr)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_return(Some(&i32_t.const_int(TACIT_STATUS_OK, false)))
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        if let Some(saved) = saved_block {
            self.builder.position_at_end(saved);
        }
        Ok(())
    }

    fn emit_free_state_vec_fields(
        &mut self,
        base_ptr: PointerValue<'ctx>,
        struct_ty: inkwell::types::StructType<'ctx>,
        fields: &[StateFieldLayout<'ctx>],
        cur_fn: FunctionValue<'ctx>,
        name: &str,
    ) -> Result<()> {
        for (index, field) in fields.iter().enumerate() {
            let field_ptr = self
                .builder
                .build_struct_gep(struct_ty, base_ptr, index as u32, &format!("{name}_field"))
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
            match &field.ty {
                StateFieldTy::Vec { slot_ty, .. } => {
                    let data_pp = self
                        .builder
                        .build_struct_gep(*slot_ty, field_ptr, 0, &format!("{name}_vec_data_pp"))
                        .map_err(|e| CodegenError::Llvm(e.to_string()))?;
                    let data = self
                        .builder
                        .build_load(
                            self.context.ptr_type(AddressSpace::default()),
                            data_pp,
                            &format!("{name}_vec_data"),
                        )
                        .map_err(|e| CodegenError::Llvm(e.to_string()))?
                        .into_pointer_value();
                    let is_null = self
                        .builder
                        .build_is_null(data, &format!("{name}_vec_null"))
                        .map_err(|e| CodegenError::Llvm(e.to_string()))?;
                    let free_bb = self
                        .context
                        .append_basic_block(cur_fn, &format!("{name}_vec_free"));
                    let cont_bb = self
                        .context
                        .append_basic_block(cur_fn, &format!("{name}_vec_next"));
                    self.builder
                        .build_conditional_branch(is_null, cont_bb, free_bb)
                        .map_err(|e| CodegenError::Llvm(e.to_string()))?;
                    self.builder.position_at_end(free_bb);
                    self.builder
                        .build_free(data)
                        .map_err(|e| CodegenError::Llvm(e.to_string()))?;
                    self.builder
                        .build_unconditional_branch(cont_bb)
                        .map_err(|e| CodegenError::Llvm(e.to_string()))?;
                    self.builder.position_at_end(cont_bb);
                    self.builder
                        .build_store(field_ptr, slot_ty.const_zero())
                        .map_err(|e| CodegenError::Llvm(e.to_string()))?;
                }
                StateFieldTy::Record { struct_ty, fields } => {
                    self.emit_free_state_vec_fields(
                        field_ptr,
                        *struct_ty,
                        fields,
                        cur_fn,
                        &format!("{name}_record"),
                    )?;
                }
                StateFieldTy::Scalar(_) => {}
            }
        }
        Ok(())
    }

    fn state_runtime(&self) -> Result<StateRuntime<'ctx>> {
        self.state_runtime.clone().ok_or(CodegenError::Unsupported(
            "state primitive outside instance library",
        ))
    }

    fn current_instance_ptr(&mut self, runtime: &StateRuntime<'ctx>) -> Result<PointerValue<'ctx>> {
        self.builder
            .build_load(
                self.context.ptr_type(AddressSpace::default()),
                runtime.instance_tls.as_pointer_value(),
                "current_instance",
            )
            .map_err(|e| CodegenError::Llvm(e.to_string()))
            .map(|value| value.into_pointer_value())
    }

    fn top_state_field(
        runtime: &StateRuntime<'ctx>,
        name: &str,
    ) -> Result<(usize, StateFieldLayout<'ctx>)> {
        runtime
            .fields
            .iter()
            .cloned()
            .enumerate()
            .find(|(_, field)| field.name == name)
            .ok_or_else(|| CodegenError::MissingField {
                field: name.to_string(),
            })
    }

    fn state_field_name_arg(node: &Node) -> Result<&str> {
        match node {
            Node::Sym { name } => Ok(name),
            _ => Err(CodegenError::Unsupported(
                "state field argument must be a literal symbol",
            )),
        }
    }

    fn state_field_ptr(
        &mut self,
        runtime: &StateRuntime<'ctx>,
        name: &str,
    ) -> Result<(PointerValue<'ctx>, StateFieldLayout<'ctx>)> {
        let (index, field) = Self::top_state_field(runtime, name)?;
        let instance = self.current_instance_ptr(runtime)?;
        let ptr = self
            .builder
            .build_struct_gep(runtime.instance_ty, instance, index as u32, "state_field")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        Ok((ptr, field))
    }

    fn value_ty_for_state_field(ty: &StateFieldTy<'ctx>) -> Result<ValueTy> {
        match ty {
            StateFieldTy::Scalar(_) => Ok(ValueTy::Int),
            StateFieldTy::Record { fields, .. } => fields
                .iter()
                .map(|field| {
                    Ok((
                        field.name.clone(),
                        Self::value_ty_for_state_field(&field.ty)?,
                    ))
                })
                .collect::<Result<Vec<_>>>()
                .map(ValueTy::Record),
            StateFieldTy::Vec { .. } => Err(CodegenError::UnsupportedValueType {
                ty: "state vector handle".to_string(),
            }),
        }
    }

    fn emit_state_load(
        &mut self,
        args: &[&Node],
        _env: &[Binding<'ctx>],
        _cur_fn: FunctionValue<'ctx>,
    ) -> Result<CompiledValue<'ctx>> {
        let runtime = self.state_runtime()?;
        let field_name = Self::state_field_name_arg(args[0])?;
        let (field_ptr, field) = self.state_field_ptr(&runtime, field_name)?;
        match field.ty {
            StateFieldTy::Scalar(scalar) => {
                let raw = self
                    .builder
                    .build_load(
                        scalar_int_type(self.context, scalar),
                        field_ptr,
                        "state_load",
                    )
                    .map_err(|e| CodegenError::Llvm(e.to_string()))?
                    .into_int_value();
                let value =
                    extend_scalar_to_i64(&self.builder, raw, self.context.i64_type(), scalar)?;
                Ok(CompiledValue::int(value))
            }
            StateFieldTy::Record { struct_ty, fields } => {
                let raw = self
                    .builder
                    .build_load(struct_ty, field_ptr, "state_load_record")
                    .map_err(|e| CodegenError::Llvm(e.to_string()))?;
                Ok(CompiledValue {
                    ty: ValueTy::Record(
                        fields
                            .iter()
                            .map(|field| {
                                Ok((
                                    field.name.clone(),
                                    Self::value_ty_for_state_field(&field.ty)?,
                                ))
                            })
                            .collect::<Result<Vec<_>>>()?,
                    ),
                    value: raw,
                })
            }
            StateFieldTy::Vec { .. } => Err(CodegenError::Unsupported(
                "@state-load of a vector field must be the direct RHS of a let binding",
            )),
        }
    }

    fn emit_state_store(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let runtime = self.state_runtime()?;
        let field_name = Self::state_field_name_arg(args[0])?;
        let (field_ptr, field) = self.state_field_ptr(&runtime, field_name)?;
        match field.ty {
            StateFieldTy::Scalar(scalar) => {
                let value = self.compile_expr(args[1], env, cur_fn)?;
                let stored = trunc_i64_to_scalar(
                    &self.builder,
                    value,
                    scalar_int_type(self.context, scalar),
                    scalar,
                )?;
                self.builder
                    .build_store(field_ptr, stored)
                    .map_err(|e| CodegenError::Llvm(e.to_string()))?;
            }
            StateFieldTy::Record { .. } => {
                let value = self.compile_value_expr(args[1], env, cur_fn)?;
                self.builder
                    .build_store(field_ptr, value.value)
                    .map_err(|e| CodegenError::Llvm(e.to_string()))?;
            }
            StateFieldTy::Vec { .. } => {
                return Err(CodegenError::Unsupported(
                    "@state-store cannot store vector fields",
                ));
            }
        }
        Ok(self.context.i64_type().const_zero())
    }

    fn emit_state_alloc_vec(
        &mut self,
        args: &[&Node],
        env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let runtime = self.state_runtime()?;
        let field_name = Self::state_field_name_arg(args[0])?;
        let (field_ptr, field) = self.state_field_ptr(&runtime, field_name)?;
        let StateFieldTy::Vec { elem, slot_ty } = field.ty else {
            return Err(CodegenError::Unsupported(
                "@state-alloc-vec requires vector field",
            ));
        };
        let count = self.compile_expr(args[1], env, cur_fn)?;
        let data_pp = self
            .builder
            .build_struct_gep(slot_ty, field_ptr, 0, "state_vec_data_pp")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let len_pp = self
            .builder
            .build_struct_gep(slot_ty, field_ptr, 1, "state_vec_len_pp")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let old_len = self
            .builder
            .build_load(self.context.i64_type(), len_pp, "state_vec_old_len")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_int_value();
        let old_nonzero = self
            .builder
            .build_int_compare(
                IntPredicate::NE,
                old_len,
                self.context.i64_type().const_zero(),
                "state_vec_not_empty",
            )
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let bad_bb = self.context.append_basic_block(cur_fn, "state_alloc_bad");
        let alloc_bb = self.context.append_basic_block(cur_fn, "state_alloc");
        let oom_bb = self.context.append_basic_block(cur_fn, "state_alloc_oom");
        let ok_bb = self.context.append_basic_block(cur_fn, "state_alloc_ok");
        let cont_bb = self.context.append_basic_block(cur_fn, "state_alloc_cont");
        self.builder
            .build_conditional_branch(old_nonzero, bad_bb, alloc_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(bad_bb);
        self.store_runtime_status(&runtime, TACIT_STATUS_BAD_ARGUMENT)?;
        self.builder
            .build_unconditional_branch(cont_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(alloc_bb);
        let elem_ty = self.llvm_int_type_for_width(elem.width);
        let data = self
            .builder
            .build_array_malloc(elem_ty, count, "state_vec_alloc")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let data_null = self
            .builder
            .build_is_null(data, "state_vec_alloc_null")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(data_null, oom_bb, ok_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(oom_bb);
        self.store_runtime_status(&runtime, TACIT_STATUS_OUT_OF_MEMORY)?;
        self.builder
            .build_unconditional_branch(cont_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(ok_bb);
        self.builder
            .build_store(data_pp, data)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_store(len_pp, count)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(cont_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(cont_bb);
        Ok(self.context.i64_type().const_zero())
    }

    fn emit_state_free_vec(
        &mut self,
        args: &[&Node],
        _env: &[Binding<'ctx>],
        cur_fn: FunctionValue<'ctx>,
    ) -> Result<IntValue<'ctx>> {
        let runtime = self.state_runtime()?;
        let field_name = Self::state_field_name_arg(args[0])?;
        let (field_ptr, field) = self.state_field_ptr(&runtime, field_name)?;
        let StateFieldTy::Vec { slot_ty, .. } = field.ty else {
            return Err(CodegenError::Unsupported(
                "@state-free-vec requires vector field",
            ));
        };
        let data_pp = self
            .builder
            .build_struct_gep(slot_ty, field_ptr, 0, "state_free_data_pp")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let data = self
            .builder
            .build_load(
                self.context.ptr_type(AddressSpace::default()),
                data_pp,
                "state_free_data",
            )
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_pointer_value();
        let is_null = self
            .builder
            .build_is_null(data, "state_free_null")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let free_bb = self.context.append_basic_block(cur_fn, "state_free_do");
        let cont_bb = self.context.append_basic_block(cur_fn, "state_free_cont");
        self.builder
            .build_conditional_branch(is_null, cont_bb, free_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder.position_at_end(free_bb);
        self.builder
            .build_free(data)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unconditional_branch(cont_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder.position_at_end(cont_bb);
        self.builder
            .build_store(field_ptr, slot_ty.const_zero())
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        Ok(self.context.i64_type().const_zero())
    }

    fn store_runtime_status(&mut self, runtime: &StateRuntime<'ctx>, status: u64) -> Result<()> {
        self.builder
            .build_store(
                runtime.status_tls.as_pointer_value(),
                self.context.i32_type().const_int(status, false),
            )
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        Ok(())
    }

    fn abi_record_struct_type(
        &self,
        record: &LibRecord,
    ) -> Result<inkwell::types::StructType<'ctx>> {
        let fields = record
            .fields
            .iter()
            .map(|field| self.abi_llvm_type(&field.ty))
            .collect::<Result<Vec<_>>>()?;
        Ok(self.context.struct_type(&fields, false))
    }

    fn internal_value_ty_for_abi(ty: &LibAbiType) -> Result<ValueTy> {
        match ty {
            LibAbiType::Unit => Ok(ValueTy::Record(Vec::new())),
            LibAbiType::Scalar(_) => Ok(ValueTy::Int),
            LibAbiType::Record(record) => record
                .fields
                .iter()
                .map(|field| {
                    Ok((
                        field.name.clone(),
                        Self::internal_value_ty_for_abi(&field.ty)?,
                    ))
                })
                .collect::<Result<Vec<_>>>()
                .map(ValueTy::Record),
            LibAbiType::BorrowedVector(vec_ty) => Err(CodegenError::UnsupportedValueType {
                ty: format!("{}vec", vec_ty),
            }),
        }
    }

    fn internal_param_types_for_abi(
        &self,
        ty: &LibAbiType,
        out: &mut Vec<BasicMetadataTypeEnum<'ctx>>,
    ) -> Result<()> {
        match ty {
            LibAbiType::BorrowedVector(_) => {
                out.push(BasicMetadataTypeEnum::PointerType(
                    self.context.ptr_type(AddressSpace::default()),
                ));
                out.push(BasicMetadataTypeEnum::IntType(self.context.i64_type()));
            }
            _ => out.push(
                self.llvm_type(&Self::internal_value_ty_for_abi(ty)?)?
                    .into(),
            ),
        }
        Ok(())
    }

    fn abi_to_internal_value(
        &mut self,
        abi_value: BasicValueEnum<'ctx>,
        abi_ty: &LibAbiType,
        name: &str,
    ) -> Result<CompiledValue<'ctx>> {
        match abi_ty {
            LibAbiType::Unit => {
                let internal_ty = ValueTy::Record(Vec::new());
                let value = self.llvm_struct_type(&internal_ty)?.get_undef().into();
                Ok(CompiledValue {
                    ty: internal_ty,
                    value,
                })
            }
            LibAbiType::Scalar(scalar) => {
                let raw = abi_value.into_int_value();
                let value =
                    extend_scalar_to_i64(&self.builder, raw, self.context.i64_type(), *scalar)?;
                Ok(CompiledValue::int(value))
            }
            LibAbiType::Record(record) => {
                let abi_struct = abi_value.into_struct_value();
                let mut internal_fields = Vec::with_capacity(record.fields.len());
                let mut internal_values = Vec::with_capacity(record.fields.len());
                for (i, field) in record.fields.iter().enumerate() {
                    let field_abi = self
                        .builder
                        .build_extract_value(abi_struct, i as u32, &format!("{name}_field"))
                        .map_err(|e| CodegenError::Llvm(e.to_string()))?;
                    let field_value =
                        self.abi_to_internal_value(field_abi, &field.ty, &field.name)?;
                    internal_fields.push((field.name.clone(), field_value.ty.clone()));
                    internal_values.push(field_value);
                }
                let internal_ty = ValueTy::Record(internal_fields);
                let internal_struct_ty = self.llvm_struct_type(&internal_ty)?;
                let mut aggregate = internal_struct_ty.get_undef();
                for (i, field) in internal_values.into_iter().enumerate() {
                    aggregate = self
                        .builder
                        .build_insert_value(aggregate, field.value, i as u32, "abi_record_in")
                        .map_err(|e| CodegenError::Llvm(e.to_string()))?
                        .into_struct_value();
                }
                Ok(CompiledValue {
                    ty: internal_ty,
                    value: aggregate.into(),
                })
            }
            LibAbiType::BorrowedVector(vec_ty) => Err(CodegenError::UnsupportedValueType {
                ty: format!("{}vec", vec_ty),
            }),
        }
    }

    fn internal_to_abi_value(
        &mut self,
        value: &CompiledValue<'ctx>,
        abi_ty: &LibAbiType,
        name: &str,
    ) -> Result<BasicValueEnum<'ctx>> {
        match abi_ty {
            LibAbiType::Unit => {
                let unit_ty = self.abi_llvm_type(abi_ty)?.into_struct_type();
                let mut unit = unit_ty.get_undef();
                unit = self
                    .builder
                    .build_insert_value(unit, self.context.i8_type().const_zero(), 0, "abi_unit")
                    .map_err(|e| CodegenError::Llvm(e.to_string()))?
                    .into_struct_value();
                Ok(unit.into())
            }
            LibAbiType::Scalar(scalar) => {
                let raw = value.clone().into_int()?;
                let c_val = trunc_i64_to_scalar(
                    &self.builder,
                    raw,
                    scalar_int_type(self.context, *scalar),
                    *scalar,
                )?;
                Ok(c_val.into())
            }
            LibAbiType::Record(record) => {
                let ValueTy::Record(internal_fields) = &value.ty else {
                    return Err(CodegenError::ValueTypeMismatch {
                        expected: "record".to_string(),
                        actual: value.ty.to_string(),
                    });
                };
                let internal_struct = value.value.into_struct_value();
                let abi_struct_ty = self.abi_record_struct_type(record)?;
                let mut aggregate = abi_struct_ty.get_undef();
                for (abi_index, field) in record.fields.iter().enumerate() {
                    let Some((internal_index, (_, field_ty))) = internal_fields
                        .iter()
                        .enumerate()
                        .find(|(_, (name, _))| name == &field.name)
                    else {
                        return Err(CodegenError::MissingField {
                            field: field.name.clone(),
                        });
                    };
                    let raw_field = self
                        .builder
                        .build_extract_value(
                            internal_struct,
                            internal_index as u32,
                            &format!("{name}_field"),
                        )
                        .map_err(|e| CodegenError::Llvm(e.to_string()))?;
                    let field_value = CompiledValue {
                        ty: field_ty.clone(),
                        value: raw_field,
                    };
                    let abi_field =
                        self.internal_to_abi_value(&field_value, &field.ty, &field.name)?;
                    aggregate = self
                        .builder
                        .build_insert_value(
                            aggregate,
                            abi_field,
                            abi_index as u32,
                            "abi_record_out",
                        )
                        .map_err(|e| CodegenError::Llvm(e.to_string()))?
                        .into_struct_value();
                }
                Ok(aggregate.into())
            }
            LibAbiType::BorrowedVector(vec_ty) => Err(CodegenError::UnsupportedValueType {
                ty: format!("{}vec", vec_ty),
            }),
        }
    }

    fn borrowed_vec_parts(
        &mut self,
        value: BasicValueEnum<'ctx>,
        name: &str,
    ) -> Result<(PointerValue<'ctx>, IntValue<'ctx>)> {
        let vec = value.into_struct_value();
        let ptr = self
            .builder
            .build_extract_value(vec, 0, &format!("{name}_data"))
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_pointer_value();
        let len = self
            .builder
            .build_extract_value(vec, 1, &format!("{name}_len"))
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_int_value();
        Ok((ptr, len))
    }

    fn borrowed_vec_value(
        &mut self,
        ptr: PointerValue<'ctx>,
        len: IntValue<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>> {
        let vec_ty = self.borrowed_vec_struct_type();
        let mut value = vec_ty.get_undef();
        value = self
            .builder
            .build_insert_value(value, ptr, 0, "borrowed_vec_data")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_struct_value();
        value = self
            .builder
            .build_insert_value(value, len, 1, "borrowed_vec_len")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_struct_value();
        Ok(value.into())
    }

    fn validate_borrowed_vec(
        &mut self,
        cur_fn: FunctionValue<'ctx>,
        ptr: PointerValue<'ctx>,
        len: IntValue<'ctx>,
        bad_arg_bb: BasicBlock<'ctx>,
        name: &str,
    ) -> Result<()> {
        let i64_t = self.context.i64_type();
        let len_nonzero = self
            .builder
            .build_int_compare(
                IntPredicate::NE,
                len,
                i64_t.const_zero(),
                &format!("{name}_len_nonzero"),
            )
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let ptr_null = self
            .builder
            .build_is_null(ptr, &format!("{name}_ptr_null"))
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let invalid = self
            .builder
            .build_and(len_nonzero, ptr_null, &format!("{name}_invalid"))
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let ok_bb = self
            .context
            .append_basic_block(cur_fn, &format!("{name}_ok"));
        self.builder
            .build_conditional_branch(invalid, bad_arg_bb, ok_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder.position_at_end(ok_bb);
        Ok(())
    }

    fn compile_host_import_trampoline(
        &mut self,
        import: &LibraryImport,
        _package_prefix: &str,
        ctx_struct_ty: inkwell::types::StructType<'ctx>,
        callbacks_struct_ty: inkwell::types::StructType<'ctx>,
        tls_global: inkwell::values::GlobalValue<'ctx>,
    ) -> Result<()> {
        let ptr_t = self.context.ptr_type(AddressSpace::default());
        let i32_t = self.context.i32_type();

        // Trampoline LLVM signature uses Tacit's internal representation:
        // scalars are normalized i64, records use internal record structs,
        // and borrowed vectors are flattened as (ptr, i64 len).
        let mut trampoline_params = Vec::new();
        for param in &import.params {
            self.internal_param_types_for_abi(param, &mut trampoline_params)?;
        }
        let trampoline_ret_ty =
            self.llvm_type(&Self::internal_value_ty_for_abi(&import.result)?)?;
        let trampoline_fn_ty = trampoline_ret_ty.fn_type(&trampoline_params, false);
        let trampoline_name = format!("{}_dispatch", import.callback);
        let trampoline_fn =
            self.module
                .add_function(&trampoline_name, trampoline_fn_ty, Some(Linkage::Private));

        let saved_block = self.builder.get_insert_block();
        let entry_bb = self.context.append_basic_block(trampoline_fn, "entry");
        let abort_bb = self.context.append_basic_block(trampoline_fn, "abort");
        let load_callbacks_bb = self
            .context
            .append_basic_block(trampoline_fn, "load_callbacks");
        let load_fn_bb = self.context.append_basic_block(trampoline_fn, "load_fn");
        let do_call_bb = self.context.append_basic_block(trampoline_fn, "do_call");
        let read_out_bb = self.context.append_basic_block(trampoline_fn, "read_out");

        self.builder.position_at_end(entry_bb);
        let ctx_ptr = self
            .builder
            .build_load(ptr_t, tls_global.as_pointer_value(), "ctx")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_pointer_value();
        let ctx_null = self
            .builder
            .build_is_null(ctx_ptr, "ctx_null")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(ctx_null, abort_bb, load_callbacks_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(abort_bb);
        let trap = self.llvm_trap();
        self.builder
            .build_call(trap, &[], "tacit_host_trap")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_unreachable()
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(load_callbacks_bb);
        let callbacks_pp = self
            .builder
            .build_struct_gep(ctx_struct_ty, ctx_ptr, 1, "callbacks_pp")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let callbacks_ptr = self
            .builder
            .build_load(ptr_t, callbacks_pp, "callbacks")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_pointer_value();
        let callbacks_null = self
            .builder
            .build_is_null(callbacks_ptr, "callbacks_null")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(callbacks_null, abort_bb, load_fn_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(load_fn_bb);
        let fn_pp = self
            .builder
            .build_struct_gep(
                callbacks_struct_ty,
                callbacks_ptr,
                import.index as u32,
                "fn_pp",
            )
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let fn_ptr = self
            .builder
            .build_load(ptr_t, fn_pp, "callback_fn")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_pointer_value();
        let fn_null = self
            .builder
            .build_is_null(fn_ptr, "callback_null")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(fn_null, abort_bb, do_call_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(do_call_bb);
        let user_pp = self
            .builder
            .build_struct_gep(ctx_struct_ty, ctx_ptr, 0, "user_pp")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let user_ptr = self
            .builder
            .build_load(ptr_t, user_pp, "user")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_pointer_value();

        let mut c_param_metas: Vec<BasicMetadataTypeEnum<'ctx>> = vec![ptr_t.into()];
        let mut c_args: Vec<BasicMetadataValueEnum<'ctx>> = vec![user_ptr.into()];
        let mut internal_index = 0u32;
        for (i, abi_ty) in import.params.iter().enumerate() {
            c_param_metas.push(self.abi_llvm_type(abi_ty)?.into());
            match abi_ty {
                LibAbiType::BorrowedVector(_) => {
                    let ptr = trampoline_fn
                        .get_nth_param(internal_index)
                        .ok_or_else(|| {
                            CodegenError::Llvm(format!("trampoline missing vec ptr arg {i}"))
                        })?
                        .into_pointer_value();
                    internal_index += 1;
                    let len = trampoline_fn
                        .get_nth_param(internal_index)
                        .ok_or_else(|| {
                            CodegenError::Llvm(format!("trampoline missing vec len arg {i}"))
                        })?
                        .into_int_value();
                    internal_index += 1;
                    c_args.push(self.borrowed_vec_value(ptr, len)?.into());
                }
                _ => {
                    let raw_arg = trampoline_fn
                        .get_nth_param(internal_index)
                        .ok_or_else(|| CodegenError::Llvm(format!("trampoline missing arg {i}")))?;
                    internal_index += 1;
                    let internal = CompiledValue {
                        ty: Self::internal_value_ty_for_abi(abi_ty)?,
                        value: raw_arg,
                    };
                    c_args.push(
                        self.internal_to_abi_value(&internal, abi_ty, "callback_arg")?
                            .into(),
                    );
                }
            }
        }

        let result_out_alloca = if import.result.is_unit() {
            None
        } else {
            let ty = self.abi_llvm_type(&import.result)?;
            c_param_metas.push(ptr_t.into());
            let saved_ip = self.builder.get_insert_block().unwrap();
            let alloca_bb = trampoline_fn.get_first_basic_block().unwrap();
            if let Some(first_inst) = alloca_bb.get_first_instruction() {
                self.builder.position_before(&first_inst);
            } else {
                self.builder.position_at_end(alloca_bb);
            }
            let alloca = self
                .builder
                .build_alloca(ty, "callback_out")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
            self.builder.position_at_end(saved_ip);
            c_args.push(alloca.into());
            Some((alloca, ty))
        };

        let c_fn_ty = i32_t.fn_type(&c_param_metas, false);
        let call_site = self
            .builder
            .build_indirect_call(c_fn_ty, fn_ptr, &c_args, "host_callback")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let status = call_site
            .try_as_basic_value()
            .basic()
            .ok_or_else(|| CodegenError::Llvm("host callback returned no value".into()))?
            .into_int_value();
        let ok_const = i32_t.const_int(TACIT_STATUS_OK, false);
        let is_ok = self
            .builder
            .build_int_compare(IntPredicate::EQ, status, ok_const, "status_ok")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        self.builder
            .build_conditional_branch(is_ok, read_out_bb, abort_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(read_out_bb);
        let result_value = match result_out_alloca {
            None => {
                let unit_ty = Self::internal_value_ty_for_abi(&LibAbiType::Unit)?;
                CompiledValue {
                    ty: unit_ty.clone(),
                    value: self
                        .llvm_type(&unit_ty)?
                        .into_struct_type()
                        .get_undef()
                        .into(),
                }
            }
            Some((alloca, ty)) => {
                let raw = self
                    .builder
                    .build_load(ty, alloca, "result_raw")
                    .map_err(|e| CodegenError::Llvm(e.to_string()))?;
                self.abi_to_internal_value(raw, &import.result, "callback_result")?
            }
        };
        self.builder
            .build_return(Some(&result_value.value))
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        if let Some(saved) = saved_block {
            self.builder.position_at_end(saved);
        }

        let abi_binding = HostImportBinding {
            value: trampoline_fn,
            params: import.params.clone(),
            result: import.result.clone(),
        };
        self.host_import_abis
            .insert(import.hash.clone(), abi_binding);

        if !import
            .params
            .iter()
            .any(|param| matches!(param, LibAbiType::BorrowedVector(_)))
        {
            let binding = FunctionBinding {
                value: trampoline_fn,
                param_tys: import
                    .params
                    .iter()
                    .map(|param| Ok(ParamTy::Value(Self::internal_value_ty_for_abi(param)?)))
                    .collect::<Result<Vec<_>>>()?,
                ret_ty: Self::internal_value_ty_for_abi(&import.result)?,
                captures: Vec::new(),
                closure_template: None,
            };
            self.host_imports.insert(import.hash.clone(), binding);
        }
        Ok(())
    }

    fn compile_library_export(
        &mut self,
        export: &LibraryExport,
        _ctx_struct_ty: inkwell::types::StructType<'ctx>,
        tls_global: inkwell::values::GlobalValue<'ctx>,
    ) -> Result<()> {
        let ptr_t = self.context.ptr_type(AddressSpace::default());
        let i32_t = self.context.i32_type();

        // Peel any leading Ann nodes; the export body may carry an annotation
        // that helps the typechecker propagate signature info but is opaque
        // to codegen.
        let mut peeled = &export.body;
        while let Node::Ann { expr, .. } = peeled {
            peeled = expr.as_ref();
        }
        let (lam_arity, body) = collect_lam_chain(peeled)
            .ok_or(CodegenError::Unsupported("export body is not a lambda"))?;
        if lam_arity != export.params.len() {
            return Err(CodegenError::FunctionArity {
                expected: export.params.len(),
                got: lam_arity,
            });
        }

        let mut wrapper_params: Vec<BasicMetadataTypeEnum<'ctx>> = vec![ptr_t.into()];
        let instance_param_offset = if export.instance_method {
            wrapper_params.push(ptr_t.into());
            1usize
        } else {
            0usize
        };
        for param in &export.params {
            wrapper_params.push(self.abi_llvm_type(param)?.into());
        }
        let has_out = !export.result.is_unit();
        if has_out {
            wrapper_params.push(ptr_t.into());
        }
        let wrapper_ty = i32_t.fn_type(&wrapper_params, false);
        let wrapper_fn =
            self.module
                .add_function(&export.symbol, wrapper_ty, Some(Linkage::External));

        let saved_block = self.builder.get_insert_block();
        let entry_bb = self.context.append_basic_block(wrapper_fn, "entry");
        let bad_arg_bb = self.context.append_basic_block(wrapper_fn, "bad_arg");
        let run_bb = self.context.append_basic_block(wrapper_fn, "run");

        self.builder.position_at_end(entry_bb);
        let ctx_in = wrapper_fn
            .get_nth_param(0)
            .ok_or_else(|| CodegenError::Llvm("wrapper missing ctx".into()))?
            .into_pointer_value();
        let ctx_null = self
            .builder
            .build_is_null(ctx_in, "ctx_null")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let invalid_arg = if export.instance_method {
            let instance_in = wrapper_fn
                .get_nth_param(1)
                .ok_or_else(|| CodegenError::Llvm("wrapper missing instance".into()))?
                .into_pointer_value();
            let instance_null = self
                .builder
                .build_is_null(instance_in, "instance_null")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
            self.builder
                .build_or(ctx_null, instance_null, "ctx_or_instance_null")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?
        } else {
            ctx_null
        };
        self.builder
            .build_conditional_branch(invalid_arg, bad_arg_bb, run_bb)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(bad_arg_bb);
        let bad_arg_const = i32_t.const_int(TACIT_STATUS_BAD_ARGUMENT, false);
        self.builder
            .build_return(Some(&bad_arg_const))
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        self.builder.position_at_end(run_bb);
        if has_out {
            let out_idx = (1 + instance_param_offset + export.params.len()) as u32;
            let out_ptr = wrapper_fn
                .get_nth_param(out_idx)
                .ok_or_else(|| CodegenError::Llvm("wrapper missing out".into()))?
                .into_pointer_value();
            let out_null = self
                .builder
                .build_is_null(out_ptr, "out_null")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
            let do_call_bb = self.context.append_basic_block(wrapper_fn, "do_call");
            self.builder
                .build_conditional_branch(out_null, bad_arg_bb, do_call_bb)
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
            self.builder.position_at_end(do_call_bb);
        }

        let mut param_bindings = Vec::with_capacity(export.params.len());
        for (i, abi_ty) in export.params.iter().enumerate() {
            let raw_param = wrapper_fn
                .get_nth_param((i + 1 + instance_param_offset) as u32)
                .ok_or_else(|| CodegenError::Llvm(format!("wrapper missing param {i}")))?;
            match abi_ty {
                LibAbiType::BorrowedVector(vec_ty) => {
                    let (ptr, len) = self.borrowed_vec_parts(raw_param, &format!("arg{i}"))?;
                    self.validate_borrowed_vec(
                        wrapper_fn,
                        ptr,
                        len,
                        bad_arg_bb,
                        &format!("arg{i}_vec"),
                    )?;
                    param_bindings.push(Binding::VecHandle {
                        ptr,
                        len,
                        ty: *vec_ty,
                    });
                }
                _ => {
                    let value =
                        self.abi_to_internal_value(raw_param, abi_ty, &format!("arg{i}"))?;
                    param_bindings.push(Binding::Value(value));
                }
            }
        }

        // Save prior TLS, install new, run, restore.
        let prior_ctx = self
            .builder
            .build_load(ptr_t, tls_global.as_pointer_value(), "prior_ctx")
            .map_err(|e| CodegenError::Llvm(e.to_string()))?
            .into_pointer_value();
        self.builder
            .build_store(tls_global.as_pointer_value(), ctx_in)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        let state_restore = if export.instance_method {
            let runtime = self.state_runtime()?;
            let instance_in = wrapper_fn
                .get_nth_param(1)
                .ok_or_else(|| CodegenError::Llvm("wrapper missing instance".into()))?
                .into_pointer_value();
            let prior_instance = self
                .builder
                .build_load(
                    ptr_t,
                    runtime.instance_tls.as_pointer_value(),
                    "prior_instance",
                )
                .map_err(|e| CodegenError::Llvm(e.to_string()))?
                .into_pointer_value();
            let prior_status = self
                .builder
                .build_load(i32_t, runtime.status_tls.as_pointer_value(), "prior_status")
                .map_err(|e| CodegenError::Llvm(e.to_string()))?
                .into_int_value();
            self.builder
                .build_store(runtime.instance_tls.as_pointer_value(), instance_in)
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
            self.builder
                .build_store(
                    runtime.status_tls.as_pointer_value(),
                    i32_t.const_int(TACIT_STATUS_OK, false),
                )
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
            Some((runtime, prior_instance, prior_status))
        } else {
            None
        };

        let mut body_env = Vec::with_capacity(param_bindings.len());
        for binding in param_bindings.into_iter().rev() {
            body_env.push(binding);
        }

        let result_value = self.compile_value_expr(body, &body_env, wrapper_fn)?;
        let expected_result = Self::internal_value_ty_for_abi(&export.result)?;
        if result_value.ty != expected_result {
            return Err(CodegenError::ValueTypeMismatch {
                expected: expected_result.to_string(),
                actual: result_value.ty.to_string(),
            });
        }

        let runtime_status = if let Some((runtime, _, _)) = &state_restore {
            Some(
                self.builder
                    .build_load(
                        i32_t,
                        runtime.status_tls.as_pointer_value(),
                        "runtime_status",
                    )
                    .map_err(|e| CodegenError::Llvm(e.to_string()))?
                    .into_int_value(),
            )
        } else {
            None
        };

        // Restore TLS regardless of return path.
        self.builder
            .build_store(tls_global.as_pointer_value(), prior_ctx)
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        if let Some((runtime, prior_instance, prior_status)) = &state_restore {
            self.builder
                .build_store(runtime.instance_tls.as_pointer_value(), *prior_instance)
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
            self.builder
                .build_store(runtime.status_tls.as_pointer_value(), *prior_status)
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        }

        if let Some(status) = runtime_status {
            let status_ok = self
                .builder
                .build_int_compare(
                    IntPredicate::EQ,
                    status,
                    i32_t.const_int(TACIT_STATUS_OK, false),
                    "runtime_status_ok",
                )
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
            let write_out_bb = self.context.append_basic_block(wrapper_fn, "write_out");
            let status_ret_bb = self.context.append_basic_block(wrapper_fn, "status_return");
            self.builder
                .build_conditional_branch(status_ok, write_out_bb, status_ret_bb)
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
            self.builder.position_at_end(status_ret_bb);
            self.builder
                .build_return(Some(&status))
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
            self.builder.position_at_end(write_out_bb);
        }

        if has_out {
            let out_idx = (1 + instance_param_offset + export.params.len()) as u32;
            let out_ptr = wrapper_fn
                .get_nth_param(out_idx)
                .ok_or_else(|| CodegenError::Llvm("wrapper missing out".into()))?
                .into_pointer_value();
            let abi_result =
                self.internal_to_abi_value(&result_value, &export.result, "export_result")?;
            self.builder
                .build_store(out_ptr, abi_result)
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
        }

        let ok_const = i32_t.const_int(TACIT_STATUS_OK, false);
        self.builder
            .build_return(Some(&ok_const))
            .map_err(|e| CodegenError::Llvm(e.to_string()))?;

        if let Some(saved) = saved_block {
            self.builder.position_at_end(saved);
        }
        Ok(())
    }
}

fn scalar_int_type<'ctx>(
    context: &'ctx Context,
    scalar: LibScalar,
) -> inkwell::types::IntType<'ctx> {
    match scalar.width_bits() {
        8 => context.i8_type(),
        16 => context.i16_type(),
        32 => context.i32_type(),
        64 => context.i64_type(),
        other => panic!("unsupported scalar width {other}"),
    }
}

fn trunc_i64_to_scalar<'ctx>(
    builder: &Builder<'ctx>,
    value: IntValue<'ctx>,
    target_ty: inkwell::types::IntType<'ctx>,
    scalar: LibScalar,
) -> Result<IntValue<'ctx>> {
    if scalar.width_bits() == 64 {
        Ok(value)
    } else {
        builder
            .build_int_truncate(value, target_ty, "abi_trunc")
            .map_err(|e| CodegenError::Llvm(e.to_string()))
    }
}

fn extend_scalar_to_i64<'ctx>(
    builder: &Builder<'ctx>,
    value: IntValue<'ctx>,
    target_ty: inkwell::types::IntType<'ctx>,
    scalar: LibScalar,
) -> Result<IntValue<'ctx>> {
    if scalar.width_bits() == 64 {
        Ok(value)
    } else if scalar.is_signed() {
        builder
            .build_int_s_extend(value, target_ty, "abi_sext")
            .map_err(|e| CodegenError::Llvm(e.to_string()))
    } else {
        builder
            .build_int_z_extend(value, target_ty, "abi_zext")
            .map_err(|e| CodegenError::Llvm(e.to_string()))
    }
}
