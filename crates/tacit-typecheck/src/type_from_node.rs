//! Convert canonical AST type expressions (second child of `ann`, inside `fn-ty`, etc.)
//! to the `Ty` used by the type checker.
//!
//! Per ADR 0034: type position is validated here; the canonical parser accepts
//! any well-formed AST in the type child of `ann`.

use std::collections::BTreeMap;

use tacit_canonical::ast::Node;

use crate::error::Diagnostic;
use crate::ty::{Subst, Ty};

/// Convert a canonical AST node in type position to a `Ty`.
///
/// `ty_vars` maps DeBruijn type-variable indices to fresh metavariables
/// (populated by the caller when inside a `forall`).
/// `path` is the AST path to the current node (for error location).
pub fn type_from_node(
    node: &Node,
    ty_vars: &[Ty],
    subst: &mut Subst,
    path: &[usize],
    diags: &mut Vec<Diagnostic>,
) -> Ty {
    match node {
        Node::Sym { name } => sym_to_ty(name, path, diags),

        Node::TyVar { index } => {
            let i = *index as usize;
            if i < ty_vars.len() {
                ty_vars[i].clone()
            } else {
                diags.push(Diagnostic::unbound_type_variable(path, *index));
                Ty::Unknown
            }
        }

        Node::FnTy { arg, ret, eff: _ } => {
            // Stage 2 ignores the effect annotation; Stage 3 will consume it.
            let arg_ty = type_from_node(arg, ty_vars, subst, &child_path(path, 0), diags);
            let ret_ty = type_from_node(ret, ty_vars, subst, &child_path(path, 1), diags);
            Ty::Fn(Box::new(arg_ty), Box::new(ret_ty))
        }

        Node::Forall {
            ty_count,
            eff_count: _,
            body,
        } => {
            // Instantiate: replace each ty-var with a fresh metavariable.
            let metas: Vec<Ty> = (0..*ty_count).map(|_| subst.fresh()).collect();
            type_from_node(body, &metas, subst, &child_path(path, 2), diags)
        }

        Node::Record { fields } => {
            let mut field_tys = BTreeMap::new();
            for (i, (name, val)) in fields.iter().enumerate() {
                let ty = type_from_node(val, ty_vars, subst, &child_path(path, i * 2 + 1), diags);
                field_tys.insert(name.clone(), ty);
            }
            Ty::Record(field_tys)
        }

        Node::App { fn_, arg } => {
            let f_ty = type_from_node(fn_, ty_vars, subst, &child_path(path, 0), diags);
            let a_ty = type_from_node(arg, ty_vars, subst, &child_path(path, 1), diags);
            // In Phase 2 there are no user-defined generic type constructors beyond
            // the builtins (Int, Bool, Str have arity 0). Any app with a known base
            // type in function position is a type-arity-mismatch.
            if matches!(f_ty, Ty::Int | Ty::Bool | Ty::Str) {
                let name = match fn_.as_ref() {
                    Node::Sym { name } => name.as_str(),
                    _ => "<type>",
                };
                diags.push(Diagnostic::type_arity_mismatch(path, name, 0, 1));
                return Ty::Unknown;
            }
            Ty::App(Box::new(f_ty), Box::new(a_ty))
        }

        // Effect-only nodes: valid in the eff position of fn-ty, ignored here.
        Node::EffSet { .. } | Node::EffVar { .. } => Ty::Unknown,

        // Anything else in type position is invalid (per ADR 0034: parse accepts,
        // typecheck rejects with a structured diagnostic).
        other => {
            diags.push(Diagnostic::unresolved_type(
                path,
                &format!("invalid type expression: {:?}", other),
            ));
            Ty::Unknown
        }
    }
}

fn sym_to_ty(name: &str, path: &[usize], diags: &mut Vec<Diagnostic>) -> Ty {
    match name {
        "Int" => Ty::Int,
        "Bool" => Ty::Bool,
        "Str" => Ty::Str,
        other => {
            diags.push(Diagnostic::unresolved_type(path, other));
            Ty::Unknown
        }
    }
}

pub fn child_path(path: &[usize], i: usize) -> Vec<usize> {
    let mut p = path.to_vec();
    p.push(i);
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sym(name: &str) -> Node {
        Node::Sym { name: name.into() }
    }

    #[test]
    fn int_sym() {
        let mut subst = Subst::default();
        let mut diags = Vec::new();
        let ty = type_from_node(&sym("Int"), &[], &mut subst, &[], &mut diags);
        assert_eq!(ty, Ty::Int);
        assert!(diags.is_empty());
    }

    #[test]
    fn unknown_sym_produces_error() {
        let mut subst = Subst::default();
        let mut diags = Vec::new();
        let ty = type_from_node(&sym("Foo"), &[], &mut subst, &[], &mut diags);
        assert_eq!(ty, Ty::Unknown);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].kind, "unresolved-type");
    }

    #[test]
    fn fn_ty_node() {
        // (fn-ty (sym Int) (sym Bool) (eff-set))
        let node = Node::FnTy {
            arg: Box::new(sym("Int")),
            ret: Box::new(sym("Bool")),
            eff: Box::new(Node::EffSet { atoms: vec![] }),
        };
        let mut subst = Subst::default();
        let mut diags = Vec::new();
        let ty = type_from_node(&node, &[], &mut subst, &[], &mut diags);
        assert_eq!(ty, Ty::Fn(Box::new(Ty::Int), Box::new(Ty::Bool)));
        assert!(diags.is_empty());
    }

    #[test]
    fn ty_var_without_forall_is_error() {
        let node = Node::TyVar { index: 0 };
        let mut subst = Subst::default();
        let mut diags = Vec::new();
        type_from_node(&node, &[], &mut subst, &[], &mut diags);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].kind, "unbound-type-variable");
    }

    #[test]
    fn forall_instantiates_with_metas() {
        // (forall 1 0 (fn-ty (ty-var 0) (ty-var 0) (eff-set)))
        let node = Node::Forall {
            ty_count: 1,
            eff_count: 0,
            body: Box::new(Node::FnTy {
                arg: Box::new(Node::TyVar { index: 0 }),
                ret: Box::new(Node::TyVar { index: 0 }),
                eff: Box::new(Node::EffSet { atoms: vec![] }),
            }),
        };
        let mut subst = Subst::default();
        let mut diags = Vec::new();
        let ty = type_from_node(&node, &[], &mut subst, &[], &mut diags);
        assert!(diags.is_empty());
        // Should be Fn(Meta(0), Meta(0)) — both sides unified to the same meta.
        assert!(matches!(ty, Ty::Fn(a, b) if a == b));
    }

    #[test]
    fn type_arity_mismatch_for_base_type_app() {
        // (app (sym Int) (sym Bool)) — Int is not a type constructor
        let node = Node::App {
            fn_: Box::new(sym("Int")),
            arg: Box::new(sym("Bool")),
        };
        let mut subst = Subst::default();
        let mut diags = Vec::new();
        type_from_node(&node, &[], &mut subst, &[], &mut diags);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].kind, "type-arity-mismatch");
    }
}
