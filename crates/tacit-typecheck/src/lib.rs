//! Phase 2 structural type and effect checker for Tacit-Lite.
//!
//! Public surface:
//! - `infer_module`: run type+effect inference on an AST node.
//! - `check_unit`: validate a canonical `unit` artifact and resolve hash refs.
//! - `check_against_tacd`: check a program against `.tacd` sidecar hints.
//! - `Diagnostic` / `DiagOutput`: structured error format per ADR 0041.
//!
//! Stage 3 adds full effect inference (IO, Alloc, Mut, Div) and effect
//! polymorphism via `eff-var` instantiation (ADR 0035, 0036).

pub mod error;
pub mod infer;
pub mod primitives;
pub mod sidecar;
pub mod ty;
pub mod type_from_node;
pub mod units;

pub use error::{DiagOutput, Diagnostic};
pub use infer::infer;
pub use sidecar::check_against_tacd;
pub use ty::{EffSet, Ty};
pub use units::{
    check_unit, check_units_in_memory, CheckedUnit, DefinitionEnv, DefinitionVisibility,
    ProvidedDefinition,
};

use tacit_canonical::ast::Node;
use ty::Subst;

/// Result of type-checking a module binding group, logical unit, or expression.
#[derive(Debug)]
pub struct TypedModule {
    /// The inferred type of the top-level expression or module binding group.
    pub ty: Ty,
    /// The eval-effect of the top-level expression (Stage 3).
    pub effects: EffSet,
    /// Binding types in DeBruijn order for `rec` and `module` binding-group nodes.
    /// Logical `unit` definition types are exposed by `check_unit`.
    pub binding_types: Vec<Ty>,
}

/// Run type and effect inference on a top-level AST node.
///
/// Returns `Ok(TypedModule)` if inference succeeds without type or effect errors,
/// or `Err(Vec<Diagnostic>)` if any errors are found.
pub fn infer_module(node: &Node) -> Result<TypedModule, Vec<Diagnostic>> {
    if matches!(node, Node::Unit { .. }) {
        units::check_unit(node, &DefinitionEnv::new())?;
        return Ok(TypedModule {
            ty: Ty::Unknown,
            effects: EffSet::empty(),
            binding_types: Vec::new(),
        });
    }

    let mut subst = Subst::default();
    let mut diags: Vec<Diagnostic> = Vec::new();

    let (ty, effects_fn) = infer(&[], node, &mut subst, &[], &mut diags);
    let ty = subst.apply(&ty);
    let effects = subst.resolve_eff(&effects_fn);

    let binding_types = match node {
        Node::Rec { bindings, .. } => infer_rec_bindings(bindings, &mut subst),
        Node::Module { bindings } => infer_module_bindings(bindings, &mut subst),
        _ => vec![],
    };

    let errors: Vec<Diagnostic> = diags
        .into_iter()
        .filter(|d| d.severity == "error")
        .collect();

    if errors.is_empty() {
        Ok(TypedModule {
            ty,
            effects,
            binding_types,
        })
    } else {
        Err(errors)
    }
}

fn infer_rec_bindings(bindings: &[Node], subst: &mut Subst) -> Vec<Ty> {
    let n = bindings.len();
    let metas: Vec<Ty> = (0..n).map(|_| subst.fresh()).collect();
    let ctx: Vec<Ty> = metas.clone();
    let mut dummy_diags = Vec::new();
    for (i, binding) in bindings.iter().enumerate() {
        let (_, _) = infer(&ctx, binding, subst, &[i], &mut dummy_diags);
    }
    metas.iter().map(|m| subst.apply(m)).collect()
}

fn infer_module_bindings(bindings: &[Node], subst: &mut Subst) -> Vec<Ty> {
    let n = bindings.len();
    let metas: Vec<Ty> = (0..n).map(|_| subst.fresh()).collect();
    let ctx: Vec<Ty> = metas.clone();
    let mut dummy_diags = Vec::new();
    for (i, binding) in bindings.iter().enumerate() {
        let (_, _) = infer(&ctx, binding, subst, &[i], &mut dummy_diags);
    }
    metas.iter().map(|m| subst.apply(m)).collect()
}
