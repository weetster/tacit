//! `@name` primitive allowlist and classification (ADR 0028, 0030, 0038, 0047, 0061).
//!
//! Categories per ADR 0028 + 0030 + 0038 + 0047:
//! - LIBC: external libc call (`write`, `read`, `exit`).
//! - ARITH: direct LLVM arithmetic instruction.
//! - CMP: `icmp` + `zext i1 → i64`.
//! - STACK-ALLOC: `@buf-alloc` (static) and `@buf-alloc-dyn` (runtime size).
//! - MEM: inline byte-level buffer operations (ADR 0047).
//! - PARSE: inline decimal integer parsing (ADR 0047).
//! - FORMAT: inline decimal integer formatting (ADR 0047).
//! - I64VEC-ALLOC: `@i64-alloc` (runtime i64 element count, ADR 0061).
//! - I64VEC: inline i64 vector operations (ADR 0061).
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
    /// Stack-allocate a byte buffer of N bytes (compile-time constant); returns pointer (ADR 0038).
    BufAlloc,
    /// Stack-allocate a byte buffer of N bytes (runtime i64); returns pointer (ADR 0047).
    BufAllocDyn,
    /// Load a single byte from a buffer (ADR 0047): `buf off → i64`.
    BufGet,
    /// Store a single byte into a buffer (ADR 0047): `buf off byte → i64` (returns 0).
    BufSet,
    /// Copy a byte range between buffers (ADR 0047): `dst dst-off src src-off len → i64` (returns 0).
    BufCopy,
    /// Byte-for-byte equality of two buffer regions (ADR 0047): returns 0 or 1.
    BufEq,
    /// Find the first occurrence of a target byte (ADR 0047): returns index or off+len.
    ScanByte,
    /// Inline decimal integer parse (ADR 0047): `buf off len → i64`.
    ParseI64,
    /// Inline decimal integer format (ADR 0047): `buf off val → i64` (bytes written).
    FmtI64,
    /// Stack-allocate an i64 vector (ADR 0061): `count → I64Vec`.
    I64Alloc,
    /// Load an i64 vector element (ADR 0061): `vec index → i64`.
    I64Get,
    /// Store an i64 vector element (ADR 0061): `vec index value → i64` (returns 0).
    I64Set,
    /// Swap two i64 vector elements (ADR 0061): `vec i j → i64` (returns 0).
    I64Swap,
    /// Overlap-safe i64 element copy (ADR 0061): `dst dst-index src src-index count → i64`.
    I64Copy,
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
            "buf-alloc" => PrimKind::BufAlloc,
            "buf-alloc-dyn" => PrimKind::BufAllocDyn,
            "buf-get" => PrimKind::BufGet,
            "buf-set" => PrimKind::BufSet,
            "buf-copy" => PrimKind::BufCopy,
            "buf-eq" => PrimKind::BufEq,
            "scan-byte" => PrimKind::ScanByte,
            "parse-i64" => PrimKind::ParseI64,
            "fmt-i64" => PrimKind::FmtI64,
            "i64-alloc" => PrimKind::I64Alloc,
            "i64-get" => PrimKind::I64Get,
            "i64-set" => PrimKind::I64Set,
            "i64-swap" => PrimKind::I64Swap,
            "i64-copy" => PrimKind::I64Copy,
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
            PrimKind::Write | PrimKind::Read => 3,
            PrimKind::Exit | PrimKind::BufAlloc | PrimKind::BufAllocDyn | PrimKind::I64Alloc => 1,
            PrimKind::BufGet | PrimKind::I64Get => 2,
            PrimKind::BufSet
            | PrimKind::ParseI64
            | PrimKind::FmtI64
            | PrimKind::I64Set
            | PrimKind::I64Swap => 3,
            PrimKind::ScanByte => 4,
            PrimKind::BufCopy | PrimKind::BufEq | PrimKind::I64Copy => 5,
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
    fn p3_primitives_lookup() {
        assert_eq!(
            PrimKind::lookup("buf-alloc-dyn"),
            Some(PrimKind::BufAllocDyn)
        );
        assert_eq!(PrimKind::lookup("buf-get"), Some(PrimKind::BufGet));
        assert_eq!(PrimKind::lookup("buf-set"), Some(PrimKind::BufSet));
        assert_eq!(PrimKind::lookup("buf-copy"), Some(PrimKind::BufCopy));
        assert_eq!(PrimKind::lookup("buf-eq"), Some(PrimKind::BufEq));
        assert_eq!(PrimKind::lookup("scan-byte"), Some(PrimKind::ScanByte));
        assert_eq!(PrimKind::lookup("parse-i64"), Some(PrimKind::ParseI64));
        assert_eq!(PrimKind::lookup("fmt-i64"), Some(PrimKind::FmtI64));
        assert_eq!(PrimKind::lookup("i64-alloc"), Some(PrimKind::I64Alloc));
        assert_eq!(PrimKind::lookup("i64-get"), Some(PrimKind::I64Get));
        assert_eq!(PrimKind::lookup("i64-set"), Some(PrimKind::I64Set));
        assert_eq!(PrimKind::lookup("i64-swap"), Some(PrimKind::I64Swap));
        assert_eq!(PrimKind::lookup("i64-copy"), Some(PrimKind::I64Copy));
    }

    #[test]
    fn arities() {
        assert_eq!(PrimKind::Write.arity(), 3);
        assert_eq!(PrimKind::Read.arity(), 3);
        assert_eq!(PrimKind::Exit.arity(), 1);
        assert_eq!(PrimKind::BufAllocDyn.arity(), 1);
        assert_eq!(PrimKind::BufGet.arity(), 2);
        assert_eq!(PrimKind::BufSet.arity(), 3);
        assert_eq!(PrimKind::BufCopy.arity(), 5);
        assert_eq!(PrimKind::BufEq.arity(), 5);
        assert_eq!(PrimKind::ScanByte.arity(), 4);
        assert_eq!(PrimKind::ParseI64.arity(), 3);
        assert_eq!(PrimKind::FmtI64.arity(), 3);
        assert_eq!(PrimKind::I64Alloc.arity(), 1);
        assert_eq!(PrimKind::I64Get.arity(), 2);
        assert_eq!(PrimKind::I64Set.arity(), 3);
        assert_eq!(PrimKind::I64Swap.arity(), 3);
        assert_eq!(PrimKind::I64Copy.arity(), 5);
        assert_eq!(PrimKind::Arith(ArithOp::Add).arity(), 2);
        assert_eq!(PrimKind::Cmp(CmpOp::Lt).arity(), 2);
    }
}
