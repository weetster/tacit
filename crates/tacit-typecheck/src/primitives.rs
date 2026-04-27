//! Type signatures for built-in @name primitives (ADR 0028, 0030, 0042).
//!
//! LIBC primitives get their types from here; libc-effects.toml provides
//! their effect sets (consumed by Stage 3).

use crate::ty::Ty;

/// Look up the type of a `@name` primitive by name.
/// Returns `None` if the name is not a known primitive.
pub fn prim_type(name: &str) -> Option<Ty> {
    Some(match name {
        // LIBC: write(fd: Int, buf: Str, len: Int) -> Int / IO
        "write" => fn3(Ty::Int, Ty::Str, Ty::Int, Ty::Int),
        // LIBC: read(fd: Int, buf: Str, len: Int) -> Int / IO  (Stage 4)
        "read" => fn3(Ty::Int, Ty::Str, Ty::Int, Ty::Int),
        // LIBC: exit(code: Int) -> Int / IO
        "exit" => fn1(Ty::Int, Ty::Int),
        // ARITH: Int → Int → Int (pure)
        "add" | "sub" | "mul" | "div" | "mod" => fn2(Ty::Int, Ty::Int, Ty::Int),
        // CMP: Int → Int → Bool (pure, ADR 0042)
        "eq" | "ne" | "lt" | "le" | "gt" | "ge" => fn2(Ty::Int, Ty::Int, Ty::Bool),
        _ => return None,
    })
}

/// True if `name` is a known arithmetic operator (ARITH category).
pub fn is_arith(name: &str) -> bool {
    matches!(name, "add" | "sub" | "mul" | "div" | "mod")
}

/// True if `name` is a known comparison operator (CMP category).
pub fn is_cmp(name: &str) -> bool {
    matches!(name, "eq" | "ne" | "lt" | "le" | "gt" | "ge")
}

fn fn1(a: Ty, r: Ty) -> Ty {
    Ty::Fn(Box::new(a), Box::new(r))
}

fn fn2(a: Ty, b: Ty, r: Ty) -> Ty {
    Ty::Fn(Box::new(a), Box::new(Ty::Fn(Box::new(b), Box::new(r))))
}

fn fn3(a: Ty, b: Ty, c: Ty, r: Ty) -> Ty {
    Ty::Fn(
        Box::new(a),
        Box::new(Ty::Fn(Box::new(b), Box::new(Ty::Fn(Box::new(c), Box::new(r))))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_type() {
        // write : Int -> Str -> Int -> Int
        let t = prim_type("write").unwrap();
        assert_eq!(
            t,
            Ty::Fn(
                Box::new(Ty::Int),
                Box::new(Ty::Fn(
                    Box::new(Ty::Str),
                    Box::new(Ty::Fn(Box::new(Ty::Int), Box::new(Ty::Int)))
                ))
            )
        );
    }

    #[test]
    fn add_type() {
        // add : Int -> Int -> Int
        let t = prim_type("add").unwrap();
        assert_eq!(
            t,
            Ty::Fn(
                Box::new(Ty::Int),
                Box::new(Ty::Fn(Box::new(Ty::Int), Box::new(Ty::Int)))
            )
        );
    }

    #[test]
    fn gt_returns_bool() {
        // gt : Int -> Int -> Bool
        let t = prim_type("gt").unwrap();
        assert!(matches!(t, Ty::Fn(_, ret) if matches!(ret.as_ref(), Ty::Fn(_, r) if matches!(r.as_ref(), Ty::Bool))));
    }

    #[test]
    fn unknown_prim() {
        assert!(prim_type("frobnicate").is_none());
    }
}
