//! Local type inference for Tacit-Lite Phase 2.
//!
//! Performs structural type checking with local inference inside function bodies.
//! Exported definitions at module boundaries require explicit signatures (Phase 2
//! plan § Stage 2).
//!
//! Context (`ctx`) is a Vec<Ty> where `ctx[0]` is the type of `var 0` (innermost).
//! Inference is monomorphic within each expression; explicit `forall` annotations
//! are instantiated when encountered (ADR 0034).

use std::collections::BTreeMap;

use tacit_canonical::ast::Node;

use crate::error::Diagnostic;
use crate::primitives::{is_arith, is_cmp, prim_type};
use crate::type_from_node::{child_path, type_from_node};
use crate::ty::{unify, Subst, Ty};

/// Infer the type of a node given a variable context.
///
/// `ctx[i]` is the type of `(var i)`. `path` tracks the AST path from the root
/// for diagnostic location. Errors are appended to `diags`; inference continues
/// past errors by returning `Ty::Unknown`.
pub fn infer(
    ctx: &[Ty],
    node: &Node,
    subst: &mut Subst,
    path: &[usize],
    diags: &mut Vec<Diagnostic>,
) -> Ty {
    match node {
        Node::Int { .. } => Ty::Int,
        Node::Str { .. } => Ty::Str,

        Node::Var { index } => {
            let i = *index as usize;
            if i < ctx.len() {
                subst.apply(&ctx[i])
            } else {
                // AST invariant violation — should not happen for well-formed programs.
                Ty::Unknown
            }
        }

        Node::Sym { name } => {
            match prim_type(name) {
                Some(t) => t,
                None => {
                    // Unknown @name — not in the Phase 2 primitive allowlist.
                    diags.push(Diagnostic::unresolved_type(path, name));
                    Ty::Unknown
                }
            }
        }

        Node::Lam { body } => {
            let param_ty = subst.fresh();
            let ctx_body = extend(ctx, param_ty.clone());
            let body_ty = infer(&ctx_body, body, subst, &child_path(path, 0), diags);
            Ty::Fn(Box::new(subst.apply(&param_ty)), Box::new(body_ty))
        }

        Node::App { fn_, arg } => {
            infer_app(ctx, fn_, arg, subst, path, diags)
        }

        Node::Let { rhs, body } => {
            let rhs_ty = infer(ctx, rhs, subst, &child_path(path, 0), diags);
            let ctx_body = extend(ctx, rhs_ty);
            infer(&ctx_body, body, subst, &child_path(path, 1), diags)
        }

        Node::Rec { bindings, body } => {
            infer_rec(ctx, bindings, body, subst, path, diags)
        }

        Node::If { cond, then, else_ } => {
            // Infer branches first so that the condition meta is constrained by
            // any sub-expression usage before we check it (e.g. `if n then @sub n 1 ...`
            // constrains n to Int before the condition check).
            let cond_ty = infer(ctx, cond, subst, &child_path(path, 0), diags);
            let then_ty = infer(ctx, then, subst, &child_path(path, 1), diags);
            let else_ty = infer(ctx, else_, subst, &child_path(path, 2), diags);

            // Check condition type after branches (metas may now be resolved).
            let cond_resolved = subst.apply(&cond_ty);
            // Phase 2: accept Int or Bool as condition (ADR 0042); Metas and Unknown
            // are also allowed (they will be constrained by enclosing context).
            if !matches!(
                cond_resolved,
                Ty::Int | Ty::Bool | Ty::Unknown | Ty::Meta(_)
            ) {
                diags.push(Diagnostic::type_mismatch(
                    &child_path(path, 0),
                    &Ty::Bool,
                    &cond_resolved,
                ));
            }

            if !unify(&then_ty, &else_ty, subst) {
                let t = subst.apply(&then_ty);
                let e = subst.apply(&else_ty);
                if !t.is_unknown() && !e.is_unknown() {
                    diags.push(Diagnostic::type_mismatch(&child_path(path, 2), &t, &e));
                }
            }
            subst.apply(&then_ty)
        }

        Node::Match { scrutinee, arms } => {
            let scrut_ty = infer(ctx, scrutinee, subst, &child_path(path, 0), diags);
            let result_meta = subst.fresh();
            for (i, arm) in arms.iter().enumerate() {
                infer_arm(ctx, arm, &scrut_ty, &result_meta, subst,
                          &child_path(path, i + 1), diags);
            }
            subst.apply(&result_meta)
        }

        Node::Arm { pattern, body } => {
            // Arms are only valid inside Match; direct inference here for completeness.
            let bindings = pattern_bindings(pattern);
            let ctx_body = bindings.into_iter().fold(ctx.to_vec(), |mut c, t| {
                c.insert(0, t);
                c
            });
            infer(&ctx_body, body, subst, &child_path(path, 1), diags)
        }

        Node::Record { fields } => {
            let mut field_tys = BTreeMap::new();
            for (i, (name, val)) in fields.iter().enumerate() {
                let ty = infer(ctx, val, subst, &child_path(path, i * 2 + 1), diags);
                field_tys.insert(name.clone(), ty);
            }
            Ty::Record(field_tys)
        }

        Node::Proj { record, field } => {
            let rec_ty = infer(ctx, record, subst, &child_path(path, 0), diags);
            let resolved = subst.apply(&rec_ty);
            match resolved {
                Ty::Record(ref fields) => {
                    fields.get(field).cloned().unwrap_or_else(|| {
                        diags.push(Diagnostic::unresolved_type(
                            path,
                            &format!("field '{}' not found in record type", field),
                        ));
                        Ty::Unknown
                    })
                }
                Ty::Unknown => Ty::Unknown,
                other => {
                    diags.push(Diagnostic::type_mismatch(
                        path,
                        &Ty::Record(BTreeMap::new()),
                        &other,
                    ));
                    Ty::Unknown
                }
            }
        }

        Node::Ctor { name, args } => {
            match name.as_str() {
                "True" | "False" if args.is_empty() => Ty::Bool,
                _ => {
                    // Unknown constructor: infer arg types for error recovery.
                    for (i, arg) in args.iter().enumerate() {
                        infer(ctx, arg, subst, &child_path(path, i + 1), diags);
                    }
                    // No type information for unknown constructors in Phase 2.
                    diags.push(Diagnostic::unresolved_type(
                        path,
                        &format!("constructor '{}'", name),
                    ));
                    Ty::Unknown
                }
            }
        }

        Node::Ann { expr, type_ } => {
            let declared = type_from_node(type_, &[], subst, &child_path(path, 1), diags);
            let inferred = infer(ctx, expr, subst, &child_path(path, 0), diags);
            if !declared.is_unknown() && !inferred.is_unknown()
                && !unify(&declared, &inferred, subst)
            {
                let d = subst.apply(&declared);
                let i = subst.apply(&inferred);
                diags.push(Diagnostic::type_mismatch(path, &d, &i));
            }
            subst.apply(&declared)
        }

        Node::Module { bindings } => {
            // Module bindings should carry explicit signatures; warn for each
            // that lacks an Ann wrapper.
            let n = bindings.len();
            let mut ctx_mod = ctx.to_vec();
            // Insert fresh metas for all bindings (allows mutual references).
            let metas: Vec<Ty> = (0..n).map(|_| subst.fresh()).collect();
            for m in metas.iter().rev() {
                ctx_mod.insert(0, m.clone());
            }
            for (i, binding) in bindings.iter().enumerate() {
                if !matches!(binding, Node::Ann { .. }) {
                    diags.push(Diagnostic::module_missing_annotation(path, i));
                }
                let ty = infer(&ctx_mod, binding, subst, &child_path(path, i), diags);
                unify(&metas[i], &ty, subst);
            }
            // Module itself has no value type.
            Ty::Unknown
        }

        Node::Hole { diag_id, payload } => {
            let msg = match payload.as_ref() {
                Node::Str { value } => value.clone(),
                _ => String::new(),
            };
            diags.push(Diagnostic::hole_diagnostic(path, diag_id, &msg));
            Ty::Unknown
        }

        // Patterns in expression position are AST errors.
        Node::PatWild | Node::PatVar | Node::PatCtor { .. } | Node::PatInt { .. } => Ty::Unknown,

        // Type-level nodes in expression position: type checker ignores them.
        Node::FnTy { .. }
        | Node::TyVar { .. }
        | Node::Forall { .. }
        | Node::EffSet { .. }
        | Node::EffVar { .. } => Ty::Unknown,
    }
}

/// Infer a function application, handling primitive operator overload resolution.
fn infer_app(
    ctx: &[Ty],
    fn_: &Node,
    arg: &Node,
    subst: &mut Subst,
    path: &[usize],
    diags: &mut Vec<Diagnostic>,
) -> Ty {
    // Check if this is a partial application of a binary operator: (app (sym op) e)
    // or full application: (app (app (sym op) e1) e2).
    // We resolve operator overloads at the outermost application.
    if let Node::App {
        fn_: inner_fn,
        arg: left_arg,
    } = fn_
    {
        if let Node::Sym { name } = inner_fn.as_ref() {
            if is_arith(name) || is_cmp(name) {
                return infer_binary_op(ctx, name, left_arg, arg, subst, path, diags);
            }
        }
    }

    let fn_ty = infer(ctx, fn_, subst, &child_path(path, 0), diags);
    let arg_ty = infer(ctx, arg, subst, &child_path(path, 1), diags);
    let fn_resolved = subst.apply(&fn_ty);

    match fn_resolved {
        Ty::Fn(param_ty, ret_ty) => {
            if !unify(&param_ty, &arg_ty, subst) {
                let p = subst.apply(&param_ty);
                let a = subst.apply(&arg_ty);
                if !p.is_unknown() && !a.is_unknown() {
                    diags.push(Diagnostic::type_mismatch(&child_path(path, 1), &p, &a));
                }
            }
            subst.apply(&ret_ty)
        }
        Ty::Unknown => Ty::Unknown,
        Ty::Meta(_) => {
            // fn_ty is still a meta; constrain it.
            let ret_meta = subst.fresh();
            let expected = Ty::Fn(Box::new(arg_ty.clone()), Box::new(ret_meta.clone()));
            unify(&fn_ty, &expected, subst);
            subst.apply(&ret_meta)
        }
        other => {
            diags.push(Diagnostic::type_mismatch(
                &child_path(path, 0),
                &Ty::Fn(Box::new(arg_ty), Box::new(subst.fresh())),
                &other,
            ));
            Ty::Unknown
        }
    }
}

/// Infer a binary operator application `(app (app (sym op) e1) e2)`.
/// Applies the no-coercion unification rule from ADR 0042.
fn infer_binary_op(
    ctx: &[Ty],
    op: &str,
    left: &Node,
    right: &Node,
    subst: &mut Subst,
    path: &[usize],
    diags: &mut Vec<Diagnostic>,
) -> Ty {
    let t1 = infer(ctx, left, subst, &child_path(path, 0), diags);
    let t2 = infer(ctx, right, subst, &child_path(path, 1), diags);
    let _t1r = subst.apply(&t1);
    let _t2r = subst.apply(&t2);

    if !unify(&t1, &t2, subst) {
        let t1f = subst.apply(&t1);
        let t2f = subst.apply(&t2);
        if !t1f.is_unknown() && !t2f.is_unknown() {
            diags.push(Diagnostic::operator_overload_failure(path, op, &t1f, &t2f));
        }
        return Ty::Unknown;
    }

    if is_cmp(op) {
        Ty::Bool
    } else {
        subst.apply(&t1)
    }
}

/// Infer the type of a `rec` node.
fn infer_rec(
    ctx: &[Ty],
    bindings: &[Node],
    body: &Node,
    subst: &mut Subst,
    path: &[usize],
    diags: &mut Vec<Diagnostic>,
) -> Ty {
    let n = bindings.len();
    // Assign fresh metas to all bindings. ctx_rec[0] = type of binding 0 = var 0.
    let metas: Vec<Ty> = (0..n).map(|_| subst.fresh()).collect();
    let ctx_rec = extend_many(ctx, &metas);

    // Type-check each binding under the extended context.
    for (i, binding) in bindings.iter().enumerate() {
        let binding_ty = infer(&ctx_rec, binding, subst, &child_path(path, i), diags);
        if !unify(&metas[i], &binding_ty, subst) {
            let m = subst.apply(&metas[i]);
            let b = subst.apply(&binding_ty);
            if !m.is_unknown() && !b.is_unknown() {
                diags.push(Diagnostic::type_mismatch(&child_path(path, i), &m, &b));
            }
        }
    }

    // Type-check the body.
    infer(&ctx_rec, body, subst, &child_path(path, n), diags)
}

/// Infer an arm inside a `match`, unifying its body type with `result_meta`.
fn infer_arm(
    ctx: &[Ty],
    arm: &Node,
    scrut_ty: &Ty,
    result_meta: &Ty,
    subst: &mut Subst,
    path: &[usize],
    diags: &mut Vec<Diagnostic>,
) {
    let (pattern, body) = match arm {
        Node::Arm { pattern, body } => (pattern.as_ref(), body.as_ref()),
        _ => return,
    };

    // Check pattern compatibility with scrutinee type.
    check_pattern(pattern, scrut_ty, subst, &child_path(path, 0), diags);

    // Extend context with any pattern variable bindings.
    let bindings = pattern_bindings(pattern);
    let ctx_body = extend_many(ctx, &bindings);
    let body_ty = infer(&ctx_body, body, subst, &child_path(path, 1), diags);

    if !unify(result_meta, &body_ty, subst) {
        let r = subst.apply(result_meta);
        let b = subst.apply(&body_ty);
        if !r.is_unknown() && !b.is_unknown() {
            diags.push(Diagnostic::type_mismatch(path, &r, &b));
        }
    }
}

/// Check that a pattern is compatible with the scrutinee type.
fn check_pattern(
    pattern: &Node,
    scrut_ty: &Ty,
    subst: &mut Subst,
    path: &[usize],
    diags: &mut Vec<Diagnostic>,
) {
    match pattern {
        Node::PatWild | Node::PatVar => {} // always OK
        Node::PatInt { .. } => {
            // pat-int requires an Int scrutinee.
            let resolved = subst.apply(scrut_ty);
            if !matches!(resolved, Ty::Int | Ty::Unknown) {
                diags.push(Diagnostic::type_mismatch(path, &Ty::Int, &resolved));
            }
        }
        Node::PatCtor { name, sub_patterns } => {
            match name.as_str() {
                "True" | "False" if sub_patterns.is_empty() => {
                    // Bool ctor pattern requires Bool scrutinee.
                    let resolved = subst.apply(scrut_ty);
                    if !matches!(resolved, Ty::Bool | Ty::Unknown) {
                        diags.push(Diagnostic::type_mismatch(path, &Ty::Bool, &resolved));
                    }
                }
                _ => {} // Unknown ctor patterns allowed (for error recovery).
            }
        }
        _ => {}
    }
}

/// Determine the types introduced by a pattern's binders.
/// Returns types in the order they're added to context (innermost = index 0).
/// For Phase 2, all pattern variables are assigned fresh metavariables.
fn pattern_bindings(pattern: &Node) -> Vec<Ty> {
    match pattern {
        Node::PatVar => vec![Ty::Unknown], // will be refined by surrounding inference
        Node::PatCtor { sub_patterns, .. } => sub_patterns
            .iter()
            .flat_map(pattern_bindings)
            .collect(),
        _ => vec![],
    }
}

/// Extend `ctx` by prepending one type (making it `var 0`).
fn extend(ctx: &[Ty], ty: Ty) -> Vec<Ty> {
    let mut new = vec![ty];
    new.extend_from_slice(ctx);
    new
}

/// Extend `ctx` by prepending multiple types (first in slice = var 0).
fn extend_many(ctx: &[Ty], tys: &[Ty]) -> Vec<Ty> {
    let mut new = tys.to_vec();
    new.extend_from_slice(ctx);
    new
}

#[cfg(test)]
mod tests {
    use super::*;

    fn int_node(v: i64) -> Node {
        Node::Int {
            value: v.to_string(),
        }
    }

    fn str_node(s: &str) -> Node {
        Node::Str { value: s.into() }
    }

    fn sym(name: &str) -> Node {
        Node::Sym { name: name.into() }
    }

    fn app(f: Node, a: Node) -> Node {
        Node::App {
            fn_: Box::new(f),
            arg: Box::new(a),
        }
    }

    fn var(i: u64) -> Node {
        Node::Var { index: i }
    }

    fn lam(body: Node) -> Node {
        Node::Lam {
            body: Box::new(body),
        }
    }

    fn let_(rhs: Node, body: Node) -> Node {
        Node::Let {
            rhs: Box::new(rhs),
            body: Box::new(body),
        }
    }

    fn if_(c: Node, t: Node, e: Node) -> Node {
        Node::If {
            cond: Box::new(c),
            then: Box::new(t),
            else_: Box::new(e),
        }
    }

    fn rec1(binding: Node, body: Node) -> Node {
        Node::Rec {
            bindings: vec![binding],
            body: Box::new(body),
        }
    }

    fn run(node: &Node) -> (Ty, Vec<Diagnostic>) {
        let mut subst = Subst::default();
        let mut diags = Vec::new();
        let ty = infer(&[], node, &mut subst, &[], &mut diags);
        let ty = subst.apply(&ty);
        (ty, diags)
    }

    // --- return-zero ---
    #[test]
    fn return_zero() {
        let (ty, diags) = run(&int_node(0));
        assert_eq!(ty, Ty::Int);
        assert!(diags.is_empty());
    }

    // --- return-computed: let x = 5 in let y = 8 in @sub (@mul x y) 7 ---
    #[test]
    fn return_computed() {
        // let x=5 in let y=8 in @sub (@mul x y) 7
        // canonical: (let 5 (let 8 (app (app sub (app (app mul (var 1)) (var 0))) 7)))
        let ast = let_(
            int_node(5),
            let_(
                int_node(8),
                app(
                    app(sym("sub"), app(app(sym("mul"), var(1)), var(0))),
                    int_node(7),
                ),
            ),
        );
        let (ty, diags) = run(&ast);
        assert_eq!(ty, Ty::Int);
        assert!(diags.is_empty());
    }

    // --- hello: let _ = @write 1 "hello" 13 in 0 ---
    #[test]
    fn hello() {
        let ast = let_(
            app(app(app(sym("write"), int_node(1)), str_node("hello")), int_node(13)),
            int_node(0),
        );
        let (ty, diags) = run(&ast);
        assert_eq!(ty, Ty::Int);
        assert!(diags.is_empty());
    }

    // --- if-branch: let x = 5 in if @gt x 3 then 1 else 0 ---
    #[test]
    fn if_branch() {
        let ast = let_(
            int_node(5),
            if_(
                app(app(sym("gt"), var(0)), int_node(3)),
                int_node(1),
                int_node(0),
            ),
        );
        let (ty, diags) = run(&ast);
        assert_eq!(ty, Ty::Int);
        assert!(diags.is_empty());
    }

    #[test]
    fn factorial() {
        // rec binding 0 = fact lambda
        // Inside lam: var 0 = n, var 1 = fact
        // body: if (var 0) (@mul (var 0) ((var 1) (@sub (var 0) 1))) 1
        let lam_body = if_(
            var(0),
            app(
                app(sym("mul"), var(0)),
                app(var(1), app(app(sym("sub"), var(0)), int_node(1))),
            ),
            int_node(1),
        );
        let ast = rec1(lam(lam_body), app(var(0), int_node(5)));
        let (ty, diags) = run(&ast);
        assert_eq!(ty, Ty::Int);
        assert!(diags.is_empty());
    }

    // --- even-odd ---
    #[test]
    fn even_odd() {
        // rec { even=lam(if n then odd(sub n 1) else 1);
        //       odd=lam(if n then even(sub n 1) else 0) } in even 4
        // binding 0=even, binding 1=odd
        // inside even's lam: var0=n, var1=even, var2=odd
        // inside odd's lam:  var0=n, var1=even, var2=odd
        let even_body = if_(
            var(0),
            app(var(2), app(app(sym("sub"), var(0)), int_node(1))),
            int_node(1),
        );
        let odd_body = if_(
            var(0),
            app(var(1), app(app(sym("sub"), var(0)), int_node(1))),
            int_node(0),
        );
        let ast = Node::Rec {
            bindings: vec![lam(even_body), lam(odd_body)],
            body: Box::new(app(var(0), int_node(4))),
        };
        let (ty, diags) = run(&ast);
        assert_eq!(ty, Ty::Int);
        assert!(diags.is_empty());
    }

    // --- exit-nonzero: @exit 7 ---
    #[test]
    fn exit_nonzero() {
        let ast = app(sym("exit"), int_node(7));
        let (ty, diags) = run(&ast);
        assert_eq!(ty, Ty::Int);
        assert!(diags.is_empty());
    }
}
