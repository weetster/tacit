//! Phase 1 `@name` primitive allowlist and classification.
//!
//! Three disjoint sets per ADR 0028 + ADR 0030:
//! - LIBC: external libc call (`write`, `read`, `exit`).
//! - ARITH: direct LLVM arithmetic instruction.
//! - CMP: `icmp` + `zext i1 → i64`.
//!
//! Codegen pattern-matches an `App` left-spine whose head is `Sym(name)`,
//! looks up `name` here, collects right-spine args, and emits accordingly.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimKind {
    /// libc `write(fd: i32, buf: i8*, len: i64) -> i64`
    Write,
    /// libc `read(fd: i32, buf: i8*, len: i64) -> i64`
    Read,
    /// libc `exit(status: i32) -> !`
    Exit,
    /// Binary `i64 → i64 → i64` arithmetic, lowering as a single LLVM op.
    Arith(ArithOp),
    /// Binary `i64 → i64 → i64` comparison: emits `icmp` + `zext`.
    Cmp(CmpOp),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArithOp {
    Add, // add nsw
    Sub, // sub nsw
    Mul, // mul nsw
    Div, // sdiv
    Mod, // srem
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    Eq, // icmp eq
    Ne, // icmp ne
    Lt, // icmp slt
    Le, // icmp sle
    Gt, // icmp sgt
    Ge, // icmp sge
}

impl PrimKind {
    pub fn lookup(name: &str) -> Option<PrimKind> {
        Some(match name {
            "write" => PrimKind::Write,
            "read" => PrimKind::Read,
            "exit" => PrimKind::Exit,
            "add" => PrimKind::Arith(ArithOp::Add),
            "sub" => PrimKind::Arith(ArithOp::Sub),
            "mul" => PrimKind::Arith(ArithOp::Mul),
            "div" => PrimKind::Arith(ArithOp::Div),
            "mod" => PrimKind::Arith(ArithOp::Mod),
            "eq" => PrimKind::Cmp(CmpOp::Eq),
            "ne" => PrimKind::Cmp(CmpOp::Ne),
            "lt" => PrimKind::Cmp(CmpOp::Lt),
            "le" => PrimKind::Cmp(CmpOp::Le),
            "gt" => PrimKind::Cmp(CmpOp::Gt),
            "ge" => PrimKind::Cmp(CmpOp::Ge),
            _ => return None,
        })
    }

    /// Required arity of the primitive's right-spine argument list.
    pub fn arity(self) -> usize {
        match self {
            PrimKind::Write => 3,
            PrimKind::Read => 3,
            PrimKind::Exit => 1,
            PrimKind::Arith(_) | PrimKind::Cmp(_) => 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn libc_lookup() {
        assert_eq!(PrimKind::lookup("write"), Some(PrimKind::Write));
        assert_eq!(PrimKind::lookup("read"), Some(PrimKind::Read));
        assert_eq!(PrimKind::lookup("exit"), Some(PrimKind::Exit));
    }

    #[test]
    fn arith_lookup() {
        assert!(matches!(
            PrimKind::lookup("add"),
            Some(PrimKind::Arith(ArithOp::Add))
        ));
        assert!(matches!(
            PrimKind::lookup("mod"),
            Some(PrimKind::Arith(ArithOp::Mod))
        ));
    }

    #[test]
    fn cmp_lookup() {
        assert!(matches!(
            PrimKind::lookup("lt"),
            Some(PrimKind::Cmp(CmpOp::Lt))
        ));
        assert!(matches!(
            PrimKind::lookup("eq"),
            Some(PrimKind::Cmp(CmpOp::Eq))
        ));
    }

    #[test]
    fn unknown_returns_none() {
        assert_eq!(PrimKind::lookup("frobnicate"), None);
        assert_eq!(PrimKind::lookup(""), None);
    }

    #[test]
    fn arities() {
        assert_eq!(PrimKind::Write.arity(), 3);
        assert_eq!(PrimKind::Read.arity(), 3);
        assert_eq!(PrimKind::Exit.arity(), 1);
        assert_eq!(PrimKind::Arith(ArithOp::Add).arity(), 2);
        assert_eq!(PrimKind::Cmp(CmpOp::Lt).arity(), 2);
    }
}
