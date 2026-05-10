//! Pure-AST pre-codegen analysis. No LLVM dependency.
//!
//! These functions classify and validate the AST shape that codegen will
//! consume. Splitting them out from `compile.rs` lets the analysis layer
//! be tested without an installed LLVM and keeps the LLVM-version pin
//! confined to the IR-emission path (see ADR 0031).

use tacit_canonical::ast::Node;

use crate::error::CodegenError;

/// Walk an `App` left-spine: `App(App(App(head, a₀), a₁), a₂)` →
/// `(head, [a₀, a₁, a₂])`. The head is whatever non-`App` node sits at
/// the leftmost position; arguments are returned in source order.
pub fn unfold_app(node: &Node) -> (&Node, Vec<&Node>) {
    let mut args: Vec<&Node> = Vec::new();
    let mut cur = node;
    loop {
        match cur {
            Node::App { fn_, arg } => {
                args.push(arg.as_ref());
                cur = fn_.as_ref();
            }
            other => {
                args.reverse();
                return (other, args);
            }
        }
    }
}

/// Walk a consecutive `Lam` chain:
/// `Lam(Lam(body))` -> `(2, body)`.
///
/// The returned arity is the number of parameters in source order. The body is
/// the first non-`Lam` node after the parameter binders.
pub fn collect_lam_chain(node: &Node) -> Option<(usize, &Node)> {
    let mut arity = 0usize;
    let mut cur = node;
    while let Node::Lam { body } = cur {
        arity += 1;
        cur = body.as_ref();
    }
    (arity > 0).then_some((arity, cur))
}

/// Verify the AST contains no `Hole` nodes. ADR 0023 / Phase 1 hard-fails
/// on holes; this gate runs once at codegen entry.
pub fn check_no_holes(node: &Node) -> Result<(), CodegenError> {
    match node {
        Node::Hole { diag_id, .. } => Err(CodegenError::Hole {
            diag_id: diag_id.clone(),
        }),
        Node::Lam { body } | Node::Let { rhs: _, body } | Node::Ann { expr: body, .. } => {
            check_no_holes(body)
        }
        Node::App { fn_, arg } => {
            check_no_holes(fn_)?;
            check_no_holes(arg)
        }
        Node::Rec { bindings, body } => {
            for b in bindings {
                check_no_holes(b)?;
            }
            check_no_holes(body)
        }
        Node::Module { bindings } => {
            for b in bindings {
                check_no_holes(b)?;
            }
            Ok(())
        }
        Node::Unit { defs, .. } | Node::Defs { defs } => {
            for def in defs {
                check_no_holes(def)?;
            }
            Ok(())
        }
        Node::Def { body, .. } => check_no_holes(body),
        Node::If { cond, then, else_ } => {
            check_no_holes(cond)?;
            check_no_holes(then)?;
            check_no_holes(else_)
        }
        Node::Match { scrutinee, arms } => {
            check_no_holes(scrutinee)?;
            for a in arms {
                check_no_holes(a)?;
            }
            Ok(())
        }
        Node::Arm { pattern, body } => {
            check_no_holes(pattern)?;
            check_no_holes(body)
        }
        Node::Record { fields } => {
            for (_, v) in fields {
                check_no_holes(v)?;
            }
            Ok(())
        }
        Node::Proj { record, .. } => check_no_holes(record),
        Node::Ctor { args, .. } => {
            for a in args {
                check_no_holes(a)?;
            }
            Ok(())
        }
        Node::PatCtor { sub_patterns, .. } => {
            for p in sub_patterns {
                check_no_holes(p)?;
            }
            Ok(())
        }
        Node::Var { .. }
        | Node::Int { .. }
        | Node::Str { .. }
        | Node::Sym { .. }
        | Node::Imports { .. }
        | Node::Import { .. }
        | Node::Exports { .. }
        | Node::Export { .. }
        | Node::Sig { .. }
        | Node::Ref { .. }
        | Node::PatWild
        | Node::PatVar
        | Node::PatInt { .. }
        | Node::FnTy { .. }
        | Node::TyVar { .. }
        | Node::Forall { .. }
        | Node::EffSet { .. }
        | Node::EffVar { .. } => Ok(()),
    }
}

/// Check that no `Var` in `body` references a binder above the `n`-deep
/// lambda binder — i.e., that the body has no free DeBruijn index ≥ n.
/// `n` is 1 for a single-arg `Lam` (ADR 0026 § 1).
pub fn check_closed(body: &Node, n: u64) -> Result<(), CodegenError> {
    fn go(node: &Node, depth: u64) -> Result<(), CodegenError> {
        match node {
            Node::Var { index } => {
                if *index >= depth {
                    Err(CodegenError::FreeVarInLambda { index: *index })
                } else {
                    Ok(())
                }
            }
            Node::Lam { body } => go(body, depth + 1),
            Node::Let { rhs, body } => {
                go(rhs, depth)?;
                go(body, depth + 1)
            }
            Node::Rec { bindings, body } => {
                let inner = depth + bindings.len() as u64;
                for b in bindings {
                    go(b, inner)?;
                }
                go(body, inner)
            }
            Node::Module { bindings } => {
                let inner = depth + bindings.len() as u64;
                for b in bindings {
                    go(b, inner)?;
                }
                Ok(())
            }
            Node::Unit { defs, .. } | Node::Defs { defs } => {
                for def in defs {
                    go(def, depth)?;
                }
                Ok(())
            }
            Node::Def { body, .. } => go(body, depth),
            Node::App { fn_, arg } => {
                go(fn_, depth)?;
                go(arg, depth)
            }
            Node::If { cond, then, else_ } => {
                go(cond, depth)?;
                go(then, depth)?;
                go(else_, depth)
            }
            Node::Match { scrutinee, arms } => {
                go(scrutinee, depth)?;
                for a in arms {
                    go(a, depth)?;
                }
                Ok(())
            }
            Node::Arm { pattern, body } => {
                let pv_count = count_pat_vars(pattern);
                go(body, depth + pv_count)
            }
            Node::Record { fields } => {
                for (_, v) in fields {
                    go(v, depth)?;
                }
                Ok(())
            }
            Node::Proj { record, .. } => go(record, depth),
            Node::Ctor { args, .. } => {
                for a in args {
                    go(a, depth)?;
                }
                Ok(())
            }
            Node::Ann { expr, .. } => go(expr, depth),
            Node::Hole { .. }
            | Node::Int { .. }
            | Node::Str { .. }
            | Node::Sym { .. }
            | Node::Imports { .. }
            | Node::Import { .. }
            | Node::Exports { .. }
            | Node::Export { .. }
            | Node::Sig { .. }
            | Node::Ref { .. }
            | Node::PatWild
            | Node::PatVar
            | Node::PatCtor { .. }
            | Node::PatInt { .. }
            | Node::FnTy { .. }
            | Node::TyVar { .. }
            | Node::Forall { .. }
            | Node::EffSet { .. }
            | Node::EffVar { .. } => Ok(()),
        }
    }
    go(body, n)
}

/// Count the number of `pat-var` bindings introduced by a pattern.
/// `pat-var` adds 1; `pat-ctor` recurses into its sub-patterns; other
/// patterns add 0.
pub fn count_pat_vars(pat: &Node) -> u64 {
    match pat {
        Node::PatVar => 1,
        Node::PatCtor { sub_patterns, .. } => sub_patterns.iter().map(count_pat_vars).sum(),
        _ => 0,
    }
}

/// Sanitize a string for use as an LLVM symbol-name suffix. Replaces
/// any non-alphanumeric byte with `_`. Used by the synthetic-name
/// generator (ADR 0026 § 2 / ADR 0027 § 4).
pub fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// Parse a canonical-form integer literal into `i64`. Canonical-text-format
/// stores integers as already-normalized decimal strings (ADR 0010); this
/// just rejects anything outside `i64`'s range with a structured error.
pub fn parse_int_literal(decimal: &str) -> Result<i64, CodegenError> {
    decimal
        .parse::<i64>()
        .map_err(|_| CodegenError::IntegerOverflow {
            value: decimal.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sym(name: &str) -> Node {
        Node::Sym { name: name.into() }
    }
    fn int(v: &str) -> Node {
        Node::Int { value: v.into() }
    }
    fn app(f: Node, a: Node) -> Node {
        Node::App {
            fn_: Box::new(f),
            arg: Box::new(a),
        }
    }
    fn lam(body: Node) -> Node {
        Node::Lam {
            body: Box::new(body),
        }
    }
    fn var(i: u64) -> Node {
        Node::Var { index: i }
    }

    #[test]
    fn unfold_simple_app() {
        let ast = app(app(sym("write"), int("1")), int("2"));
        let (head, args) = unfold_app(&ast);
        assert!(matches!(head, Node::Sym { name } if name == "write"));
        assert_eq!(args.len(), 2);
        assert!(matches!(args[0], Node::Int { value } if value == "1"));
        assert!(matches!(args[1], Node::Int { value } if value == "2"));
    }

    #[test]
    fn unfold_three_arg_spine() {
        // (app (app (app (sym write) (int 1)) (int 2)) (int 3))
        let ast = app(app(app(sym("write"), int("1")), int("2")), int("3"));
        let (head, args) = unfold_app(&ast);
        assert!(matches!(head, Node::Sym { name } if name == "write"));
        assert_eq!(args.len(), 3);
    }

    #[test]
    fn unfold_non_app_returns_self() {
        let n = int("42");
        let (head, args) = unfold_app(&n);
        assert!(matches!(head, Node::Int { value } if value == "42"));
        assert!(args.is_empty());
    }

    #[test]
    fn unfold_lam_head_app() {
        // ((lam (var 0)) (int 5))
        let ast = app(lam(var(0)), int("5"));
        let (head, args) = unfold_app(&ast);
        assert!(matches!(head, Node::Lam { .. }));
        assert_eq!(args.len(), 1);
    }

    #[test]
    fn collect_lam_chain_counts_consecutive_lambdas() {
        let ast = lam(lam(app(var(1), var(0))));
        let (arity, body) = collect_lam_chain(&ast).expect("lambda chain");
        assert_eq!(arity, 2);
        assert!(matches!(body, Node::App { .. }));
        assert!(collect_lam_chain(&int("0")).is_none());
    }

    #[test]
    fn closed_check_passes_for_var_zero_under_lam() {
        // body of (lam <body>) is closed iff body has no var index >= 1.
        assert!(check_closed(&var(0), 1).is_ok());
    }

    #[test]
    fn closed_check_fails_for_free_var() {
        assert!(matches!(
            check_closed(&var(1), 1),
            Err(CodegenError::FreeVarInLambda { index: 1 })
        ));
    }

    #[test]
    fn closed_check_lambda_extends_depth() {
        // (lam (var 1)) inside a 1-deep lambda is closed: inner var 1
        // resolves to the outer binder, not above it.
        let inner = lam(var(1));
        assert!(check_closed(&inner, 1).is_ok());
    }

    #[test]
    fn closed_check_let_extends_body_depth() {
        // body of (let rhs body) sees +1 depth; rhs sees same depth.
        let n = Node::Let {
            rhs: Box::new(int("5")),
            body: Box::new(var(0)), // refers to the new binding
        };
        assert!(check_closed(&n, 0).is_ok());

        let bad = Node::Let {
            rhs: Box::new(var(0)), // free at depth 0
            body: Box::new(int("0")),
        };
        assert!(matches!(
            check_closed(&bad, 0),
            Err(CodegenError::FreeVarInLambda { index: 0 })
        ));
    }

    #[test]
    fn closed_check_rec_extends_by_n_bindings() {
        // (rec (lam (var 1)) (lam (var 0)) body) — 2 bindings, so inside
        // each lambda body we have +1 (lam) + +2 (rec) = +3 depth.
        // From outer depth 0, var 1 inside the first lam is OK (resolves
        // to the rec's binding 1).
        let body0 = lam(var(1));
        let body1 = lam(var(0));
        let body = var(0);
        let n = Node::Rec {
            bindings: vec![body0, body1],
            body: Box::new(body),
        };
        assert!(check_closed(&n, 0).is_ok());
    }

    #[test]
    fn closed_check_pat_var_extends_depth() {
        // match (var 0) with | (pat-ctor X (pat-var)) => (var 0)
        // Inside the arm body, depth is 1 (pat-var) + outer 1 = 2; var 0 OK.
        let arm_body = var(0);
        let pat = Node::PatCtor {
            name: "X".into(),
            sub_patterns: vec![Node::PatVar],
        };
        let m = Node::Match {
            scrutinee: Box::new(var(0)),
            arms: vec![Node::Arm {
                pattern: Box::new(pat),
                body: Box::new(arm_body),
            }],
        };
        assert!(check_closed(&m, 1).is_ok());
    }

    #[test]
    fn no_holes_rejects_hole() {
        let h = Node::Hole {
            diag_id: "X".into(),
            payload: Box::new(Node::Str { value: "".into() }),
        };
        assert!(matches!(check_no_holes(&h), Err(CodegenError::Hole { .. })));
    }

    #[test]
    fn no_holes_passes_normal_ast() {
        let n = app(sym("add"), int("1"));
        assert!(check_no_holes(&n).is_ok());
    }

    #[test]
    fn no_holes_finds_nested_hole() {
        // (lam (let (int 0) (hole HOLE "msg")))
        let h = Node::Hole {
            diag_id: "HOLE".into(),
            payload: Box::new(Node::Str {
                value: "msg".into(),
            }),
        };
        let inner = Node::Let {
            rhs: Box::new(int("0")),
            body: Box::new(h),
        };
        let outer = lam(inner);
        assert!(matches!(
            check_no_holes(&outer),
            Err(CodegenError::Hole { .. })
        ));
    }

    #[test]
    fn count_pat_vars_simple() {
        assert_eq!(count_pat_vars(&Node::PatWild), 0);
        assert_eq!(count_pat_vars(&Node::PatVar), 1);
    }

    #[test]
    fn count_pat_vars_nested_ctor() {
        // (pat-ctor X (pat-var) (pat-wild) (pat-var)) → 2
        let pat = Node::PatCtor {
            name: "X".into(),
            sub_patterns: vec![Node::PatVar, Node::PatWild, Node::PatVar],
        };
        assert_eq!(count_pat_vars(&pat), 2);
    }

    #[test]
    fn parse_int_literal_basic() {
        assert_eq!(parse_int_literal("0").unwrap(), 0);
        assert_eq!(parse_int_literal("42").unwrap(), 42);
        assert_eq!(parse_int_literal("-1").unwrap(), -1);
    }

    #[test]
    fn parse_int_literal_overflow() {
        let err = parse_int_literal("99999999999999999999").unwrap_err();
        assert!(matches!(err, CodegenError::IntegerOverflow { .. }));
    }

    #[test]
    fn sanitize_keeps_alnum() {
        assert_eq!(sanitize("foo123"), "foo123");
        assert_eq!(sanitize("a-b"), "a_b");
        assert_eq!(sanitize(""), "");
    }
}
