//! Type representation and substitution for Phase 2 type inference.
//!
//! Effects are tracked as `EffSet` for Stage 3; Stage 2 only checks types.

use std::collections::BTreeMap;
use std::collections::HashMap;

/// A Tacit-Lite type.
#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    /// `Int` (i64) — the sole numeric type in Phase 2.
    Int,
    /// `Bool` (i1) — comparison result type (ADR 0042).
    Bool,
    /// `Str` — string type (for @write buffer argument).
    Str,
    /// Function type `arg → ret`.
    Fn(Box<Ty>, Box<Ty>),
    /// Record type.
    Record(BTreeMap<String, Ty>),
    /// Type application, e.g. `List Int` in Phase 3+.
    App(Box<Ty>, Box<Ty>),
    /// Unification metavariable (generated during inference).
    Meta(u32),
    /// Error-recovery type: does not unify with anything; prevents cascading errors.
    Unknown,
}

impl Ty {
    pub fn is_unknown(&self) -> bool {
        matches!(self, Ty::Unknown)
    }

    /// True if this type has no remaining metavariables after applying `subst`.
    pub fn is_ground(&self, subst: &Subst) -> bool {
        match subst.apply(self) {
            Ty::Int | Ty::Bool | Ty::Str | Ty::Unknown => true,
            Ty::Fn(a, b) => a.is_ground(subst) && b.is_ground(subst),
            Ty::Record(fields) => fields.values().all(|v| v.is_ground(subst)),
            Ty::App(f, a) => f.is_ground(subst) && a.is_ground(subst),
            Ty::Meta(_) => false,
        }
    }
}

impl std::fmt::Display for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ty::Int => write!(f, "Int"),
            Ty::Bool => write!(f, "Bool"),
            Ty::Str => write!(f, "Str"),
            Ty::Fn(a, b) => {
                let parens = matches!(a.as_ref(), Ty::Fn(_, _));
                if parens {
                    write!(f, "({}) -> {}", a, b)
                } else {
                    write!(f, "{} -> {}", a, b)
                }
            }
            Ty::Record(fields) => {
                write!(f, "{{")?;
                for (i, (k, v)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", k, v)?;
                }
                write!(f, "}}")
            }
            Ty::App(fun, arg) => write!(f, "{} {}", fun, arg),
            Ty::Meta(n) => write!(f, "?{}", n),
            Ty::Unknown => write!(f, "<unknown>"),
        }
    }
}

/// Substitution: maps metavariable IDs to types.
#[derive(Default)]
pub struct Subst {
    map: HashMap<u32, Ty>,
    next: u32,
}

impl Subst {
    /// Allocate a fresh unification metavariable.
    pub fn fresh(&mut self) -> Ty {
        let id = self.next;
        self.next += 1;
        Ty::Meta(id)
    }

    /// Apply the substitution to a type, following chains.
    pub fn apply(&self, ty: &Ty) -> Ty {
        match ty {
            Ty::Meta(id) => {
                if let Some(t) = self.map.get(id) {
                    self.apply(t)
                } else {
                    ty.clone()
                }
            }
            Ty::Fn(a, b) => Ty::Fn(Box::new(self.apply(a)), Box::new(self.apply(b))),
            Ty::Record(fields) => {
                Ty::Record(fields.iter().map(|(k, v)| (k.clone(), self.apply(v))).collect())
            }
            Ty::App(fun, arg) => Ty::App(Box::new(self.apply(fun)), Box::new(self.apply(arg))),
            other => other.clone(),
        }
    }

    /// Bind metavariable `id` to `ty`. Panics on occur-check violation (caller
    /// is responsible for checking with `occurs` first if needed).
    pub fn bind(&mut self, id: u32, ty: Ty) {
        self.map.insert(id, ty);
    }
}

/// True if metavariable `id` appears free in `ty` (occur check).
pub fn occurs(id: u32, ty: &Ty, subst: &Subst) -> bool {
    match subst.apply(ty) {
        Ty::Meta(other) => other == id,
        Ty::Fn(a, b) => occurs(id, &a, subst) || occurs(id, &b, subst),
        Ty::Record(fields) => fields.values().any(|v| occurs(id, v, subst)),
        Ty::App(f, a) => occurs(id, &f, subst) || occurs(id, &a, subst),
        _ => false,
    }
}

/// Attempt to unify `t1` and `t2` under `subst`.
/// Returns `true` on success (subst is updated), `false` on failure (subst unchanged).
pub fn unify(t1: &Ty, t2: &Ty, subst: &mut Subst) -> bool {
    let t1 = subst.apply(t1);
    let t2 = subst.apply(t2);

    match (&t1, &t2) {
        // Unknown propagates without failure to avoid cascading errors.
        (Ty::Unknown, _) | (_, Ty::Unknown) => true,
        (Ty::Int, Ty::Int) | (Ty::Bool, Ty::Bool) | (Ty::Str, Ty::Str) => true,
        (Ty::Meta(id1), Ty::Meta(id2)) if id1 == id2 => true,
        (Ty::Meta(id), other) => {
            if occurs(*id, other, subst) {
                false
            } else {
                subst.bind(*id, other.clone());
                true
            }
        }
        (other, Ty::Meta(id)) => {
            if occurs(*id, other, subst) {
                false
            } else {
                subst.bind(*id, other.clone());
                true
            }
        }
        (Ty::Fn(a1, b1), Ty::Fn(a2, b2)) => {
            let a1 = a1.clone();
            let b1 = b1.clone();
            let a2 = a2.clone();
            let b2 = b2.clone();
            unify(&a1, &a2, subst) && unify(&b1, &b2, subst)
        }
        (Ty::Record(f1), Ty::Record(f2)) => {
            if f1.len() != f2.len() {
                return false;
            }
            let pairs: Vec<_> = f1
                .iter()
                .zip(f2.iter())
                .map(|((k1, v1), (k2, v2))| (k1.clone(), v1.clone(), k2.clone(), v2.clone()))
                .collect();
            pairs.iter().all(|(k1, v1, k2, v2)| k1 == k2 && unify(v1, v2, subst))
        }
        (Ty::App(f1, a1), Ty::App(f2, a2)) => {
            let f1 = f1.clone();
            let a1 = a1.clone();
            let f2 = f2.clone();
            let a2 = a2.clone();
            unify(&f1, &f2, subst) && unify(&a1, &a2, subst)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_gives_distinct_ids() {
        let mut s = Subst::default();
        let a = s.fresh();
        let b = s.fresh();
        assert_ne!(a, b);
    }

    #[test]
    fn unify_int_int() {
        let mut s = Subst::default();
        assert!(unify(&Ty::Int, &Ty::Int, &mut s));
    }

    #[test]
    fn unify_int_bool_fails() {
        let mut s = Subst::default();
        assert!(!unify(&Ty::Int, &Ty::Bool, &mut s));
    }

    #[test]
    fn unify_meta_int() {
        let mut s = Subst::default();
        let m = s.fresh();
        assert!(unify(&m, &Ty::Int, &mut s));
        assert_eq!(s.apply(&m), Ty::Int);
    }

    #[test]
    fn unify_fn_types() {
        let mut s = Subst::default();
        let m = s.fresh();
        let t1 = Ty::Fn(Box::new(Ty::Int), Box::new(m.clone()));
        let t2 = Ty::Fn(Box::new(Ty::Int), Box::new(Ty::Bool));
        assert!(unify(&t1, &t2, &mut s));
        assert_eq!(s.apply(&m), Ty::Bool);
    }

    #[test]
    fn occur_check_prevents_infinite_type() {
        let mut s = Subst::default();
        let m = s.fresh();
        let recursive = Ty::Fn(Box::new(m.clone()), Box::new(m.clone()));
        // m = Fn(m, m) would create an infinite type
        assert!(!unify(&m, &recursive, &mut s));
    }
}
