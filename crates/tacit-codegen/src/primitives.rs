//! `@name` primitive allowlist and classification (ADR 0028, 0030, 0038, 0047, 0061, 0062, 0063, 0064, 0067, 0068, 0069, 0074, 0084).
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
//! - TEXT-INDEX: inline text boundary indexing into I64Vec range tables (ADR 0062).
//! - TEXT-INDEX-ANY: multi-delimiter token indexing (ADR 0063).
//! - RANGE-TABLE: I64Vec start/length pair accessors (ADR 0062).
//! - ORDER: inline ordering operations over I64Vec and range tables.
//! - SEARCH: inline binary search over sorted I64Vec prefixes (ADR 0064).
//! - RANGE-GROUP: inline adjacent range grouping helpers (ADR 0064).
//! - STREAM-IO: full-stream `read`/`write` framing wrappers (ADR 0067).
//! - BUF-MUT: in-place byte-range mutation helpers (ADR 0067).
//! - ASCII-CASE: pure ASCII case shifts (ADR 0068).
//! - ASCII-CLASS: pure ASCII character classification predicates (ADR 0068).
//! - UTF8: codepoint decode/encode/length helpers (ADR 0069).
//! - COMBINATOR: higher-order I64Vec traversal forms (ADR 0074).
//! - FIXED-INT: fixed-width casts, arithmetic, bits, shifts, masks, and
//!   byte-order helpers (ADR 0084).
//!
//! Codegen pattern-matches an `App` left-spine whose head is `Sym(name)`,
//! looks up `name` here, collects right-spine args, and emits accordingly.

use tacit_typecheck::primitives::{
    parse_fixed_prim, parse_vec_prim, FixedPrim, U8VecBusOp, U8VecOp, VecOp, VecPrim,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimKind {
    /// Fixed-width integer primitive (ADR 0084).
    Fixed(FixedPrim),
    /// Typed mutable memory primitive (ADR 0085).
    Vec(VecPrim),
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
    /// Index LF-delimited line ranges into an I64Vec pair table (ADR 0062).
    LineIndex,
    /// Index delimiter-separated token ranges into an I64Vec pair table (ADR 0062).
    TokenIndex,
    /// Index token ranges separated by any byte from a delimiter buffer (ADR 0063).
    TokenIndexAny,
    /// Load the start field for a range-table row (ADR 0062).
    RangeStart,
    /// Load the length field for a range-table row (ADR 0062).
    RangeLen,
    /// Sort an i64 vector prefix ascending in place.
    SortI64,
    /// Sort range-table rows lexicographically by the bytes they reference.
    SortRangesByBytes,
    /// Stable-sort i64 keys and apply the same permutation to i64 values.
    StableSortPairsI64,
    /// Find the lower-bound insertion index in a sorted i64 vector prefix.
    LowerBoundI64,
    /// Count adjacent equal byte ranges into `(start, len, count)` triples.
    CountEqualRanges,
    /// Deduplicate adjacent equal byte ranges into start/length pairs.
    DedupAdjacentRanges,
    /// Read fd 0 until EOF or `cap` bytes (ADR 0067): `buf cap → i64`.
    StdinSlurp,
    /// Write a byte-range slice to a file descriptor (ADR 0067):
    /// `fd buf off len → i64` (returns 0).
    WriteRange,
    /// Reverse a byte-range in place (ADR 0067): `buf off len → i64` (returns 0).
    BufRev,
    /// ASCII case shift: lowercase letter → uppercase, identity otherwise (ADR 0068).
    AsciiToupper,
    /// ASCII case shift: uppercase letter → lowercase, identity otherwise (ADR 0068).
    AsciiTolower,
    /// ASCII classification: 1 if A-Z or a-z, else 0 (ADR 0068).
    AsciiIsAlpha,
    /// ASCII classification: 1 if 0-9, else 0 (ADR 0068).
    AsciiIsDigit,
    /// ASCII classification: 1 if TAB, LF, VT, FF, CR, or SP, else 0 (ADR 0068).
    AsciiIsSpace,
    /// UTF-8 decode one codepoint (ADR 0069): `buf off → i64`,
    /// returns packed `cp * 8 + byte_len` or 0 on invalid.
    Utf8Decode,
    /// UTF-8 encode one codepoint (ADR 0069): `buf off cp → i64`,
    /// returns bytes written (1..=4) or 0 if `cp` is invalid.
    Utf8Encode,
    /// UTF-8 byte length of a codepoint (ADR 0069): `cp → i64` in 1..=4 or 0 on invalid.
    Utf8Len,
    /// I64Vec map (ADR 0074): `src count callback out -> i64`.
    Map,
    /// I64Vec fold (ADR 0074): `src count init callback -> i64`.
    Fold,
    /// I64Vec for-each (ADR 0074): `src count callback -> i64`.
    ForEach,
    /// Bounded-stack iteration (ADR 0093): `@loop init step -> S`.
    Loop,
    /// Loop continuation directive (ADR 0093): `@step value -> {tag,value}`.
    LoopStep,
    /// Loop termination directive (ADR 0093): `@exit value -> {tag,value}`.
    LoopExit,
    /// Load a field from the current package instance (ADR 0094).
    StateLoad,
    /// Store a scalar/record field into the current package instance.
    StateStore,
    /// Allocate an instance-owned typed-vector field.
    StateAllocVec,
    /// Free an instance-owned typed-vector field.
    StateFreeVec,
    /// Borrow a u8 sub-slice from an instance-owned vector field.
    StateSlice,
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
        if let Some(prim) = parse_vec_prim(name) {
            return Some(PrimKind::Vec(prim));
        }
        if let Some(prim) = parse_fixed_prim(name) {
            return Some(PrimKind::Fixed(prim));
        }

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
            "line-index" => PrimKind::LineIndex,
            "token-index" => PrimKind::TokenIndex,
            "token-index-any" => PrimKind::TokenIndexAny,
            "range-start" => PrimKind::RangeStart,
            "range-len" => PrimKind::RangeLen,
            "sort-i64" => PrimKind::SortI64,
            "sort-ranges-by-bytes" => PrimKind::SortRangesByBytes,
            "stable-sort-pairs-i64" => PrimKind::StableSortPairsI64,
            "lower-bound-i64" => PrimKind::LowerBoundI64,
            "count-equal-ranges" => PrimKind::CountEqualRanges,
            "dedup-adjacent-ranges" => PrimKind::DedupAdjacentRanges,
            "stdin-slurp" => PrimKind::StdinSlurp,
            "write-range" => PrimKind::WriteRange,
            "buf-rev" => PrimKind::BufRev,
            "ascii-tolower" => PrimKind::AsciiTolower,
            "ascii-toupper" => PrimKind::AsciiToupper,
            "ascii-is-alpha" => PrimKind::AsciiIsAlpha,
            "ascii-is-digit" => PrimKind::AsciiIsDigit,
            "ascii-is-space" => PrimKind::AsciiIsSpace,
            "utf8-decode" => PrimKind::Utf8Decode,
            "utf8-encode" => PrimKind::Utf8Encode,
            "utf8-len" => PrimKind::Utf8Len,
            "map" => PrimKind::Map,
            "fold" => PrimKind::Fold,
            "for-each" => PrimKind::ForEach,
            "loop" => PrimKind::Loop,
            "loop-step" => PrimKind::LoopStep,
            "loop-exit" => PrimKind::LoopExit,
            "state-load" => PrimKind::StateLoad,
            "state-store" => PrimKind::StateStore,
            "state-alloc-vec" => PrimKind::StateAllocVec,
            "state-free-vec" => PrimKind::StateFreeVec,
            "state-slice" => PrimKind::StateSlice,
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
            PrimKind::Fixed(prim) => fixed_prim_arity(prim),
            PrimKind::Vec(prim) => vec_prim_arity(prim),
            PrimKind::Write | PrimKind::Read => 3,
            PrimKind::Exit
            | PrimKind::BufAlloc
            | PrimKind::BufAllocDyn
            | PrimKind::I64Alloc
            | PrimKind::AsciiTolower
            | PrimKind::AsciiToupper
            | PrimKind::AsciiIsAlpha
            | PrimKind::AsciiIsDigit
            | PrimKind::AsciiIsSpace
            | PrimKind::Utf8Len => 1,
            PrimKind::BufGet | PrimKind::I64Get | PrimKind::RangeStart | PrimKind::RangeLen => 2,
            PrimKind::SortI64 | PrimKind::StdinSlurp | PrimKind::Utf8Decode => 2,
            PrimKind::BufSet
            | PrimKind::ParseI64
            | PrimKind::FmtI64
            | PrimKind::I64Set
            | PrimKind::I64Swap
            | PrimKind::LineIndex
            | PrimKind::SortRangesByBytes
            | PrimKind::StableSortPairsI64
            | PrimKind::LowerBoundI64
            | PrimKind::BufRev
            | PrimKind::Utf8Encode => 3,
            PrimKind::ScanByte
            | PrimKind::CountEqualRanges
            | PrimKind::DedupAdjacentRanges
            | PrimKind::WriteRange
            | PrimKind::Map
            | PrimKind::Fold => 4,
            PrimKind::BufCopy | PrimKind::BufEq | PrimKind::I64Copy | PrimKind::TokenIndex => 5,
            PrimKind::TokenIndexAny => 6,
            PrimKind::ForEach => 3,
            PrimKind::Loop => 2,
            PrimKind::LoopStep | PrimKind::LoopExit => 1,
            PrimKind::StateLoad | PrimKind::StateFreeVec => 1,
            PrimKind::StateStore | PrimKind::StateAllocVec => 2,
            PrimKind::StateSlice => 3,
            PrimKind::Arith(_) | PrimKind::Cmp(_) => 2,
        }
    }
}

fn vec_prim_arity(prim: VecPrim) -> usize {
    match prim {
        VecPrim::Vec { op, .. } => match op {
            VecOp::Alloc => 1,
            VecOp::Len => 1,
            VecOp::Get => 2,
            VecOp::Set => 3,
        },
        VecPrim::U8Vec(op) => match op {
            U8VecOp::Slice => 3,
            U8VecOp::Scan => 4,
            U8VecOp::Fill => 4,
            U8VecOp::Eq => 5,
            U8VecOp::Copy => 5,
        },
        VecPrim::U8VecBus(op) => match op {
            U8VecBusOp::Load { .. } => 2,
            U8VecBusOp::Store { .. } => 3,
        },
    }
}

fn fixed_prim_arity(prim: FixedPrim) -> usize {
    match prim {
        FixedPrim::FromIntWrap { .. } | FixedPrim::Cast { .. } => 1,
        FixedPrim::Arith { .. } => 2,
        FixedPrim::Bit { op, .. } => match op {
            tacit_typecheck::primitives::FixedBitOp::Not => 1,
            tacit_typecheck::primitives::FixedBitOp::And
            | tacit_typecheck::primitives::FixedBitOp::Or
            | tacit_typecheck::primitives::FixedBitOp::Xor => 2,
        },
        FixedPrim::Shift { .. } => 2,
        FixedPrim::MaskLow { .. } => 1,
        FixedPrim::Bytes { ty, .. } => (ty.width / 8) as usize,
        FixedPrim::ByteSwap { .. } => 1,
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
        assert_eq!(PrimKind::lookup("line-index"), Some(PrimKind::LineIndex));
        assert_eq!(PrimKind::lookup("token-index"), Some(PrimKind::TokenIndex));
        assert_eq!(
            PrimKind::lookup("token-index-any"),
            Some(PrimKind::TokenIndexAny)
        );
        assert_eq!(PrimKind::lookup("range-start"), Some(PrimKind::RangeStart));
        assert_eq!(PrimKind::lookup("range-len"), Some(PrimKind::RangeLen));
        assert_eq!(PrimKind::lookup("sort-i64"), Some(PrimKind::SortI64));
        assert_eq!(
            PrimKind::lookup("sort-ranges-by-bytes"),
            Some(PrimKind::SortRangesByBytes)
        );
        assert_eq!(
            PrimKind::lookup("stable-sort-pairs-i64"),
            Some(PrimKind::StableSortPairsI64)
        );
        assert_eq!(
            PrimKind::lookup("lower-bound-i64"),
            Some(PrimKind::LowerBoundI64)
        );
        assert_eq!(
            PrimKind::lookup("count-equal-ranges"),
            Some(PrimKind::CountEqualRanges)
        );
        assert_eq!(
            PrimKind::lookup("dedup-adjacent-ranges"),
            Some(PrimKind::DedupAdjacentRanges)
        );
        assert_eq!(PrimKind::lookup("stdin-slurp"), Some(PrimKind::StdinSlurp));
        assert_eq!(PrimKind::lookup("write-range"), Some(PrimKind::WriteRange));
        assert_eq!(PrimKind::lookup("buf-rev"), Some(PrimKind::BufRev));
        assert_eq!(
            PrimKind::lookup("ascii-tolower"),
            Some(PrimKind::AsciiTolower)
        );
        assert_eq!(
            PrimKind::lookup("ascii-toupper"),
            Some(PrimKind::AsciiToupper)
        );
        assert_eq!(
            PrimKind::lookup("ascii-is-alpha"),
            Some(PrimKind::AsciiIsAlpha)
        );
        assert_eq!(
            PrimKind::lookup("ascii-is-digit"),
            Some(PrimKind::AsciiIsDigit)
        );
        assert_eq!(
            PrimKind::lookup("ascii-is-space"),
            Some(PrimKind::AsciiIsSpace)
        );
        assert_eq!(PrimKind::lookup("utf8-decode"), Some(PrimKind::Utf8Decode));
        assert_eq!(PrimKind::lookup("utf8-encode"), Some(PrimKind::Utf8Encode));
        assert_eq!(PrimKind::lookup("utf8-len"), Some(PrimKind::Utf8Len));
        assert_eq!(PrimKind::lookup("map"), Some(PrimKind::Map));
        assert_eq!(PrimKind::lookup("fold"), Some(PrimKind::Fold));
        assert_eq!(PrimKind::lookup("for-each"), Some(PrimKind::ForEach));
        assert_eq!(PrimKind::lookup("loop"), Some(PrimKind::Loop));
        assert_eq!(PrimKind::lookup("loop-step"), Some(PrimKind::LoopStep));
        assert_eq!(PrimKind::lookup("loop-exit"), Some(PrimKind::LoopExit));
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
        assert_eq!(PrimKind::LineIndex.arity(), 3);
        assert_eq!(PrimKind::TokenIndex.arity(), 5);
        assert_eq!(PrimKind::TokenIndexAny.arity(), 6);
        assert_eq!(PrimKind::RangeStart.arity(), 2);
        assert_eq!(PrimKind::RangeLen.arity(), 2);
        assert_eq!(PrimKind::SortI64.arity(), 2);
        assert_eq!(PrimKind::SortRangesByBytes.arity(), 3);
        assert_eq!(PrimKind::StableSortPairsI64.arity(), 3);
        assert_eq!(PrimKind::LowerBoundI64.arity(), 3);
        assert_eq!(PrimKind::CountEqualRanges.arity(), 4);
        assert_eq!(PrimKind::DedupAdjacentRanges.arity(), 4);
        assert_eq!(PrimKind::StdinSlurp.arity(), 2);
        assert_eq!(PrimKind::WriteRange.arity(), 4);
        assert_eq!(PrimKind::BufRev.arity(), 3);
        assert_eq!(PrimKind::AsciiTolower.arity(), 1);
        assert_eq!(PrimKind::AsciiToupper.arity(), 1);
        assert_eq!(PrimKind::AsciiIsAlpha.arity(), 1);
        assert_eq!(PrimKind::AsciiIsDigit.arity(), 1);
        assert_eq!(PrimKind::AsciiIsSpace.arity(), 1);
        assert_eq!(PrimKind::Utf8Decode.arity(), 2);
        assert_eq!(PrimKind::Utf8Encode.arity(), 3);
        assert_eq!(PrimKind::Utf8Len.arity(), 1);
        assert_eq!(PrimKind::Map.arity(), 4);
        assert_eq!(PrimKind::Fold.arity(), 4);
        assert_eq!(PrimKind::ForEach.arity(), 3);
        assert_eq!(PrimKind::Loop.arity(), 2);
        assert_eq!(PrimKind::LoopStep.arity(), 1);
        assert_eq!(PrimKind::LoopExit.arity(), 1);
        assert_eq!(PrimKind::Arith(ArithOp::Add).arity(), 2);
        assert_eq!(PrimKind::Cmp(CmpOp::Lt).arity(), 2);
    }
}
