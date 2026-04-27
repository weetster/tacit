//! Type and effect signatures for built-in @name primitives (ADR 0028, 0030, 0042).
//!
//! Effect sets mirror `stdlib/libc-effects.toml` (ADR 0025, consumed in Stage 3).
//! The canonical source for primitive effects is that TOML file; values here match it.

use crate::ty::{EffAtom, EffSet, FnEff, Ty};

/// Look up the type of a `@name` primitive by name.
/// Returns `None` if the name is not a known primitive.
///
/// IO effect sits at the innermost (final) application: partial applications
/// are pure closures; IO is produced only when all args are supplied.
pub fn prim_type(name: &str) -> Option<Ty> {
    Some(match name {
        // LIBC: write(fd: Int, buf: Str, len: Int) -> Int / IO
        "write" => fn3_io(Ty::Int, Ty::Str, Ty::Int, Ty::Int),
        // LIBC: read(fd: Int, buf: Str, len: Int) -> Int / IO
        "read" => fn3_io(Ty::Int, Ty::Str, Ty::Int, Ty::Int),
        // LIBC: exit(code: Int) -> Int / IO
        "exit" => fn1_io(Ty::Int, Ty::Int),
        // ARITH: Int → Int → Int (pure)
        "add" | "sub" | "mul" | "div" | "mod" => fn2_pure(Ty::Int, Ty::Int, Ty::Int),
        // CMP: Int → Int → Bool (pure, ADR 0042)
        "eq" | "ne" | "lt" | "le" | "gt" | "ge" => fn2_pure(Ty::Int, Ty::Int, Ty::Bool),
        _ => return None,
    })
}

/// True if `name` is a known arithmetic operator.
pub fn is_arith(name: &str) -> bool {
    matches!(name, "add" | "sub" | "mul" | "div" | "mod")
}

/// True if `name` is a known comparison operator.
pub fn is_cmp(name: &str) -> bool {
    matches!(name, "eq" | "ne" | "lt" | "le" | "gt" | "ge")
}

/// True if `name` is an IO-producing primitive (from libc-effects.toml).
pub fn is_io_prim(name: &str) -> bool {
    matches!(name, "write" | "read" | "exit")
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn io_eff() -> FnEff {
    FnEff::from_set(EffSet::of([EffAtom::IO]))
}

/// Unary function with IO on the single application.
fn fn1_io(a: Ty, r: Ty) -> Ty {
    Ty::Fn(Box::new(a), Box::new(r), io_eff())
}

/// Binary function, all pure; IO at the innermost application.
#[allow(dead_code)]
fn fn2_io(a: Ty, b: Ty, r: Ty) -> Ty {
    Ty::Fn(
        Box::new(a),
        Box::new(Ty::Fn(Box::new(b), Box::new(r), io_eff())),
        FnEff::pure_(),
    )
}

/// Ternary function, IO only at the innermost (fully-applied) step.
fn fn3_io(a: Ty, b: Ty, c: Ty, r: Ty) -> Ty {
    Ty::Fn(
        Box::new(a),
        Box::new(Ty::Fn(
            Box::new(b),
            Box::new(Ty::Fn(Box::new(c), Box::new(r), io_eff())),
            FnEff::pure_(),
        )),
        FnEff::pure_(),
    )
}

/// Unary pure function.
#[allow(dead_code)]
fn fn1_pure(a: Ty, r: Ty) -> Ty {
    Ty::Fn(Box::new(a), Box::new(r), FnEff::pure_())
}

/// Binary pure function.
fn fn2_pure(a: Ty, b: Ty, r: Ty) -> Ty {
    Ty::Fn(
        Box::new(a),
        Box::new(Ty::Fn(Box::new(b), Box::new(r), FnEff::pure_())),
        FnEff::pure_(),
    )
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_type_has_io_at_innermost() {
        let t = prim_type("write").unwrap();
        // write :: Int → (Str → (Int → Int / IO) / {}) / {}
        match &t {
            Ty::Fn(a, mid, outer_eff) => {
                assert_eq!(a.as_ref(), &Ty::Int);
                assert_eq!(outer_eff, &FnEff::pure_());
                match mid.as_ref() {
                    Ty::Fn(b, inner, mid_eff) => {
                        assert_eq!(b.as_ref(), &Ty::Str);
                        assert_eq!(mid_eff, &FnEff::pure_());
                        match inner.as_ref() {
                            Ty::Fn(c, r, inner_eff) => {
                                assert_eq!(c.as_ref(), &Ty::Int);
                                assert_eq!(r.as_ref(), &Ty::Int);
                                assert_eq!(inner_eff, &FnEff::from_set(EffSet::of([EffAtom::IO])));
                            }
                            _ => panic!("expected innermost Fn"),
                        }
                    }
                    _ => panic!("expected middle Fn"),
                }
            }
            _ => panic!("expected outer Fn"),
        }
    }

    #[test]
    fn exit_type_has_io() {
        let t = prim_type("exit").unwrap();
        // exit :: Int → Int / IO
        match &t {
            Ty::Fn(_, _, eff) => {
                assert_eq!(eff, &FnEff::from_set(EffSet::of([EffAtom::IO])));
            }
            _ => panic!("expected Fn"),
        }
    }

    #[test]
    fn add_type_is_pure() {
        let t = prim_type("add").unwrap();
        match &t {
            Ty::Fn(_, mid, outer_eff) => {
                assert_eq!(outer_eff, &FnEff::pure_());
                match mid.as_ref() {
                    Ty::Fn(_, _, inner_eff) => assert_eq!(inner_eff, &FnEff::pure_()),
                    _ => panic!("expected inner Fn"),
                }
            }
            _ => panic!("expected Fn"),
        }
    }

    #[test]
    fn gt_returns_bool() {
        let t = prim_type("gt").unwrap();
        assert!(matches!(
            &t,
            Ty::Fn(_, ret, _) if matches!(ret.as_ref(), Ty::Fn(_, r, _) if matches!(r.as_ref(), Ty::Bool))
        ));
    }

    #[test]
    fn unknown_prim() {
        assert!(prim_type("frobnicate").is_none());
    }
}
