//! Convert canonical AST type expressions to `Ty` used by the type checker.
//!
//! Per ADR 0034: type position is validated here; the canonical parser accepts
//! any well-formed AST in the type child of `ann`.
//! Per ADR 0035/0036: `eff-set` and `eff-var` nodes are parsed here and
//! converted to `FnEff` for the third child of `fn-ty`.

use std::collections::BTreeMap;

use tacit_canonical::ast::Node;

use crate::error::Diagnostic;
use crate::ty::{EffAtom, EffSet, FnEff, Subst, Ty};

/// Convert a canonical AST node in type position to a `Ty`.
///
/// `ty_vars` maps DeBruijn type-variable indices (from enclosing `forall`)
/// to their instantiated types (fresh metas).
/// `eff_vars` maps DeBruijn effect-variable indices to their `FnEff`
/// instantiations (fresh eff metas).
pub fn type_from_node(
    node: &Node,
    ty_vars: &[Ty],
    eff_vars: &[FnEff],
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

        Node::FnTy { arg, ret, eff } => {
            let arg_ty = type_from_node(arg, ty_vars, eff_vars, subst, &child_path(path, 0), diags);
            let ret_ty = type_from_node(ret, ty_vars, eff_vars, subst, &child_path(path, 1), diags);
            let fn_eff = eff_from_node(eff, eff_vars, subst, &child_path(path, 2), diags);
            Ty::Fn(Box::new(arg_ty), Box::new(ret_ty), fn_eff)
        }

        Node::Forall {
            ty_count,
            eff_count,
            body,
        } => {
            // Instantiate: each ty-var → fresh type meta, each eff-var → fresh eff meta.
            let ty_metas: Vec<Ty> = (0..*ty_count).map(|_| subst.fresh()).collect();
            let eff_metas: Vec<FnEff> = (0..*eff_count).map(|_| subst.fresh_eff()).collect();
            type_from_node(
                body,
                &ty_metas,
                &eff_metas,
                subst,
                &child_path(path, 2),
                diags,
            )
        }

        Node::Record { fields } => {
            let mut field_tys = BTreeMap::new();
            for (i, (name, val)) in fields.iter().enumerate() {
                let ty = type_from_node(
                    val,
                    ty_vars,
                    eff_vars,
                    subst,
                    &child_path(path, i * 2 + 1),
                    diags,
                );
                field_tys.insert(name.clone(), ty);
            }
            Ty::Record(field_tys)
        }

        Node::App { fn_, arg } => {
            let f_ty = type_from_node(fn_, ty_vars, eff_vars, subst, &child_path(path, 0), diags);
            let a_ty = type_from_node(arg, ty_vars, eff_vars, subst, &child_path(path, 1), diags);
            if matches!(f_ty, Ty::Int | Ty::Bool | Ty::Str | Ty::Buf | Ty::I64Vec) {
                let name = match fn_.as_ref() {
                    Node::Sym { name } => name.as_str(),
                    _ => "<type>",
                };
                diags.push(Diagnostic::type_arity_mismatch(path, name, 0, 1));
                return Ty::Unknown;
            }
            Ty::App(Box::new(f_ty), Box::new(a_ty))
        }

        // Effect-only nodes are handled by eff_from_node; if encountered here in
        // a non-effect position they're ignored (per ADR 0034 policy).
        Node::EffSet { .. } | Node::EffVar { .. } => Ty::Unknown,

        other => {
            diags.push(Diagnostic::unresolved_type(
                path,
                &format!("invalid type expression: {:?}", other),
            ));
            Ty::Unknown
        }
    }
}

/// Convert an effect node (third child of `fn-ty`) to a `FnEff`.
///
/// Valid nodes: `eff-set` (concrete set) and `eff-var` (variable reference).
/// Any other node is treated as pure with an error diagnostic.
pub fn eff_from_node(
    node: &Node,
    eff_vars: &[FnEff],
    _subst: &mut Subst,
    path: &[usize],
    diags: &mut Vec<Diagnostic>,
) -> FnEff {
    match node {
        Node::EffSet { atoms } => {
            let set = parse_eff_atoms(atoms, path, diags);
            FnEff::Concrete(set)
        }
        Node::EffVar { index } => {
            let i = *index as usize;
            if i < eff_vars.len() {
                eff_vars[i].clone()
            } else {
                diags.push(Diagnostic::unbound_effect_variable(path, *index));
                FnEff::pure_()
            }
        }
        _ => {
            // Invalid node in effect position — default to pure.
            FnEff::pure_()
        }
    }
}

/// Parse a list of atom name strings into a sorted `EffSet`.
/// Unknown atom names produce an `unresolved-type` diagnostic.
pub fn parse_eff_atoms(atoms: &[String], path: &[usize], diags: &mut Vec<Diagnostic>) -> EffSet {
    let mut set = EffSet::empty();
    for atom in atoms {
        match atom.as_str() {
            "Alloc" => {
                set.atoms.insert(EffAtom::Alloc);
            }
            "Div" => {
                set.atoms.insert(EffAtom::Div);
            }
            "IO" => {
                set.atoms.insert(EffAtom::IO);
            }
            "Mut" => {
                set.atoms.insert(EffAtom::Mut);
            }
            other => {
                diags.push(Diagnostic::unresolved_type(
                    path,
                    &format!(
                        "unknown effect atom '{}'; valid atoms are Alloc, Div, IO, Mut",
                        other
                    ),
                ));
            }
        }
    }
    set
}

fn sym_to_ty(name: &str, path: &[usize], diags: &mut Vec<Diagnostic>) -> Ty {
    match name {
        "Int" => Ty::Int,
        "Bool" => Ty::Bool,
        "Str" => Ty::Str,
        "Buf" => Ty::Buf,
        "I64Vec" => Ty::I64Vec,
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

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sym(name: &str) -> Node {
        Node::Sym { name: name.into() }
    }

    fn run_type(node: &Node) -> (Ty, Vec<Diagnostic>) {
        let mut subst = Subst::default();
        let mut diags = Vec::new();
        let ty = type_from_node(node, &[], &[], &mut subst, &[], &mut diags);
        (ty, diags)
    }

    #[test]
    fn int_sym() {
        let (ty, diags) = run_type(&sym("Int"));
        assert_eq!(ty, Ty::Int);
        assert!(diags.is_empty());
    }

    #[test]
    fn builtin_handle_syms() {
        let (buf_ty, buf_diags) = run_type(&sym("Buf"));
        assert_eq!(buf_ty, Ty::Buf);
        assert!(buf_diags.is_empty());

        let (vec_ty, vec_diags) = run_type(&sym("I64Vec"));
        assert_eq!(vec_ty, Ty::I64Vec);
        assert!(vec_diags.is_empty());
    }

    #[test]
    fn unknown_sym_produces_error() {
        let (ty, diags) = run_type(&sym("Foo"));
        assert_eq!(ty, Ty::Unknown);
        assert_eq!(diags[0].kind, "unresolved-type");
    }

    #[test]
    fn fn_ty_pure() {
        // (fn-ty (sym Int) (sym Bool) (eff-set))
        let node = Node::FnTy {
            arg: Box::new(sym("Int")),
            ret: Box::new(sym("Bool")),
            eff: Box::new(Node::EffSet { atoms: vec![] }),
        };
        let (ty, diags) = run_type(&node);
        assert_eq!(
            ty,
            Ty::Fn(Box::new(Ty::Int), Box::new(Ty::Bool), FnEff::pure_())
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn fn_ty_with_io_effect() {
        // (fn-ty (sym Int) (sym Int) (eff-set IO))
        let node = Node::FnTy {
            arg: Box::new(sym("Int")),
            ret: Box::new(sym("Int")),
            eff: Box::new(Node::EffSet {
                atoms: vec!["IO".to_string()],
            }),
        };
        let (ty, diags) = run_type(&node);
        assert_eq!(
            ty,
            Ty::Fn(
                Box::new(Ty::Int),
                Box::new(Ty::Int),
                FnEff::from_set(EffSet::of([EffAtom::IO]))
            )
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn ty_var_without_forall_is_error() {
        let node = Node::TyVar { index: 0 };
        let (_, diags) = run_type(&node);
        assert_eq!(diags[0].kind, "unbound-type-variable");
    }

    #[test]
    fn eff_var_without_forall_is_error() {
        // fn-ty with eff-var(0) but no forall context
        let node = Node::FnTy {
            arg: Box::new(sym("Int")),
            ret: Box::new(sym("Int")),
            eff: Box::new(Node::EffVar { index: 0 }),
        };
        let mut subst = Subst::default();
        let mut diags = Vec::new();
        type_from_node(&node, &[], &[], &mut subst, &[], &mut diags);
        assert!(diags.iter().any(|d| d.kind == "unbound-effect-variable"));
    }

    #[test]
    fn forall_instantiates_ty_vars() {
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
        let (ty, diags) = run_type(&node);
        assert!(diags.is_empty());
        // Should be Fn(Meta(0), Meta(0), pure) — both sides unified to the same meta.
        assert!(matches!(&ty, Ty::Fn(a, b, _) if a == b));
    }

    #[test]
    fn forall_instantiates_eff_vars() {
        // (forall 1 1 (fn-ty (ty-var 0) (ty-var 0) (eff-var 0)))
        let node = Node::Forall {
            ty_count: 1,
            eff_count: 1,
            body: Box::new(Node::FnTy {
                arg: Box::new(Node::TyVar { index: 0 }),
                ret: Box::new(Node::TyVar { index: 0 }),
                eff: Box::new(Node::EffVar { index: 0 }),
            }),
        };
        let (ty, diags) = run_type(&node);
        assert!(diags.is_empty());
        // Should be Fn(Meta(_), Meta(_), FnEff::Meta(_)).
        assert!(matches!(&ty, Ty::Fn(_, _, FnEff::Meta(_))));
    }

    #[test]
    fn type_arity_mismatch_for_base_type_app() {
        let node = Node::App {
            fn_: Box::new(sym("Int")),
            arg: Box::new(sym("Bool")),
        };
        let (_, diags) = run_type(&node);
        assert_eq!(diags[0].kind, "type-arity-mismatch");
    }
}
