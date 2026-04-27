//! Phase 2 structural type checker for Tacit-Lite.
//!
//! Public surface per [phase-2-plan.md § Stage 2](../../plans/phase-2-plan.md):
//! - `infer_module`: run type inference on an AST node.
//! - `check_against_sidecar`: check a program against `.tac.sidecar.toml` expectations.
//! - `Diagnostic` / `DiagOutput`: structured error format per ADR 0041.
//!
//! Stage 2 scope: structural types (Int, Bool, Str, functions, records),
//! local inference, generics via explicit `forall` annotations, operator
//! overload resolution (ADR 0042). Effects are Stage 3.

pub mod error;
pub mod infer;
pub mod primitives;
pub mod sidecar;
pub mod ty;
pub mod type_from_node;

pub use error::{DiagOutput, Diagnostic};
pub use infer::infer;
pub use sidecar::{check_against_sidecar, TypeSidecar};
pub use ty::Ty;

use tacit_canonical::ast::Node;
use ty::Subst;

/// Result of type-checking a module or top-level expression.
#[derive(Debug)]
pub struct TypedModule {
    /// The inferred type of the top-level expression or module.
    pub ty: Ty,
    /// Binding types in DeBruijn order (0 = innermost / first) for `rec`/`module` nodes.
    pub binding_types: Vec<Ty>,
}

/// Run type inference on a top-level AST node.
///
/// Returns `Ok(TypedModule)` if inference succeeds without type errors,
/// or `Err(Vec<Diagnostic>)` if any type errors are found.
/// Warnings (e.g. `module-missing-annotation`) are not returned as errors.
pub fn infer_module(node: &Node) -> Result<TypedModule, Vec<Diagnostic>> {
    let mut subst = Subst::default();
    let mut diags: Vec<Diagnostic> = Vec::new();

    let ty = infer(&[], node, &mut subst, &[], &mut diags);
    let ty = subst.apply(&ty);

    // Collect binding types if the root is a Rec or Module.
    let binding_types = match node {
        Node::Rec { bindings, .. } => {
            // Re-infer to get binding types. We re-run with a fresh subst seeded
            // from the first pass to avoid redundant errors.
            infer_rec_bindings(bindings, &mut subst)
        }
        Node::Module { bindings } => {
            infer_module_bindings(bindings, &mut subst)
        }
        _ => vec![],
    };

    let errors: Vec<Diagnostic> = diags
        .into_iter()
        .filter(|d| d.severity == "error")
        .collect();

    if errors.is_empty() {
        Ok(TypedModule { ty, binding_types })
    } else {
        Err(errors)
    }
}

fn infer_rec_bindings(bindings: &[Node], subst: &mut Subst) -> Vec<Ty> {
    let n = bindings.len();
    let metas: Vec<Ty> = (0..n).map(|_| subst.fresh()).collect();
    let ctx: Vec<Ty> = metas.clone();
    // The ctx_rec has metas for each binding at positions 0..n-1.
    // We do a second infer pass to get the resolved binding types.
    let mut dummy_diags = Vec::new();
    for (i, binding) in bindings.iter().enumerate() {
        let _ty = infer(&ctx, binding, subst, &[i], &mut dummy_diags);
    }
    metas.iter().map(|m| subst.apply(m)).collect()
}

fn infer_module_bindings(bindings: &[Node], subst: &mut Subst) -> Vec<Ty> {
    let n = bindings.len();
    let metas: Vec<Ty> = (0..n).map(|_| subst.fresh()).collect();
    let ctx: Vec<Ty> = metas.clone();
    let mut dummy_diags = Vec::new();
    for (i, binding) in bindings.iter().enumerate() {
        let _ty = infer(&ctx, binding, subst, &[i], &mut dummy_diags);
    }
    metas.iter().map(|m| subst.apply(m)).collect()
}
