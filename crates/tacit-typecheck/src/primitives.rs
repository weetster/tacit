//! Type and effect signatures for built-in @name primitives (ADR 0028, 0030, 0042, 0047, 0061, 0062, 0063).
//!
//! Effect sets mirror `stdlib/libc-effects.toml` (ADR 0025, consumed in Stage 3).
//! The canonical source for primitive effects is that TOML file; values here match it.
//! Phase 3 additions (ADR 0047): PARSE, FORMAT, MEM categories, STACK-ALLOC extension.
//! Library-mediated additions:
//! - ADR 0061: I64Vec allocation and element operations.
//! - ADR 0062: text indexing into I64Vec range tables.
//! - ADR 0063: multi-delimiter token indexing.

use crate::ty::{EffAtom, EffSet, FnEff, Ty};

/// Look up the type of a `@name` primitive by name.
/// Returns `None` if the name is not a known primitive.
///
/// IO effect sits at the innermost (final) application: partial applications
/// are pure closures; IO is produced only when all args are supplied.
pub fn prim_type(name: &str) -> Option<Ty> {
    Some(match name {
        // LIBC: write(fd: Int, buf: Buf|Str, len: Int) -> Int / IO
        "write" => fn3_io(Ty::Int, Ty::Unknown, Ty::Int, Ty::Int),
        // LIBC: read(fd: Int, buf: Buf, len: Int) -> Int / {IO, Mut}
        "read" => fn3_mut_io(Ty::Int, Ty::Unknown, Ty::Int, Ty::Int),
        // LIBC: exit(code: Int) -> Int / IO
        "exit" => fn1_io(Ty::Int, Ty::Int),
        // STACK-ALLOC: buf-alloc(size: Int) -> Buf / {Alloc} (ADR 0038)
        "buf-alloc" => fn1_alloc(Ty::Int, Ty::Buf),
        // STACK-ALLOC: buf-alloc-dyn(size: Int) -> Buf / {Alloc} (ADR 0047)
        "buf-alloc-dyn" => fn1_alloc(Ty::Int, Ty::Buf),
        // MEM: buf-get(buf: Buf, off: Int) -> Int  (pure)
        "buf-get" => fn2_pure(Ty::Buf, Ty::Int, Ty::Int),
        // MEM: buf-set(buf: Buf, off: Int, byte: Int) -> Int  / {Mut}
        "buf-set" => fn3_mut(Ty::Buf, Ty::Int, Ty::Int, Ty::Int),
        // MEM: buf-copy(dst: Buf, dst-off: Int, src: Buf, src-off: Int, len: Int) -> Int / {Mut}
        "buf-copy" => fn5_mut(Ty::Buf, Ty::Int, Ty::Buf, Ty::Int, Ty::Int, Ty::Int),
        // MEM: buf-eq(a: Buf, a-off: Int, b: Buf, b-off: Int, len: Int) -> Int  (pure)
        "buf-eq" => fn5_pure(Ty::Buf, Ty::Int, Ty::Buf, Ty::Int, Ty::Int, Ty::Int),
        // MEM: scan-byte(buf: Buf, off: Int, len: Int, target: Int) -> Int  (pure)
        "scan-byte" => fn4_pure(Ty::Buf, Ty::Int, Ty::Int, Ty::Int, Ty::Int),
        // PARSE: parse-i64(buf: Buf, off: Int, len: Int) -> Int  (pure)
        "parse-i64" => fn3_pure(Ty::Buf, Ty::Int, Ty::Int, Ty::Int),
        // FORMAT: fmt-i64(buf: Buf, off: Int, val: Int) -> Int  / {Mut}
        "fmt-i64" => fn3_mut(Ty::Buf, Ty::Int, Ty::Int, Ty::Int),
        // I64VEC-ALLOC: i64-alloc(count: Int) -> I64Vec / {Alloc}
        "i64-alloc" => fn1_alloc(Ty::Int, Ty::I64Vec),
        // I64VEC: i64-get(vec: I64Vec, index: Int) -> Int (pure)
        "i64-get" => fn2_pure(Ty::I64Vec, Ty::Int, Ty::Int),
        // I64VEC: i64-set(vec: I64Vec, index: Int, value: Int) -> Int / {Mut}
        "i64-set" => fn3_mut(Ty::I64Vec, Ty::Int, Ty::Int, Ty::Int),
        // I64VEC: i64-swap(vec: I64Vec, i: Int, j: Int) -> Int / {Mut}
        "i64-swap" => fn3_mut(Ty::I64Vec, Ty::Int, Ty::Int, Ty::Int),
        // I64VEC: i64-copy(dst, dst-index, src, src-index, count) -> Int / {Mut}
        "i64-copy" => fn5_mut(Ty::I64Vec, Ty::Int, Ty::I64Vec, Ty::Int, Ty::Int, Ty::Int),
        // TEXT-INDEX: line-index(text: Buf, len: Int, table: I64Vec) -> Int / {Mut}
        "line-index" => fn3_mut(Ty::Buf, Ty::Int, Ty::I64Vec, Ty::Int),
        // TEXT-INDEX: token-index(text: Buf, off: Int, len: Int, delim: Int, table: I64Vec) -> Int / {Mut}
        "token-index" => fn5_mut(Ty::Buf, Ty::Int, Ty::Int, Ty::Int, Ty::I64Vec, Ty::Int),
        // TEXT-INDEX: token-index-any(text: Buf, off: Int, len: Int, delims: Buf, delim-count: Int, table: I64Vec) -> Int / {Mut}
        "token-index-any" => fn6_mut(
            Ty::Buf,
            Ty::Int,
            Ty::Int,
            Ty::Buf,
            Ty::Int,
            Ty::I64Vec,
            Ty::Int,
        ),
        // RANGE-TABLE: range-start(table: I64Vec, index: Int) -> Int (pure)
        "range-start" => fn2_pure(Ty::I64Vec, Ty::Int, Ty::Int),
        // RANGE-TABLE: range-len(table: I64Vec, index: Int) -> Int (pure)
        "range-len" => fn2_pure(Ty::I64Vec, Ty::Int, Ty::Int),
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

/// True if `name` is an Alloc-producing primitive.
pub fn is_alloc_prim(name: &str) -> bool {
    matches!(name, "buf-alloc" | "buf-alloc-dyn" | "i64-alloc")
}

/// True if `name` is a Mut-producing primitive (ADR 0047).
pub fn is_mut_prim(name: &str) -> bool {
    matches!(
        name,
        "buf-set"
            | "buf-copy"
            | "fmt-i64"
            | "i64-set"
            | "i64-swap"
            | "i64-copy"
            | "line-index"
            | "token-index"
            | "token-index-any"
    )
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn io_eff() -> FnEff {
    FnEff::from_set(EffSet::of([EffAtom::IO]))
}

fn alloc_eff() -> FnEff {
    FnEff::from_set(EffSet::of([EffAtom::Alloc]))
}

fn mut_eff() -> FnEff {
    FnEff::from_set(EffSet::of([EffAtom::Mut]))
}

fn io_mut_eff() -> FnEff {
    FnEff::from_set(EffSet::of([EffAtom::IO, EffAtom::Mut]))
}

/// Unary function with IO on the single application.
fn fn1_io(a: Ty, r: Ty) -> Ty {
    Ty::Fn(Box::new(a), Box::new(r), io_eff())
}

/// Unary function with Alloc on the single application.
fn fn1_alloc(a: Ty, r: Ty) -> Ty {
    Ty::Fn(Box::new(a), Box::new(r), alloc_eff())
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

/// Ternary function, {IO, Mut} at the innermost (fully-applied) step.
fn fn3_mut_io(a: Ty, b: Ty, c: Ty, r: Ty) -> Ty {
    Ty::Fn(
        Box::new(a),
        Box::new(Ty::Fn(
            Box::new(b),
            Box::new(Ty::Fn(Box::new(c), Box::new(r), io_mut_eff())),
            FnEff::pure_(),
        )),
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

/// Ternary pure function.
fn fn3_pure(a: Ty, b: Ty, c: Ty, r: Ty) -> Ty {
    Ty::Fn(
        Box::new(a),
        Box::new(Ty::Fn(
            Box::new(b),
            Box::new(Ty::Fn(Box::new(c), Box::new(r), FnEff::pure_())),
            FnEff::pure_(),
        )),
        FnEff::pure_(),
    )
}

/// Ternary function, {Mut} at the innermost (fully-applied) step.
fn fn3_mut(a: Ty, b: Ty, c: Ty, r: Ty) -> Ty {
    Ty::Fn(
        Box::new(a),
        Box::new(Ty::Fn(
            Box::new(b),
            Box::new(Ty::Fn(Box::new(c), Box::new(r), mut_eff())),
            FnEff::pure_(),
        )),
        FnEff::pure_(),
    )
}

/// Quaternary pure function.
fn fn4_pure(a: Ty, b: Ty, c: Ty, d: Ty, r: Ty) -> Ty {
    Ty::Fn(
        Box::new(a),
        Box::new(Ty::Fn(
            Box::new(b),
            Box::new(Ty::Fn(
                Box::new(c),
                Box::new(Ty::Fn(Box::new(d), Box::new(r), FnEff::pure_())),
                FnEff::pure_(),
            )),
            FnEff::pure_(),
        )),
        FnEff::pure_(),
    )
}

/// Quinary pure function.
fn fn5_pure(a: Ty, b: Ty, c: Ty, d: Ty, e: Ty, r: Ty) -> Ty {
    Ty::Fn(
        Box::new(a),
        Box::new(Ty::Fn(
            Box::new(b),
            Box::new(Ty::Fn(
                Box::new(c),
                Box::new(Ty::Fn(
                    Box::new(d),
                    Box::new(Ty::Fn(Box::new(e), Box::new(r), FnEff::pure_())),
                    FnEff::pure_(),
                )),
                FnEff::pure_(),
            )),
            FnEff::pure_(),
        )),
        FnEff::pure_(),
    )
}

/// Quinary function, {Mut} at the innermost (fully-applied) step.
fn fn5_mut(a: Ty, b: Ty, c: Ty, d: Ty, e: Ty, r: Ty) -> Ty {
    Ty::Fn(
        Box::new(a),
        Box::new(Ty::Fn(
            Box::new(b),
            Box::new(Ty::Fn(
                Box::new(c),
                Box::new(Ty::Fn(
                    Box::new(d),
                    Box::new(Ty::Fn(Box::new(e), Box::new(r), mut_eff())),
                    FnEff::pure_(),
                )),
                FnEff::pure_(),
            )),
            FnEff::pure_(),
        )),
        FnEff::pure_(),
    )
}

/// Six-argument function, {Mut} at the innermost (fully-applied) step.
fn fn6_mut(a: Ty, b: Ty, c: Ty, d: Ty, e: Ty, f: Ty, r: Ty) -> Ty {
    Ty::Fn(
        Box::new(a),
        Box::new(Ty::Fn(
            Box::new(b),
            Box::new(Ty::Fn(
                Box::new(c),
                Box::new(Ty::Fn(
                    Box::new(d),
                    Box::new(Ty::Fn(
                        Box::new(e),
                        Box::new(Ty::Fn(Box::new(f), Box::new(r), mut_eff())),
                        FnEff::pure_(),
                    )),
                    FnEff::pure_(),
                )),
                FnEff::pure_(),
            )),
            FnEff::pure_(),
        )),
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
        // write :: Int → (Unknown → (Int → Int / IO) / {}) / {}
        match &t {
            Ty::Fn(a, mid, outer_eff) => {
                assert_eq!(a.as_ref(), &Ty::Int);
                assert_eq!(outer_eff, &FnEff::pure_());
                match mid.as_ref() {
                    Ty::Fn(b, inner, mid_eff) => {
                        assert_eq!(b.as_ref(), &Ty::Unknown);
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

    #[test]
    fn buf_alloc_dyn_type() {
        let t = prim_type("buf-alloc-dyn").unwrap();
        match &t {
            Ty::Fn(a, r, eff) => {
                assert_eq!(a.as_ref(), &Ty::Int);
                assert_eq!(r.as_ref(), &Ty::Buf);
                assert_eq!(eff, &FnEff::from_set(EffSet::of([EffAtom::Alloc])));
            }
            _ => panic!("expected Fn"),
        }
    }

    #[test]
    fn buf_get_is_pure() {
        let t = prim_type("buf-get").unwrap();
        match &t {
            Ty::Fn(a, mid, e1) => {
                assert_eq!(a.as_ref(), &Ty::Buf);
                assert_eq!(e1, &FnEff::pure_());
                match mid.as_ref() {
                    Ty::Fn(b, r, e2) => {
                        assert_eq!(b.as_ref(), &Ty::Int);
                        assert_eq!(r.as_ref(), &Ty::Int);
                        assert_eq!(e2, &FnEff::pure_());
                    }
                    _ => panic!("expected inner Fn"),
                }
            }
            _ => panic!("expected Fn"),
        }
    }

    #[test]
    fn buf_set_has_mut() {
        let t = prim_type("buf-set").unwrap();
        // buf-set :: Buf → Int → Int → Int / {Mut}
        if let Ty::Fn(_, mid, _) = &t {
            if let Ty::Fn(_, inner, _) = mid.as_ref() {
                if let Ty::Fn(_, _, eff) = inner.as_ref() {
                    assert_eq!(eff, &FnEff::from_set(EffSet::of([EffAtom::Mut])));
                    return;
                }
            }
        }
        panic!("unexpected type shape for buf-set");
    }

    #[test]
    fn parse_i64_is_pure() {
        let t = prim_type("parse-i64").unwrap();
        if let Ty::Fn(_, mid, _) = &t {
            if let Ty::Fn(_, inner, _) = mid.as_ref() {
                if let Ty::Fn(_, r, eff) = inner.as_ref() {
                    assert_eq!(r.as_ref(), &Ty::Int);
                    assert_eq!(eff, &FnEff::pure_());
                    return;
                }
            }
        }
        panic!("unexpected type shape for parse-i64");
    }

    #[test]
    fn fmt_i64_has_mut() {
        let t = prim_type("fmt-i64").unwrap();
        if let Ty::Fn(_, mid, _) = &t {
            if let Ty::Fn(_, inner, _) = mid.as_ref() {
                if let Ty::Fn(_, _, eff) = inner.as_ref() {
                    assert_eq!(eff, &FnEff::from_set(EffSet::of([EffAtom::Mut])));
                    return;
                }
            }
        }
        panic!("unexpected type shape for fmt-i64");
    }

    #[test]
    fn scan_byte_is_pure() {
        let t = prim_type("scan-byte").unwrap();
        // scan-byte :: Buf → Int → Int → Int → Int (all pure)
        assert!(prim_type("scan-byte").is_some());
        // Verify it's a 4-arg function ending pure
        fn innermost(t: &Ty) -> &FnEff {
            match t {
                Ty::Fn(_, b, eff) => {
                    if matches!(b.as_ref(), Ty::Fn(_, _, _)) {
                        innermost(b)
                    } else {
                        eff
                    }
                }
                _ => panic!("not a Fn"),
            }
        }
        assert_eq!(innermost(&t), &FnEff::pure_());
    }

    #[test]
    fn buf_eq_and_buf_copy_arities() {
        // buf-eq and buf-copy are 5-arg functions
        fn depth(t: &Ty) -> usize {
            match t {
                Ty::Fn(_, b, _) => 1 + depth(b),
                _ => 0,
            }
        }
        assert_eq!(depth(&prim_type("buf-eq").unwrap()), 5);
        assert_eq!(depth(&prim_type("buf-copy").unwrap()), 5);
    }

    #[test]
    fn buf_copy_has_mut_at_innermost() {
        let t = prim_type("buf-copy").unwrap();
        fn innermost_eff(t: &Ty) -> FnEff {
            match t {
                Ty::Fn(_, b, eff) => {
                    if matches!(b.as_ref(), Ty::Fn(_, _, _)) {
                        innermost_eff(b)
                    } else {
                        eff.clone()
                    }
                }
                _ => panic!("not a Fn"),
            }
        }
        assert_eq!(
            innermost_eff(&t),
            FnEff::from_set(EffSet::of([EffAtom::Mut]))
        );
    }

    #[test]
    fn i64_alloc_type() {
        let t = prim_type("i64-alloc").unwrap();
        match &t {
            Ty::Fn(a, r, eff) => {
                assert_eq!(a.as_ref(), &Ty::Int);
                assert_eq!(r.as_ref(), &Ty::I64Vec);
                assert_eq!(eff, &FnEff::from_set(EffSet::of([EffAtom::Alloc])));
            }
            _ => panic!("expected Fn"),
        }
    }

    #[test]
    fn i64_get_is_pure() {
        let t = prim_type("i64-get").unwrap();
        match &t {
            Ty::Fn(a, mid, e1) => {
                assert_eq!(a.as_ref(), &Ty::I64Vec);
                assert_eq!(e1, &FnEff::pure_());
                match mid.as_ref() {
                    Ty::Fn(b, r, e2) => {
                        assert_eq!(b.as_ref(), &Ty::Int);
                        assert_eq!(r.as_ref(), &Ty::Int);
                        assert_eq!(e2, &FnEff::pure_());
                    }
                    _ => panic!("expected inner Fn"),
                }
            }
            _ => panic!("expected Fn"),
        }
    }

    #[test]
    fn i64_set_and_swap_have_mut() {
        for name in ["i64-set", "i64-swap"] {
            let t = prim_type(name).unwrap();
            if let Ty::Fn(a, mid, _) = &t {
                assert_eq!(a.as_ref(), &Ty::I64Vec);
                if let Ty::Fn(_, inner, _) = mid.as_ref() {
                    if let Ty::Fn(_, _, eff) = inner.as_ref() {
                        assert_eq!(eff, &FnEff::from_set(EffSet::of([EffAtom::Mut])));
                        continue;
                    }
                }
            }
            panic!("unexpected type shape for {name}");
        }
    }

    #[test]
    fn i64_copy_has_expected_shape_and_mut() {
        let t = prim_type("i64-copy").unwrap();
        fn args_and_eff(t: &Ty, args: &mut Vec<Ty>) -> FnEff {
            match t {
                Ty::Fn(a, b, eff) => {
                    args.push(a.as_ref().clone());
                    if matches!(b.as_ref(), Ty::Fn(_, _, _)) {
                        args_and_eff(b, args)
                    } else {
                        eff.clone()
                    }
                }
                _ => panic!("not a Fn"),
            }
        }
        let mut args = Vec::new();
        let eff = args_and_eff(&t, &mut args);
        assert_eq!(
            args,
            vec![Ty::I64Vec, Ty::Int, Ty::I64Vec, Ty::Int, Ty::Int]
        );
        assert_eq!(eff, FnEff::from_set(EffSet::of([EffAtom::Mut])));
    }

    #[test]
    fn token_index_any_has_expected_shape_and_mut() {
        let t = prim_type("token-index-any").unwrap();
        fn args_and_eff(t: &Ty, args: &mut Vec<Ty>) -> FnEff {
            match t {
                Ty::Fn(a, b, eff) => {
                    args.push(a.as_ref().clone());
                    if matches!(b.as_ref(), Ty::Fn(_, _, _)) {
                        args_and_eff(b, args)
                    } else {
                        eff.clone()
                    }
                }
                _ => panic!("not a Fn"),
            }
        }
        let mut args = Vec::new();
        let eff = args_and_eff(&t, &mut args);
        assert_eq!(
            args,
            vec![Ty::Buf, Ty::Int, Ty::Int, Ty::Buf, Ty::Int, Ty::I64Vec]
        );
        assert_eq!(eff, FnEff::from_set(EffSet::of([EffAtom::Mut])));
    }
}
