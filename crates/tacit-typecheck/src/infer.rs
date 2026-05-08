//! Local type and effect inference for Tacit-Lite Phase 2.
//!
//! Each expression has both a **type** (the value it computes) and an
//! **eval-effect** (the effects produced when evaluating it).  `infer`
//! returns both as `(Ty, FnEff)`.  Callers that need a concrete `EffSet`
//! call `subst.resolve_eff(&eff)` after inference is complete.
//!
//! Returning `FnEff` instead of `EffSet` is critical for effect polymorphism:
//! the effect of `f x` (where `f` has meta type) carries a `FnEff::Meta` that
//! remains unresolved until the caller is later unified with a concrete function.
//!
//! Effect rules (Stage 3):
//! - `lam body`: eval-eff = `{}`, fn-type carries `body_eff` as the call effect.
//! - `app f x`: eval-eff = eff(f) ∪ eff(x) ∪ call-eff-of(f's type).
//! - `let rhs in body`: eval-eff = eff(rhs) ∪ eff(body).
//! - `if c t e`: eval-eff = eff(c) ∪ eff(t) ∪ eff(e)  (conservative).
//! - `rec {b0..bn} in body`: n > 1 → all Fn bindings augmented with `Div`;
//!   n = 1 → no automatic Div.  Body effects propagate normally.
//! - Primitives (`@write`, `@read`, `@exit`): IO sits at the innermost
//!   application (fully-applied call).

use std::collections::{BTreeMap, BTreeSet};

use tacit_canonical::ast::Node;

use crate::error::Diagnostic;
use crate::primitives::{is_arith, is_cmp, prim_type};
use crate::ty::{join_fn_eff, unify, EffAtom, EffSet, FnEff, Subst, Ty};
use crate::type_from_node::{child_path, type_from_node};

/// Infer the type and eval-effect of a node given a variable context.
///
/// `ctx[i]` is the type of `(var i)`. Errors are appended to `diags`;
/// inference continues past errors by returning `Ty::Unknown` / `FnEff::pure_()`.
pub fn infer(
    ctx: &[Ty],
    node: &Node,
    subst: &mut Subst,
    path: &[usize],
    diags: &mut Vec<Diagnostic>,
) -> (Ty, FnEff) {
    match node {
        Node::Int { .. } => (Ty::Int, FnEff::pure_()),
        Node::Str { .. } => (Ty::Str, FnEff::pure_()),

        Node::Var { index } => {
            let i = *index as usize;
            let ty = if i < ctx.len() {
                subst.apply(&ctx[i])
            } else {
                Ty::Unknown
            };
            (ty, FnEff::pure_())
        }

        Node::Sym { name } => {
            let ty = match prim_type(name) {
                Some(t) => t,
                None => {
                    diags.push(Diagnostic::unresolved_type(path, name));
                    Ty::Unknown
                }
            };
            // The symbol itself evaluates with no effects; effects appear on call.
            (ty, FnEff::pure_())
        }

        Node::Lam { .. } => {
            let (arity, lam_body) = collect_lam_chain(node).expect("lam node has arity");
            validate_lambda_captures(ctx, lam_body, arity as u64, subst, path, diags);
            infer_lam_chain_type(ctx, lam_body, arity, subst, path, diags)
        }

        Node::App { fn_, arg } => infer_app(ctx, fn_, arg, subst, path, diags),

        Node::Let { rhs, body } => {
            let (rhs_ty, rhs_eff) = infer(ctx, rhs, subst, &child_path(path, 0), diags);
            let ctx_body = extend(ctx, rhs_ty);
            let (body_ty, body_eff) = infer(&ctx_body, body, subst, &child_path(path, 1), diags);
            (body_ty, join_fn_eff(&rhs_eff, &body_eff, subst))
        }

        Node::Rec { bindings, body } => infer_rec(ctx, bindings, body, subst, path, diags),

        Node::If { cond, then, else_ } => {
            let (cond_ty, cond_eff) = infer(ctx, cond, subst, &child_path(path, 0), diags);
            let (then_ty, then_eff) = infer(ctx, then, subst, &child_path(path, 1), diags);
            let (else_ty, else_eff) = infer(ctx, else_, subst, &child_path(path, 2), diags);

            let cond_resolved = subst.apply(&cond_ty);
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
                    push_type_mismatch(diags, &child_path(path, 2), &t, &e);
                }
            }
            let total_eff =
                join_fn_eff(&join_fn_eff(&cond_eff, &then_eff, subst), &else_eff, subst);
            (subst.apply(&then_ty), total_eff)
        }

        Node::Match { scrutinee, arms } => {
            let (scrut_ty, scrut_eff) = infer(ctx, scrutinee, subst, &child_path(path, 0), diags);
            let result_meta = subst.fresh();
            let mut arms_eff = FnEff::pure_();
            for (i, arm) in arms.iter().enumerate() {
                let arm_eff = infer_arm(
                    ctx,
                    arm,
                    &scrut_ty,
                    &result_meta,
                    subst,
                    &child_path(path, i + 1),
                    diags,
                );
                arms_eff = join_fn_eff(&arms_eff, &arm_eff, subst);
            }
            (
                subst.apply(&result_meta),
                join_fn_eff(&scrut_eff, &arms_eff, subst),
            )
        }

        Node::Arm { pattern, body } => {
            let bindings = pattern_bindings(pattern);
            let ctx_body = bindings.into_iter().fold(ctx.to_vec(), |mut c, t| {
                c.insert(0, t);
                c
            });
            let (ty, eff) = infer(&ctx_body, body, subst, &child_path(path, 1), diags);
            (ty, eff)
        }

        Node::Record { fields } => {
            let mut field_tys = BTreeMap::new();
            let mut eff = FnEff::pure_();
            for (i, (name, val)) in fields.iter().enumerate() {
                let (ty, feff) = infer(ctx, val, subst, &child_path(path, i * 2 + 1), diags);
                if field_tys.contains_key(name) {
                    diags.push(Diagnostic::duplicate_record_field(
                        &child_path(path, i * 2),
                        name,
                    ));
                }
                field_tys.insert(name.clone(), ty);
                eff = join_fn_eff(&eff, &feff, subst);
            }
            (Ty::Record(field_tys), eff)
        }

        Node::Proj { record, field } => {
            let (rec_ty, rec_eff) = infer(ctx, record, subst, &child_path(path, 0), diags);
            let resolved = subst.apply(&rec_ty);
            let field_ty = match resolved {
                Ty::Record(ref fields) => fields.get(field).cloned().unwrap_or_else(|| {
                    diags.push(Diagnostic::missing_record_field(path, field, &resolved));
                    Ty::Unknown
                }),
                Ty::Unknown => Ty::Unknown,
                other => {
                    diags.push(Diagnostic::invalid_projection(path, field, &other));
                    Ty::Unknown
                }
            };
            (field_ty, rec_eff)
        }

        Node::Ctor { name, args } => match name.as_str() {
            "True" | "False" if args.is_empty() => (Ty::Bool, FnEff::pure_()),
            _ => {
                let mut eff = FnEff::pure_();
                for (i, arg) in args.iter().enumerate() {
                    let (_, aeff) = infer(ctx, arg, subst, &child_path(path, i + 1), diags);
                    eff = join_fn_eff(&eff, &aeff, subst);
                }
                diags.push(Diagnostic::unresolved_type(
                    path,
                    &format!("constructor '{}'", name),
                ));
                (Ty::Unknown, eff)
            }
        },

        Node::Ann { expr, type_ } => {
            let declared = type_from_node(type_, &[], &[], subst, &child_path(path, 1), diags);

            if let Some((arity, lam_body)) = collect_lam_chain(expr) {
                validate_lambda_captures(
                    ctx,
                    lam_body,
                    arity as u64,
                    subst,
                    &child_path(path, 0),
                    diags,
                );
                if let Some((param_tys, ret_ty, call_eff)) = flatten_fn_type(&declared, subst) {
                    if param_tys.len() == arity {
                        let debruijn_params: Vec<Ty> = param_tys.iter().rev().cloned().collect();
                        let ctx_body = extend_many(ctx, &debruijn_params);
                        let (body_ty, body_eff) =
                            infer(&ctx_body, lam_body, subst, &child_path(path, 0), diags);

                        if !unify(&ret_ty, &body_ty, subst) {
                            let r = subst.apply(&ret_ty);
                            let b = subst.apply(&body_ty);
                            push_type_mismatch(diags, path, &r, &b);
                        }

                        let declared_set = subst.resolve_eff(&call_eff);
                        let body_set = subst.resolve_eff(&body_eff);
                        if !body_set.is_subset_of(&declared_set) {
                            diags.push(Diagnostic::effect_violation(
                                path,
                                &declared_set.to_string(),
                                &body_set.to_string(),
                            ));
                        }

                        return (subst.apply(&declared), FnEff::pure_());
                    }
                }
            }

            let (inferred, eval_eff) = infer(ctx, expr, subst, &child_path(path, 0), diags);

            // Type check: declared vs inferred.
            if !declared.is_unknown()
                && !inferred.is_unknown()
                && !unify(&declared, &inferred, subst)
            {
                let d = subst.apply(&declared);
                let i = subst.apply(&inferred);
                push_type_mismatch(diags, path, &d, &i);
            }

            // Effect check: if the annotation specifies a function type with an effect,
            // verify that the inferred function's call-effect is within the declared set.
            check_fn_effect_annotation(&declared, &inferred, subst, path, diags);

            (subst.apply(&declared), eval_eff)
        }

        Node::Module { bindings } => {
            let n = bindings.len();
            let mut ctx_mod = ctx.to_vec();
            let metas: Vec<Ty> = (0..n).map(|_| subst.fresh()).collect();
            for m in metas.iter().rev() {
                ctx_mod.insert(0, m.clone());
            }
            let mut total_eff = FnEff::pure_();
            for (i, binding) in bindings.iter().enumerate() {
                if !matches!(binding, Node::Ann { .. }) {
                    diags.push(Diagnostic::module_missing_annotation(path, i));
                }
                let (ty, beff) = infer(&ctx_mod, binding, subst, &child_path(path, i), diags);
                unify(&metas[i], &ty, subst);
                total_eff = join_fn_eff(&total_eff, &beff, subst);
            }
            (Ty::Unknown, total_eff)
        }

        Node::Hole { diag_id, payload } => {
            let msg = match payload.as_ref() {
                Node::Str { value } => value.clone(),
                _ => String::new(),
            };
            diags.push(Diagnostic::hole_diagnostic(path, diag_id, &msg));
            (Ty::Unknown, FnEff::pure_())
        }

        Node::PatWild | Node::PatVar | Node::PatCtor { .. } | Node::PatInt { .. } => {
            (Ty::Unknown, FnEff::pure_())
        }

        Node::FnTy { .. }
        | Node::TyVar { .. }
        | Node::Forall { .. }
        | Node::EffSet { .. }
        | Node::EffVar { .. } => (Ty::Unknown, FnEff::pure_()),
    }
}

fn collect_lam_chain(node: &Node) -> Option<(usize, &Node)> {
    let mut arity = 0usize;
    let mut cur = node;
    while let Node::Lam { body } = cur {
        arity += 1;
        cur = body.as_ref();
    }
    (arity > 0).then_some((arity, cur))
}

fn flatten_fn_type(ty: &Ty, subst: &Subst) -> Option<(Vec<Ty>, Ty, FnEff)> {
    let mut params = Vec::new();
    let mut cur = subst.apply(ty);
    loop {
        match cur {
            Ty::Fn(arg, ret, eff) => {
                params.push(subst.apply(&arg));
                match subst.apply(&ret) {
                    Ty::Fn(_, _, _) => {
                        cur = subst.apply(&ret);
                    }
                    other => return Some((params, other, eff)),
                }
            }
            _ => return None,
        }
    }
}

fn infer_lam_chain_type(
    ctx: &[Ty],
    lam_body: &Node,
    arity: usize,
    subst: &mut Subst,
    path: &[usize],
    diags: &mut Vec<Diagnostic>,
) -> (Ty, FnEff) {
    let param_tys: Vec<Ty> = (0..arity).map(|_| subst.fresh()).collect();
    let debruijn_params: Vec<Ty> = param_tys.iter().rev().cloned().collect();
    let ctx_body = extend_many(ctx, &debruijn_params);
    let (body_ty, body_eff) = infer(
        &ctx_body,
        lam_body,
        subst,
        &lam_body_path(path, arity),
        diags,
    );

    let mut ty = body_ty;
    for (i, param_ty) in param_tys.into_iter().enumerate().rev() {
        let call_eff = if i + 1 == arity {
            body_eff.clone()
        } else {
            FnEff::pure_()
        };
        ty = Ty::Fn(Box::new(subst.apply(&param_ty)), Box::new(ty), call_eff);
    }
    (ty, FnEff::pure_())
}

fn validate_lambda_captures(
    ctx: &[Ty],
    lam_body: &Node,
    depth: u64,
    subst: &Subst,
    path: &[usize],
    diags: &mut Vec<Diagnostic>,
) {
    let mut free = BTreeSet::new();
    collect_free_outer_indices(lam_body, depth, &mut free);
    for outer_index in free {
        let Some(ty) = ctx.get(outer_index) else {
            continue;
        };
        let actual = subst.apply(ty);
        if !is_capturable_type(&actual) {
            diags.push(Diagnostic::invalid_capture(
                path,
                outer_index as u64 + depth,
                &actual,
            ));
        }
    }
}

fn is_capturable_type(ty: &Ty) -> bool {
    match ty {
        Ty::Buf | Ty::I64Vec => false,
        Ty::Record(fields) => fields.values().all(is_capturable_type),
        Ty::Fn(_, _, _) | Ty::Int | Ty::Bool | Ty::Str | Ty::Unknown | Ty::Meta(_) => true,
        Ty::App(_, _) => true,
    }
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
            collect_free_outer_indices(body, depth + count_pat_vars(pattern), out);
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
        | Node::EffVar { .. } => {}
    }
}

fn count_pat_vars(node: &Node) -> u64 {
    match node {
        Node::PatVar => 1,
        Node::PatCtor { sub_patterns, .. } => sub_patterns.iter().map(count_pat_vars).sum(),
        _ => 0,
    }
}

fn app_head_path(path: &[usize], arg_count: usize) -> Vec<usize> {
    let mut head = path.to_vec();
    head.extend(std::iter::repeat_n(0, arg_count));
    head
}

fn lam_body_path(path: &[usize], arity: usize) -> Vec<usize> {
    let mut body = path.to_vec();
    body.extend(std::iter::repeat_n(0, arity));
    body
}

fn push_type_mismatch(diags: &mut Vec<Diagnostic>, path: &[usize], expected: &Ty, actual: &Ty) {
    if matches!((expected, actual), (Ty::Record(_), Ty::Record(_))) {
        diags.push(Diagnostic::record_type_mismatch(path, expected, actual));
    } else {
        diags.push(Diagnostic::type_mismatch(path, expected, actual));
    }
}

// ── App inference ──────────────────────────────────────────────────────────────

fn infer_app(
    ctx: &[Ty],
    fn_: &Node,
    arg: &Node,
    subst: &mut Subst,
    path: &[usize],
    diags: &mut Vec<Diagnostic>,
) -> (Ty, FnEff) {
    if let Some(result) = infer_full_primitive_app(ctx, fn_, arg, subst, path, diags) {
        return result;
    }

    let (head, spine_args) = unfold_app_from_parts(fn_, arg);
    if let Some((arity, lam_body)) = collect_lam_chain(head) {
        if arity == spine_args.len() {
            let head_path = app_head_path(path, spine_args.len());
            validate_lambda_captures(ctx, lam_body, arity as u64, subst, &head_path, diags);
            let mut arg_tys = Vec::with_capacity(spine_args.len());
            let mut total_eff = FnEff::pure_();
            for (i, spine_arg) in spine_args.iter().enumerate() {
                let (arg_ty, arg_eff) =
                    infer(ctx, spine_arg, subst, &child_path(path, i + 1), diags);
                arg_tys.push(arg_ty);
                total_eff = join_fn_eff(&total_eff, &arg_eff, subst);
            }
            let debruijn_args: Vec<Ty> = arg_tys.iter().rev().cloned().collect();
            let ctx_body = extend_many(ctx, &debruijn_args);
            let (body_ty, body_eff) = infer(&ctx_body, lam_body, subst, path, diags);
            total_eff = join_fn_eff(&total_eff, &body_eff, subst);
            return (body_ty, total_eff);
        }
    }

    // Detect binary operator pattern: (app (app (sym op) e1) e2)
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

    let (fn_ty, fn_eff) = infer(ctx, fn_, subst, &child_path(path, 0), diags);
    let (arg_ty, arg_eff) = infer(ctx, arg, subst, &child_path(path, 1), diags);
    let fn_resolved = subst.apply(&fn_ty);

    let (ret_ty, call_eff) = match fn_resolved {
        Ty::Fn(param_ty, ret_ty, eff) => {
            if !unify(&param_ty, &arg_ty, subst) {
                let p = subst.apply(&param_ty);
                let a = subst.apply(&arg_ty);
                if !p.is_unknown() && !a.is_unknown() {
                    push_type_mismatch(diags, &child_path(path, 1), &p, &a);
                }
            }
            // eff is already applied (subst.apply resolves eff inside Fn).
            (subst.apply(&ret_ty), eff)
        }
        Ty::Unknown => (Ty::Unknown, FnEff::pure_()),
        Ty::Meta(_) => {
            let ret_meta = subst.fresh();
            let eff_meta = subst.fresh_eff();
            let expected = Ty::Fn(
                Box::new(arg_ty.clone()),
                Box::new(ret_meta.clone()),
                eff_meta.clone(),
            );
            unify(&fn_ty, &expected, subst);
            // Keep eff_meta as FnEff::Meta — do NOT resolve eagerly.
            // This lets the meta propagate until unified with a concrete type.
            (subst.apply(&ret_meta), eff_meta)
        }
        other => {
            diags.push(Diagnostic::apply_non_function(&child_path(path, 0), &other));
            (Ty::Unknown, FnEff::pure_())
        }
    };

    let total_eff = join_fn_eff(&join_fn_eff(&fn_eff, &arg_eff, subst), &call_eff, subst);
    (ret_ty, total_eff)
}

fn infer_full_primitive_app(
    ctx: &[Ty],
    fn_: &Node,
    arg: &Node,
    subst: &mut Subst,
    path: &[usize],
    diags: &mut Vec<Diagnostic>,
) -> Option<(Ty, FnEff)> {
    let (head, args) = unfold_app_from_parts(fn_, arg);
    let Node::Sym { name } = head else {
        return None;
    };

    match name.as_str() {
        "write" if args.len() == 3 => Some(infer_write_app(ctx, &args, subst, path, diags)),
        "read" if args.len() == 3 => Some(infer_read_app(ctx, &args, subst, path, diags)),
        "token-index-any" if args.len() == 6 => {
            Some(infer_token_index_any_app(ctx, &args, subst, path, diags))
        }
        "map" if args.len() == 4 => Some(infer_map_app(ctx, &args, subst, path, diags)),
        "fold" if args.len() == 4 => Some(infer_fold_app(ctx, &args, subst, path, diags)),
        "for-each" if args.len() == 3 => Some(infer_for_each_app(ctx, &args, subst, path, diags)),
        _ => None,
    }
}

fn unfold_app_from_parts<'a>(fn_: &'a Node, arg: &'a Node) -> (&'a Node, Vec<&'a Node>) {
    let mut args = vec![arg];
    let mut cur = fn_;
    loop {
        match cur {
            Node::App { fn_, arg } => {
                args.push(arg.as_ref());
                cur = fn_.as_ref();
            }
            head => {
                args.reverse();
                return (head, args);
            }
        }
    }
}

fn infer_write_app(
    ctx: &[Ty],
    args: &[&Node],
    subst: &mut Subst,
    path: &[usize],
    diags: &mut Vec<Diagnostic>,
) -> (Ty, FnEff) {
    let (fd_ty, fd_eff) = infer(ctx, args[0], subst, &child_path(path, 0), diags);
    let (buf_ty, buf_eff) = infer(ctx, args[1], subst, &child_path(path, 1), diags);
    let (len_ty, len_eff) = infer(ctx, args[2], subst, &child_path(path, 2), diags);
    expect_type(&child_path(path, 0), &Ty::Int, &fd_ty, subst, diags);
    expect_write_buffer_arg(&child_path(path, 1), &buf_ty, subst, diags);
    expect_type(&child_path(path, 2), &Ty::Int, &len_ty, subst, diags);
    let eval_eff = join_fn_eff(
        &join_fn_eff(&fd_eff, &buf_eff, subst),
        &join_fn_eff(&len_eff, &FnEff::from_set(EffSet::of([EffAtom::IO])), subst),
        subst,
    );
    (Ty::Int, eval_eff)
}

fn infer_read_app(
    ctx: &[Ty],
    args: &[&Node],
    subst: &mut Subst,
    path: &[usize],
    diags: &mut Vec<Diagnostic>,
) -> (Ty, FnEff) {
    let (fd_ty, fd_eff) = infer(ctx, args[0], subst, &child_path(path, 0), diags);
    let (buf_ty, buf_eff) = infer(ctx, args[1], subst, &child_path(path, 1), diags);
    let (len_ty, len_eff) = infer(ctx, args[2], subst, &child_path(path, 2), diags);
    expect_type(&child_path(path, 0), &Ty::Int, &fd_ty, subst, diags);
    expect_type(&child_path(path, 1), &Ty::Buf, &buf_ty, subst, diags);
    expect_type(&child_path(path, 2), &Ty::Int, &len_ty, subst, diags);
    let eval_eff = join_fn_eff(
        &join_fn_eff(&fd_eff, &buf_eff, subst),
        &join_fn_eff(
            &len_eff,
            &FnEff::from_set(EffSet::of([EffAtom::IO, EffAtom::Mut])),
            subst,
        ),
        subst,
    );
    (Ty::Int, eval_eff)
}

fn infer_token_index_any_app(
    ctx: &[Ty],
    args: &[&Node],
    subst: &mut Subst,
    path: &[usize],
    diags: &mut Vec<Diagnostic>,
) -> (Ty, FnEff) {
    let (text_ty, text_eff) = infer(ctx, args[0], subst, &child_path(path, 0), diags);
    let (off_ty, off_eff) = infer(ctx, args[1], subst, &child_path(path, 1), diags);
    let (len_ty, len_eff) = infer(ctx, args[2], subst, &child_path(path, 2), diags);
    let (delims_ty, delims_eff) = infer(ctx, args[3], subst, &child_path(path, 3), diags);
    let (delim_count_ty, delim_count_eff) = infer(ctx, args[4], subst, &child_path(path, 4), diags);
    let (table_ty, table_eff) = infer(ctx, args[5], subst, &child_path(path, 5), diags);

    expect_type(&child_path(path, 0), &Ty::Buf, &text_ty, subst, diags);
    expect_type(&child_path(path, 1), &Ty::Int, &off_ty, subst, diags);
    expect_type(&child_path(path, 2), &Ty::Int, &len_ty, subst, diags);
    expect_token_delims_arg(&child_path(path, 3), &delims_ty, subst, diags);
    expect_type(
        &child_path(path, 4),
        &Ty::Int,
        &delim_count_ty,
        subst,
        diags,
    );
    expect_type(&child_path(path, 5), &Ty::I64Vec, &table_ty, subst, diags);

    let mut eval_eff = text_eff;
    eval_eff = join_fn_eff(&eval_eff, &off_eff, subst);
    eval_eff = join_fn_eff(&eval_eff, &len_eff, subst);
    eval_eff = join_fn_eff(&eval_eff, &delims_eff, subst);
    eval_eff = join_fn_eff(&eval_eff, &delim_count_eff, subst);
    eval_eff = join_fn_eff(&eval_eff, &table_eff, subst);
    eval_eff = join_fn_eff(
        &eval_eff,
        &FnEff::from_set(EffSet::of([EffAtom::Mut])),
        subst,
    );
    (Ty::Int, eval_eff)
}

fn infer_map_app(
    ctx: &[Ty],
    args: &[&Node],
    subst: &mut Subst,
    path: &[usize],
    diags: &mut Vec<Diagnostic>,
) -> (Ty, FnEff) {
    let mut eval_eff =
        infer_i64_collection_arg(ctx, args[0], subst, &child_path(path, 0), "map", diags);

    let (count_ty, count_eff) = infer(ctx, args[1], subst, &child_path(path, 1), diags);
    expect_type(&child_path(path, 1), &Ty::Int, &count_ty, subst, diags);
    eval_eff = join_fn_eff(&eval_eff, &count_eff, subst);

    let (callback_ty, callback_eval_eff) = infer(ctx, args[2], subst, &child_path(path, 2), diags);
    let callback_call_eff =
        expect_unary_int_callback(&child_path(path, 2), "map", &callback_ty, subst, diags);
    eval_eff = join_fn_eff(&eval_eff, &callback_eval_eff, subst);
    eval_eff = join_fn_eff(&eval_eff, &callback_call_eff, subst);

    let out_eff = infer_i64_collection_arg(ctx, args[3], subst, &child_path(path, 3), "map", diags);
    eval_eff = join_fn_eff(&eval_eff, &out_eff, subst);
    eval_eff = join_fn_eff(
        &eval_eff,
        &FnEff::from_set(EffSet::of([EffAtom::Mut])),
        subst,
    );

    (Ty::Int, eval_eff)
}

fn infer_fold_app(
    ctx: &[Ty],
    args: &[&Node],
    subst: &mut Subst,
    path: &[usize],
    diags: &mut Vec<Diagnostic>,
) -> (Ty, FnEff) {
    let mut eval_eff =
        infer_i64_collection_arg(ctx, args[0], subst, &child_path(path, 0), "fold", diags);

    let (count_ty, count_eff) = infer(ctx, args[1], subst, &child_path(path, 1), diags);
    expect_type(&child_path(path, 1), &Ty::Int, &count_ty, subst, diags);
    eval_eff = join_fn_eff(&eval_eff, &count_eff, subst);

    let (init_ty, init_eff) = infer(ctx, args[2], subst, &child_path(path, 2), diags);
    if !unify(&Ty::Int, &init_ty, subst) {
        let actual = subst.apply(&init_ty);
        if !actual.is_unknown() {
            diags.push(Diagnostic::invalid_accumulator_shape(
                &child_path(path, 2),
                "fold",
                &Ty::Int,
                &actual,
            ));
        }
    }
    eval_eff = join_fn_eff(&eval_eff, &init_eff, subst);

    let (callback_ty, callback_eval_eff) = infer(ctx, args[3], subst, &child_path(path, 3), diags);
    let callback_call_eff =
        expect_fold_int_callback(&child_path(path, 3), &callback_ty, subst, diags);
    eval_eff = join_fn_eff(&eval_eff, &callback_eval_eff, subst);
    eval_eff = join_fn_eff(&eval_eff, &callback_call_eff, subst);

    (Ty::Int, eval_eff)
}

fn infer_for_each_app(
    ctx: &[Ty],
    args: &[&Node],
    subst: &mut Subst,
    path: &[usize],
    diags: &mut Vec<Diagnostic>,
) -> (Ty, FnEff) {
    let mut eval_eff =
        infer_i64_collection_arg(ctx, args[0], subst, &child_path(path, 0), "for-each", diags);

    let (count_ty, count_eff) = infer(ctx, args[1], subst, &child_path(path, 1), diags);
    expect_type(&child_path(path, 1), &Ty::Int, &count_ty, subst, diags);
    eval_eff = join_fn_eff(&eval_eff, &count_eff, subst);

    let (callback_ty, callback_eval_eff) = infer(ctx, args[2], subst, &child_path(path, 2), diags);
    let callback_call_eff =
        expect_unary_int_callback(&child_path(path, 2), "for-each", &callback_ty, subst, diags);
    eval_eff = join_fn_eff(&eval_eff, &callback_eval_eff, subst);
    eval_eff = join_fn_eff(&eval_eff, &callback_call_eff, subst);

    (Ty::Int, eval_eff)
}

fn infer_i64_collection_arg(
    ctx: &[Ty],
    arg: &Node,
    subst: &mut Subst,
    path: &[usize],
    combinator: &str,
    diags: &mut Vec<Diagnostic>,
) -> FnEff {
    let (ty, eff) = infer(ctx, arg, subst, path, diags);
    if !unify(&Ty::I64Vec, &ty, subst) {
        let actual = subst.apply(&ty);
        if !actual.is_unknown() {
            diags.push(Diagnostic::unsupported_collection_shape(
                path,
                combinator,
                &Ty::I64Vec,
                &actual,
            ));
        }
    }
    eff
}

fn expect_unary_int_callback(
    path: &[usize],
    combinator: &str,
    actual: &Ty,
    subst: &mut Subst,
    diags: &mut Vec<Diagnostic>,
) -> FnEff {
    let call_eff = subst.fresh_eff();
    let expected = Ty::Fn(Box::new(Ty::Int), Box::new(Ty::Int), call_eff);
    if !unify(&expected, actual, subst) {
        let e = subst.apply(&expected);
        let a = subst.apply(actual);
        if !a.is_unknown() {
            diags.push(Diagnostic::callback_type_mismatch(path, combinator, &e, &a));
        }
        return FnEff::pure_();
    }

    match subst.apply(&expected) {
        Ty::Fn(_, _, eff) => eff,
        _ => FnEff::pure_(),
    }
}

fn expect_fold_int_callback(
    path: &[usize],
    actual: &Ty,
    subst: &mut Subst,
    diags: &mut Vec<Diagnostic>,
) -> FnEff {
    let inner_eff = subst.fresh_eff();
    let expected = Ty::Fn(
        Box::new(Ty::Int),
        Box::new(Ty::Fn(Box::new(Ty::Int), Box::new(Ty::Int), inner_eff)),
        FnEff::pure_(),
    );
    if !unify(&expected, actual, subst) {
        let e = subst.apply(&expected);
        let a = subst.apply(actual);
        if !a.is_unknown() {
            diags.push(Diagnostic::invalid_accumulator_shape(path, "fold", &e, &a));
        }
        return FnEff::pure_();
    }

    match subst.apply(actual) {
        Ty::Fn(_, ret, outer_eff) => {
            let outer_set = subst.resolve_eff(&outer_eff);
            if !outer_set.is_pure() {
                diags.push(Diagnostic::callback_effect_mismatch(
                    path,
                    "fold",
                    &EffSet::empty(),
                    &outer_set,
                ));
            }
            match ret.as_ref() {
                Ty::Fn(_, _, inner_eff) => join_fn_eff(&outer_eff, inner_eff, subst),
                _ => FnEff::pure_(),
            }
        }
        _ => FnEff::pure_(),
    }
}

fn expect_type(
    path: &[usize],
    expected: &Ty,
    actual: &Ty,
    subst: &mut Subst,
    diags: &mut Vec<Diagnostic>,
) {
    if !unify(expected, actual, subst) {
        let e = subst.apply(expected);
        let a = subst.apply(actual);
        if !e.is_unknown() && !a.is_unknown() {
            push_type_mismatch(diags, path, &e, &a);
        }
    }
}

fn expect_write_buffer_arg(
    path: &[usize],
    actual: &Ty,
    subst: &mut Subst,
    diags: &mut Vec<Diagnostic>,
) {
    match subst.apply(actual) {
        Ty::Buf | Ty::Str | Ty::Unknown | Ty::Meta(_) => {}
        other => diags.push(Diagnostic::type_mismatch(path, &Ty::Buf, &other)),
    }
}

fn expect_token_delims_arg(
    path: &[usize],
    actual: &Ty,
    subst: &mut Subst,
    diags: &mut Vec<Diagnostic>,
) {
    match subst.apply(actual) {
        Ty::Buf | Ty::Str | Ty::Unknown | Ty::Meta(_) => {}
        other => diags.push(Diagnostic::type_mismatch(path, &Ty::Buf, &other)),
    }
}

/// Infer a binary operator: `(app (app (sym op) e1) e2)`.
fn infer_binary_op(
    ctx: &[Ty],
    op: &str,
    left: &Node,
    right: &Node,
    subst: &mut Subst,
    path: &[usize],
    diags: &mut Vec<Diagnostic>,
) -> (Ty, FnEff) {
    let (t1, eff1) = infer(ctx, left, subst, &child_path(path, 0), diags);
    let (t2, eff2) = infer(ctx, right, subst, &child_path(path, 1), diags);

    if !unify(&t1, &t2, subst) {
        let t1f = subst.apply(&t1);
        let t2f = subst.apply(&t2);
        if !t1f.is_unknown() && !t2f.is_unknown() {
            diags.push(Diagnostic::operator_overload_failure(path, op, &t1f, &t2f));
        }
        return (Ty::Unknown, join_fn_eff(&eff1, &eff2, subst));
    }

    // Arithmetic and comparison operators are pure; their operand eval-effects propagate.
    let result_ty = if is_cmp(op) {
        Ty::Bool
    } else {
        subst.apply(&t1)
    };
    (result_ty, join_fn_eff(&eff1, &eff2, subst))
}

// ── Rec inference ──────────────────────────────────────────────────────────────

/// Infer a `rec` node.
///
/// Multi-binding (mutual recursion): all bindings whose inferred type is `Fn`
/// are augmented with `Div` in their effect.  Single-binding (self-recursion):
/// no automatic Div (termination is assumed for self-recursive functions).
fn infer_rec(
    ctx: &[Ty],
    bindings: &[Node],
    body: &Node,
    subst: &mut Subst,
    path: &[usize],
    diags: &mut Vec<Diagnostic>,
) -> (Ty, FnEff) {
    let n = bindings.len();

    // Pass 1: infer binding types with fresh metas as placeholders.
    let metas: Vec<Ty> = (0..n).map(|_| subst.fresh()).collect();
    let ctx_pass1 = extend_many(ctx, &metas);

    let mut binding_types: Vec<Ty> = vec![Ty::Unknown; n];
    for (i, binding) in bindings.iter().enumerate() {
        let (ty, _eff) = if let Some((arity, lam_body)) = collect_lam_chain(binding) {
            // Rec members may use non-escapable outer handles through direct-call
            // hidden captures. Reification remains guarded later by closure capture checks.
            infer_lam_chain_type(
                &ctx_pass1,
                lam_body,
                arity,
                subst,
                &child_path(path, i),
                diags,
            )
        } else {
            infer(&ctx_pass1, binding, subst, &child_path(path, i), diags)
        };
        let resolved = subst.apply(&ty);
        unify(&metas[i], &resolved, subst);
        binding_types[i] = subst.apply(&metas[i]);
    }

    // If multi-binding: augment all Fn types with Div (mutual recursion → may diverge).
    let final_types: Vec<Ty> = if n > 1 {
        binding_types.iter().map(augment_with_div).collect()
    } else {
        binding_types
    };

    // Infer body with the final binding types.
    let ctx_final = extend_many(ctx, &final_types);
    let (body_ty, body_eff) = infer(&ctx_final, body, subst, &child_path(path, n), diags);
    (body_ty, body_eff)
}

/// Add `Div` to the call-effect of a `Fn` type, recursively.
/// Non-Fn types are returned unchanged.
fn augment_with_div(ty: &Ty) -> Ty {
    match ty {
        Ty::Fn(a, b, FnEff::Concrete(eff)) => {
            let new_eff = eff.join(&EffSet::of([EffAtom::Div]));
            Ty::Fn(a.clone(), b.clone(), FnEff::Concrete(new_eff))
        }
        Ty::Fn(a, b, FnEff::Meta(_)) => {
            // Effect meta → replace with Div (meta would have been unresolved).
            Ty::Fn(
                a.clone(),
                b.clone(),
                FnEff::Concrete(EffSet::of([EffAtom::Div])),
            )
        }
        other => other.clone(),
    }
}

// ── Match arm inference ────────────────────────────────────────────────────────

fn infer_arm(
    ctx: &[Ty],
    arm: &Node,
    scrut_ty: &Ty,
    result_meta: &Ty,
    subst: &mut Subst,
    path: &[usize],
    diags: &mut Vec<Diagnostic>,
) -> FnEff {
    let (pattern, body) = match arm {
        Node::Arm { pattern, body } => (pattern.as_ref(), body.as_ref()),
        _ => return FnEff::pure_(),
    };

    check_pattern(pattern, scrut_ty, subst, &child_path(path, 0), diags);

    let bindings = pattern_bindings(pattern);
    let ctx_body = extend_many(ctx, &bindings);
    let (body_ty, body_eff) = infer(&ctx_body, body, subst, &child_path(path, 1), diags);

    if !unify(result_meta, &body_ty, subst) {
        let r = subst.apply(result_meta);
        let b = subst.apply(&body_ty);
        if !r.is_unknown() && !b.is_unknown() {
            push_type_mismatch(diags, path, &r, &b);
        }
    }
    body_eff
}

fn check_pattern(
    pattern: &Node,
    scrut_ty: &Ty,
    subst: &mut Subst,
    path: &[usize],
    diags: &mut Vec<Diagnostic>,
) {
    match pattern {
        Node::PatWild | Node::PatVar => {}
        Node::PatInt { .. } => {
            let resolved = subst.apply(scrut_ty);
            if !matches!(resolved, Ty::Int | Ty::Unknown) {
                diags.push(Diagnostic::type_mismatch(path, &Ty::Int, &resolved));
            }
        }
        Node::PatCtor { name, sub_patterns } => match name.as_str() {
            "True" | "False" if sub_patterns.is_empty() => {
                let resolved = subst.apply(scrut_ty);
                if !matches!(resolved, Ty::Bool | Ty::Unknown) {
                    diags.push(Diagnostic::type_mismatch(path, &Ty::Bool, &resolved));
                }
            }
            _ => {}
        },
        _ => {}
    }
}

fn pattern_bindings(pattern: &Node) -> Vec<Ty> {
    match pattern {
        Node::PatVar => vec![Ty::Unknown],
        Node::PatCtor { sub_patterns, .. } => {
            sub_patterns.iter().flat_map(pattern_bindings).collect()
        }
        _ => vec![],
    }
}

// ── Effect annotation checking ────────────────────────────────────────────────

/// At an `ann` node, if the declared type is a `Fn`, verify the inferred
/// function's call-effect is ⊆ the declared call-effect.
fn check_fn_effect_annotation(
    declared: &Ty,
    inferred: &Ty,
    subst: &mut Subst,
    path: &[usize],
    diags: &mut Vec<Diagnostic>,
) {
    let dec = subst.apply(declared);
    let inf = subst.apply(inferred);
    if let (Ty::Fn(_, _, dec_eff), Ty::Fn(_, _, inf_eff)) = (&dec, &inf) {
        let dec_set = subst.resolve_eff(dec_eff);
        let inf_set = subst.resolve_eff(inf_eff);
        if !inf_set.is_subset_of(&dec_set) {
            diags.push(Diagnostic::effect_violation(
                path,
                &dec_set.to_string(),
                &inf_set.to_string(),
            ));
        }
        // Recurse into return types in case they're also Fn.
        if let (Ty::Fn(_, dec_ret, _), Ty::Fn(_, inf_ret, _)) = (&dec, &inf) {
            check_fn_effect_annotation(dec_ret, inf_ret, subst, path, diags);
        }
    }
}

// ── Context helpers ────────────────────────────────────────────────────────────

fn extend(ctx: &[Ty], ty: Ty) -> Vec<Ty> {
    let mut new = vec![ty];
    new.extend_from_slice(ctx);
    new
}

fn extend_many(ctx: &[Ty], tys: &[Ty]) -> Vec<Ty> {
    let mut new = tys.to_vec();
    new.extend_from_slice(ctx);
    new
}

// ── Tests ────────────────────────────────────────────────────────────────────

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

    fn run(node: &Node) -> (Ty, EffSet, Vec<Diagnostic>) {
        let mut subst = Subst::default();
        let mut diags = Vec::new();
        let (ty, eff_fn) = infer(&[], node, &mut subst, &[], &mut diags);
        let ty = subst.apply(&ty);
        let eff = subst.resolve_eff(&eff_fn);
        (ty, eff, diags)
    }

    // ── Smoke-corpus type/effect checks ────────────────────────────────────────

    #[test]
    fn return_zero() {
        let (ty, eff, diags) = run(&int_node(0));
        assert_eq!(ty, Ty::Int);
        assert_eq!(eff, EffSet::empty());
        assert!(diags.is_empty());
    }

    #[test]
    fn return_computed() {
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
        let (ty, eff, diags) = run(&ast);
        assert_eq!(ty, Ty::Int);
        assert_eq!(eff, EffSet::empty());
        assert!(diags.is_empty());
    }

    #[test]
    fn hello_has_io_effect() {
        // let _ = @write 1 "hello" 13 in 0
        let ast = let_(
            app(
                app(app(sym("write"), int_node(1)), str_node("hello")),
                int_node(13),
            ),
            int_node(0),
        );
        let (ty, eff, diags) = run(&ast);
        assert_eq!(ty, Ty::Int);
        assert_eq!(eff, EffSet::of([EffAtom::IO]));
        assert!(diags.is_empty());
    }

    #[test]
    fn if_branch_is_pure() {
        // let x = 5 in if @gt x 3 then 1 else 0
        let ast = let_(
            int_node(5),
            if_(
                app(app(sym("gt"), var(0)), int_node(3)),
                int_node(1),
                int_node(0),
            ),
        );
        let (ty, eff, diags) = run(&ast);
        assert_eq!(ty, Ty::Int);
        assert_eq!(eff, EffSet::empty());
        assert!(diags.is_empty());
    }

    #[test]
    fn factorial_is_pure() {
        // rec {fact = lam n. if n then @mul n (fact (@sub n 1)) else 1} in fact 5
        let lam_body = if_(
            var(0),
            app(
                app(sym("mul"), var(0)),
                app(var(1), app(app(sym("sub"), var(0)), int_node(1))),
            ),
            int_node(1),
        );
        let ast = rec1(lam(lam_body), app(var(0), int_node(5)));
        let (ty, eff, diags) = run(&ast);
        assert_eq!(ty, Ty::Int);
        assert_eq!(eff, EffSet::empty());
        assert!(diags.is_empty());
    }

    #[test]
    fn even_odd_has_div_effect() {
        // rec {even = lam n. if n then odd (sub n 1) else 1;
        //      odd  = lam n. if n then even (sub n 1) else 0}
        // in even 4
        // binding 0 = even, binding 1 = odd
        // inside even's lam: var0=n, var1=even, var2=odd
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
        let (ty, eff, diags) = run(&ast);
        assert_eq!(ty, Ty::Int);
        assert!(
            eff.atoms.contains(&EffAtom::Div),
            "expected Div in {:?}",
            eff
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn exit_nonzero_has_io_effect() {
        let ast = app(sym("exit"), int_node(7));
        let (ty, eff, diags) = run(&ast);
        assert_eq!(ty, Ty::Int);
        assert_eq!(eff, EffSet::of([EffAtom::IO]));
        assert!(diags.is_empty());
    }

    // ── Effect-violation detection ─────────────────────────────────────────────

    #[test]
    fn effect_violation_on_pure_annotation_with_io_body() {
        // ann (lam n. let _ = @write 1 "x" 1 in n)
        //     (fn-ty Int Int (eff-set))   ← declared pure
        let fn_ty_node = Node::FnTy {
            arg: Box::new(Node::Sym { name: "Int".into() }),
            ret: Box::new(Node::Sym { name: "Int".into() }),
            eff: Box::new(Node::EffSet { atoms: vec![] }), // pure
        };
        let write_call = app(
            app(app(sym("write"), int_node(1)), str_node("x")),
            int_node(1),
        );
        let body = let_(write_call, var(1)); // let _ = write(...) in n
        let annotated = Node::Ann {
            expr: Box::new(lam(body)),
            type_: Box::new(fn_ty_node),
        };
        let (_, _, diags) = run(&annotated);
        assert!(
            diags.iter().any(|d| d.kind == "effect-violation"),
            "expected effect-violation, got: {:?}",
            diags.iter().map(|d| &d.kind).collect::<Vec<_>>()
        );
    }

    // ── Effect polymorphism ────────────────────────────────────────────────────

    #[test]
    fn effect_poly_io_propagation() {
        // apply :: ∀(a, e). (a → a / e) → a → a / e
        // Body: lam f. lam x. f x
        // Call: apply write_fn 42  where write_fn = lam n. let _ = @write 1 "x" 1 in n
        // Expected result type: Int, expected eval-effect: {IO}

        // apply :: ∀(a, e). (a → a / e) → (a → a / e) / {}
        //
        // In the curried representation, the OUTER arrow is pure (applying
        // `apply` to its callback just creates a closure; no effect yet).
        // The INNER arrow carries the callback's effect `e` (calling the
        // returned function invokes the callback, which may be effectful).
        //
        // Canonical: forall 1 1 (fn-ty (fn-ty ty-var(0) ty-var(0) eff-var(0))
        //                               (fn-ty ty-var(0) ty-var(0) eff-var(0))
        //                               (eff-set))   ← outer is pure
        let sig = Node::Forall {
            ty_count: 1,
            eff_count: 1,
            body: Box::new(Node::FnTy {
                arg: Box::new(Node::FnTy {
                    arg: Box::new(Node::TyVar { index: 0 }),
                    ret: Box::new(Node::TyVar { index: 0 }),
                    eff: Box::new(Node::EffVar { index: 0 }),
                }),
                ret: Box::new(Node::FnTy {
                    arg: Box::new(Node::TyVar { index: 0 }),
                    ret: Box::new(Node::TyVar { index: 0 }),
                    eff: Box::new(Node::EffVar { index: 0 }),
                }),
                eff: Box::new(Node::EffSet { atoms: vec![] }), // outer: pure
            }),
        };

        // lam f. lam x. f x   — var 1 = f, var 0 = x inside inner lam
        let apply_body = lam(lam(app(var(1), var(0))));
        let apply = Node::Ann {
            expr: Box::new(apply_body),
            type_: Box::new(sig),
        };

        // write_fn = lam n. let _ = @write 1 "x" 1 in n
        // Inside lam: var 0 = n
        // After let: var 0 = _ (result of write), var 1 = n
        let write_call = app(
            app(app(sym("write"), int_node(1)), str_node("x")),
            int_node(1),
        );
        let write_fn = lam(let_(write_call, var(1)));

        // apply write_fn 42
        let test = app(app(apply, write_fn), int_node(42));
        let (ty, eff, diags) = run(&test);
        assert!(
            diags.iter().all(|d| d.severity != "error"),
            "unexpected errors: {:?}",
            diags
                .iter()
                .filter(|d| d.severity == "error")
                .map(|d| &d.message)
                .collect::<Vec<_>>()
        );
        assert!(
            matches!(ty, Ty::Int | Ty::Unknown | Ty::Meta(_)),
            "expected Int-compatible type, got {:?}",
            ty
        );
        assert!(
            eff.atoms.contains(&EffAtom::IO),
            "expected IO in effect, got {:?}",
            eff
        );
    }
}
