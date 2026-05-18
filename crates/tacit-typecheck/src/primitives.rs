//! Type and effect signatures for built-in @name primitives (ADR 0028, 0030, 0042, 0047, 0061, 0062, 0063, 0064, 0067, 0068, 0069, 0074, 0084).
//!
//! Effect sets mirror `stdlib/libc-effects.toml` (ADR 0025, consumed in Stage 3).
//! The canonical source for primitive effects is that TOML file; values here match it.
//! Phase 3 additions (ADR 0047): PARSE, FORMAT, MEM categories, STACK-ALLOC extension.
//! Library-mediated additions:
//! - ADR 0061: I64Vec allocation and element operations.
//! - ADR 0062: text indexing into I64Vec range tables.
//! - ADR 0063: multi-delimiter token indexing.
//! - Bundle C: ordering operations over I64Vec and range tables.
//! - ADR 0064: search and adjacent range grouping helpers.
//! - ADR 0067: Bundle E stream IO sugar (`stdin-slurp`, `write-range`, `buf-rev`).
//! - ADR 0068: Bundle F ASCII case (`ascii-tolower`, `ascii-toupper`) and
//!   classification (`ascii-is-alpha`, `ascii-is-digit`, `ascii-is-space`).
//! - ADR 0069: Bundle G UTF-8 codepoint primitives (`utf8-decode`,
//!   `utf8-encode`, `utf8-len`).
//! - ADR 0074: Phase 4 higher-order combinators (`map`, `fold`, `for-each`).
//! - ADR 0084: fixed-width integer casts, arithmetic, bit operations, shifts,
//!   masks, and byte-order helpers.

use std::collections::BTreeMap;

use crate::ty::{EffAtom, EffSet, FixedIntTy, FnEff, IntSign, Ty};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedCastKind {
    Trunc,
    SignExtend,
    ZeroExtend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedArithOp {
    Add,
    Sub,
    Mul,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedArithMode {
    Wrap,
    Check,
    Saturate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedBitOp {
    And,
    Or,
    Xor,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedShiftOp {
    Shl,
    Shr,
    Rotl,
    Rotr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedEndian {
    Big,
    Little,
}

/// Operation kinds for the uniform per-width typed-vector primitives
/// (ADR 0085). Every typed vector exposes `alloc`, `len`, `get`, `set`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VecOp {
    Alloc,
    Len,
    Get,
    Set,
}

/// `u8vec`-only byte-buffer extras (ADR 0085).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum U8VecOp {
    Fill,
    Copy,
    Slice,
    Eq,
    Scan,
}

/// `u8vec` byte-bus cross-width helpers (ADR 0085). `ty` is one of
/// `u16`, `u32`, `u64`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum U8VecBusOp {
    Load { ty: FixedIntTy, endian: FixedEndian },
    Store { ty: FixedIntTy, endian: FixedEndian },
}

/// Stage 7 typed mutable memory primitive (ADR 0085).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VecPrim {
    /// Uniform per-width op.
    Vec { ty: FixedIntTy, op: VecOp },
    /// `u8vec`-only extras.
    U8Vec(U8VecOp),
    /// `u8vec` byte-bus typed load/store.
    U8VecBus(U8VecBusOp),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedPrim {
    FromIntWrap {
        dst: FixedIntTy,
    },
    Cast {
        src: FixedIntTy,
        dst: FixedIntTy,
        kind: FixedCastKind,
    },
    Arith {
        ty: FixedIntTy,
        op: FixedArithOp,
        mode: FixedArithMode,
    },
    Bit {
        ty: FixedIntTy,
        op: FixedBitOp,
    },
    Shift {
        ty: FixedIntTy,
        op: FixedShiftOp,
    },
    MaskLow {
        ty: FixedIntTy,
    },
    Bytes {
        ty: FixedIntTy,
        endian: FixedEndian,
    },
    ByteSwap {
        ty: FixedIntTy,
    },
}

/// Look up the type of a `@name` primitive by name.
/// Returns `None` if the name is not a known primitive.
///
/// IO effect sits at the innermost (final) application: partial applications
/// are pure closures; IO is produced only when all args are supplied.
pub fn prim_type(name: &str) -> Option<Ty> {
    if let Some(prim) = parse_vec_prim(name) {
        return Some(vec_prim_type(prim));
    }
    if let Some(prim) = parse_fixed_prim(name) {
        return Some(fixed_prim_type(prim));
    }

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
        // ORDER: sort-i64(vec: I64Vec, count: Int) -> Int / {Mut}
        "sort-i64" => fn2_mut(Ty::I64Vec, Ty::Int, Ty::Int),
        // ORDER: sort-ranges-by-bytes(text: Buf, table: I64Vec, count: Int) -> Int / {Mut}
        "sort-ranges-by-bytes" => fn3_mut(Ty::Buf, Ty::I64Vec, Ty::Int, Ty::Int),
        // ORDER: stable-sort-pairs-i64(keys: I64Vec, values: I64Vec, count: Int) -> Int / {Mut}
        "stable-sort-pairs-i64" => fn3_mut(Ty::I64Vec, Ty::I64Vec, Ty::Int, Ty::Int),
        // SEARCH: lower-bound-i64(vec: I64Vec, count: Int, value: Int) -> Int (pure)
        "lower-bound-i64" => fn3_pure(Ty::I64Vec, Ty::Int, Ty::Int, Ty::Int),
        // RANGE-GROUP: count-equal-ranges(text: Buf, table: I64Vec, count: Int, out: I64Vec) -> Int / {Mut}
        "count-equal-ranges" => fn4_mut(Ty::Buf, Ty::I64Vec, Ty::Int, Ty::I64Vec, Ty::Int),
        // RANGE-GROUP: dedup-adjacent-ranges(text: Buf, table: I64Vec, count: Int, out: I64Vec) -> Int / {Mut}
        "dedup-adjacent-ranges" => fn4_mut(Ty::Buf, Ty::I64Vec, Ty::Int, Ty::I64Vec, Ty::Int),
        // STREAM-IO: stdin-slurp(buf: Buf, cap: Int) -> Int / {IO, Mut} (ADR 0067)
        "stdin-slurp" => fn2_mut_io(Ty::Buf, Ty::Int, Ty::Int),
        // STREAM-IO: write-range(fd: Int, buf: Buf, off: Int, len: Int) -> Int / {IO} (ADR 0067)
        "write-range" => fn4_io(Ty::Int, Ty::Buf, Ty::Int, Ty::Int, Ty::Int),
        // BUF-MUT: buf-rev(buf: Buf, off: Int, len: Int) -> Int / {Mut} (ADR 0067)
        "buf-rev" => fn3_mut(Ty::Buf, Ty::Int, Ty::Int, Ty::Int),
        // ASCII-CASE: ascii-tolower(b: Int) -> Int (pure, ADR 0068)
        "ascii-tolower" => fn1_pure(Ty::Int, Ty::Int),
        // ASCII-CASE: ascii-toupper(b: Int) -> Int (pure, ADR 0068)
        "ascii-toupper" => fn1_pure(Ty::Int, Ty::Int),
        // ASCII-CLASS: ascii-is-alpha(b: Int) -> Int (pure, ADR 0068)
        "ascii-is-alpha" => fn1_pure(Ty::Int, Ty::Int),
        // ASCII-CLASS: ascii-is-digit(b: Int) -> Int (pure, ADR 0068)
        "ascii-is-digit" => fn1_pure(Ty::Int, Ty::Int),
        // ASCII-CLASS: ascii-is-space(b: Int) -> Int (pure, ADR 0068)
        "ascii-is-space" => fn1_pure(Ty::Int, Ty::Int),
        // UTF8: utf8-decode(buf: Buf, off: Int) -> Int (pure, ADR 0069)
        "utf8-decode" => fn2_pure(Ty::Buf, Ty::Int, Ty::Int),
        // UTF8: utf8-encode(buf: Buf, off: Int, cp: Int) -> Int / {Mut} (ADR 0069)
        "utf8-encode" => fn3_mut(Ty::Buf, Ty::Int, Ty::Int, Ty::Int),
        // UTF8: utf8-len(cp: Int) -> Int (pure, ADR 0069)
        "utf8-len" => fn1_pure(Ty::Int, Ty::Int),
        // P4 COMBINATORS: full applications are inferred specially in
        // infer.rs so callback effects can be propagated precisely.
        "map" => map_i64_ty(),
        "fold" => fold_i64_ty(),
        "for-each" => for_each_i64_ty(),
        // SHB STAGE 2 (ADR 0093): @loop is inferred specially in infer.rs to
        // unify state types and propagate the step-callback effect.  Partial
        // applications produce closures whose final result is the loop's
        // state value; the standalone type here is a placeholder used by
        // partial-application typing only.
        "loop" => loop_ty(),
        "loop-step" => loop_directive_ty(),
        "loop-exit" => loop_directive_ty(),
        // ARITH: Int → Int → Int (pure)
        "add" | "sub" | "mul" | "div" | "mod" => fn2_pure(Ty::Int, Ty::Int, Ty::Int),
        // CMP: Int → Int → Bool (pure, ADR 0042)
        "eq" | "ne" | "lt" | "le" | "gt" | "ge" => fn2_pure(Ty::Int, Ty::Int, Ty::Bool),
        _ => return None,
    })
}

pub fn parse_fixed_prim(name: &str) -> Option<FixedPrim> {
    let parts: Vec<&str> = name.split('-').collect();
    if parts.len() == 4 && parts[1] == "from" && parts[2] == "int" && parts[3] == "wrap" {
        return Some(FixedPrim::FromIntWrap {
            dst: FixedIntTy::parse_name(parts[0])?,
        });
    }

    if parts.len() == 4 && parts[1] == "to" {
        let src = FixedIntTy::parse_name(parts[0])?;
        let dst = FixedIntTy::parse_name(parts[2])?;
        let kind = match parts[3] {
            "trunc" => FixedCastKind::Trunc,
            "sext" => FixedCastKind::SignExtend,
            "zext" => FixedCastKind::ZeroExtend,
            _ => return None,
        };
        if valid_cast(src, dst, kind) {
            return Some(FixedPrim::Cast { src, dst, kind });
        }
        return None;
    }

    if parts.len() == 3 {
        if let Some(ty) = FixedIntTy::parse_name(parts[0]) {
            let op = match parts[1] {
                "add" => Some(FixedArithOp::Add),
                "sub" => Some(FixedArithOp::Sub),
                "mul" => Some(FixedArithOp::Mul),
                _ => None,
            };
            if let Some(op) = op {
                let mode = match parts[2] {
                    "wrap" => FixedArithMode::Wrap,
                    "check" if op != FixedArithOp::Mul => FixedArithMode::Check,
                    "sat" if op != FixedArithOp::Mul => FixedArithMode::Saturate,
                    _ => return None,
                };
                return Some(FixedPrim::Arith { ty, op, mode });
            }
        }
    }

    if parts.len() == 2 {
        if let Some(ty) = FixedIntTy::parse_name(parts[0]) {
            let op = match parts[1] {
                "and" => Some(FixedBitOp::And),
                "or" => Some(FixedBitOp::Or),
                "xor" => Some(FixedBitOp::Xor),
                "not" => Some(FixedBitOp::Not),
                _ => None,
            };
            if let Some(op) = op {
                return Some(FixedPrim::Bit { ty, op });
            }
            let op = match parts[1] {
                "shl" => Some(FixedShiftOp::Shl),
                "shr" => Some(FixedShiftOp::Shr),
                "rotl" => Some(FixedShiftOp::Rotl),
                "rotr" => Some(FixedShiftOp::Rotr),
                _ => None,
            };
            if let Some(op) = op {
                return Some(FixedPrim::Shift { ty, op });
            }
            if parts[1] == "bswap" && ty.sign == crate::ty::IntSign::Unsigned && ty.width > 8 {
                return Some(FixedPrim::ByteSwap { ty });
            }
        }
    }

    if parts.len() == 3 {
        if let Some(ty) = FixedIntTy::parse_name(parts[0]) {
            if parts[1] == "mask" && parts[2] == "low" {
                return Some(FixedPrim::MaskLow { ty });
            }
            if parts[1] == "from"
                && ty.sign == crate::ty::IntSign::Unsigned
                && matches!(ty.width, 16 | 32 | 64)
            {
                let endian = match parts[2] {
                    "be" => FixedEndian::Big,
                    "le" => FixedEndian::Little,
                    _ => return None,
                };
                return Some(FixedPrim::Bytes { ty, endian });
            }
        }
    }

    None
}

fn valid_cast(src: FixedIntTy, dst: FixedIntTy, kind: FixedCastKind) -> bool {
    match kind {
        FixedCastKind::Trunc => src.width > dst.width,
        FixedCastKind::SignExtend => src.sign.is_signed() && dst.width > src.width,
        FixedCastKind::ZeroExtend => dst.width > src.width,
    }
}

pub fn fixed_prim_type(prim: FixedPrim) -> Ty {
    match prim {
        FixedPrim::FromIntWrap { dst } => fn1_pure(Ty::Int, Ty::FixedInt(dst)),
        FixedPrim::Cast { src, dst, .. } => fn1_pure(Ty::FixedInt(src), Ty::FixedInt(dst)),
        FixedPrim::Arith { ty, mode, .. } => {
            let arg = Ty::FixedInt(ty);
            let ret = match mode {
                FixedArithMode::Wrap | FixedArithMode::Saturate => Ty::FixedInt(ty),
                FixedArithMode::Check => checked_result_ty(ty),
            };
            fn2_pure(arg.clone(), arg, ret)
        }
        FixedPrim::Bit { ty, op } => {
            let arg = Ty::FixedInt(ty);
            match op {
                FixedBitOp::Not => fn1_pure(arg.clone(), arg),
                FixedBitOp::And | FixedBitOp::Or | FixedBitOp::Xor => {
                    fn2_pure(arg.clone(), arg.clone(), arg)
                }
            }
        }
        FixedPrim::Shift { ty, .. } => fn2_pure(Ty::FixedInt(ty), Ty::Int, Ty::FixedInt(ty)),
        FixedPrim::MaskLow { ty } => fn1_pure(Ty::Int, Ty::FixedInt(ty)),
        FixedPrim::Bytes { ty, .. } => fixed_fn(
            &vec![
                Ty::FixedInt(FixedIntTy::new(crate::ty::IntSign::Unsigned, 8));
                (ty.width / 8) as usize
            ],
            Ty::FixedInt(ty),
        ),
        FixedPrim::ByteSwap { ty } => fn1_pure(Ty::FixedInt(ty), Ty::FixedInt(ty)),
    }
}

/// Parse a Stage 7 typed-vector primitive name.
///
/// Accepts:
/// - `<intty>vec-alloc` | `-len` | `-get` | `-set` for the eight widths,
/// - `u8vec-fill` | `-copy` | `-slice` | `-eq` | `-scan`,
/// - `u8vec-load-<utype>-<le|be>` and `u8vec-store-<utype>-<le|be>`
///   for `u16`, `u32`, `u64`.
pub fn parse_vec_prim(name: &str) -> Option<VecPrim> {
    let dash = name.find('-')?;
    let (head, tail) = name.split_at(dash);
    let rest = &tail[1..]; // skip the '-'

    if !head.ends_with("vec") {
        return None;
    }
    let int_name = &head[..head.len() - 3];
    let ty = FixedIntTy::parse_name(int_name)?;

    // Uniform per-width ops.
    let op = match rest {
        "alloc" => Some(VecOp::Alloc),
        "len" => Some(VecOp::Len),
        "get" => Some(VecOp::Get),
        "set" => Some(VecOp::Set),
        _ => None,
    };
    if let Some(op) = op {
        return Some(VecPrim::Vec { ty, op });
    }

    // u8vec-specific extras and byte-bus helpers.
    if head != "u8vec" {
        return None;
    }
    match rest {
        "fill" => return Some(VecPrim::U8Vec(U8VecOp::Fill)),
        "copy" => return Some(VecPrim::U8Vec(U8VecOp::Copy)),
        "slice" => return Some(VecPrim::U8Vec(U8VecOp::Slice)),
        "eq" => return Some(VecPrim::U8Vec(U8VecOp::Eq)),
        "scan" => return Some(VecPrim::U8Vec(U8VecOp::Scan)),
        _ => {}
    }

    // u8vec-load-<utype>-<endian>, u8vec-store-<utype>-<endian>.
    let parts: Vec<&str> = rest.split('-').collect();
    if parts.len() == 3 {
        let dir = parts[0];
        let bus_ty = FixedIntTy::parse_name(parts[1])?;
        if bus_ty.sign != IntSign::Unsigned || !matches!(bus_ty.width, 16 | 32 | 64) {
            return None;
        }
        let endian = match parts[2] {
            "le" => FixedEndian::Little,
            "be" => FixedEndian::Big,
            _ => return None,
        };
        match dir {
            "load" => return Some(VecPrim::U8VecBus(U8VecBusOp::Load { ty: bus_ty, endian })),
            "store" => return Some(VecPrim::U8VecBus(U8VecBusOp::Store { ty: bus_ty, endian })),
            _ => return None,
        }
    }

    None
}

/// Type of a Stage 7 typed-vector primitive (ADR 0085).
pub fn vec_prim_type(prim: VecPrim) -> Ty {
    match prim {
        VecPrim::Vec { ty, op } => {
            let vec_ty = Ty::Vec(ty);
            let elem_ty = Ty::FixedInt(ty);
            match op {
                // alloc(count: Int) -> <ty>vec / {Alloc}
                VecOp::Alloc => Ty::Fn(Box::new(Ty::Int), Box::new(vec_ty), alloc_eff()),
                // len(v: <ty>vec) -> Int  (pure)
                VecOp::Len => Ty::Fn(Box::new(vec_ty), Box::new(Ty::Int), FnEff::pure_()),
                // get(v, i) -> <ty>  (pure)
                VecOp::Get => fn2_pure(vec_ty, Ty::Int, elem_ty),
                // set(v, i, x) -> Int / {Mut}
                VecOp::Set => fn3_mut(vec_ty, Ty::Int, elem_ty, Ty::Int),
            }
        }
        VecPrim::U8Vec(op) => {
            let u8vec = || Ty::Vec(FixedIntTy::new(IntSign::Unsigned, 8));
            let u8 = || Ty::FixedInt(FixedIntTy::new(IntSign::Unsigned, 8));
            match op {
                // fill(v, off, len, byte) -> Int / {Mut}
                U8VecOp::Fill => fn4_mut(u8vec(), Ty::Int, Ty::Int, u8(), Ty::Int),
                // copy(dst, dst-off, src, src-off, len) -> Int / {Mut}
                U8VecOp::Copy => fn5_mut(u8vec(), Ty::Int, u8vec(), Ty::Int, Ty::Int, Ty::Int),
                // slice(v, off, len) -> u8vec  (pure; aliasing sub-view)
                U8VecOp::Slice => fn3_pure(u8vec(), Ty::Int, Ty::Int, u8vec()),
                // eq(a, a-off, b, b-off, len) -> Bool  (pure)
                U8VecOp::Eq => fn5_pure(u8vec(), Ty::Int, u8vec(), Ty::Int, Ty::Int, Ty::Bool),
                // scan(v, off, len, byte) -> Int  (pure)
                U8VecOp::Scan => fn4_pure(u8vec(), Ty::Int, Ty::Int, u8(), Ty::Int),
            }
        }
        VecPrim::U8VecBus(op) => {
            let u8vec = || Ty::Vec(FixedIntTy::new(IntSign::Unsigned, 8));
            match op {
                // load(v, off) -> <bus-ty>  (pure)
                U8VecBusOp::Load { ty, .. } => fn2_pure(u8vec(), Ty::Int, Ty::FixedInt(ty)),
                // store(v, off, x) -> Int / {Mut}
                U8VecBusOp::Store { ty, .. } => {
                    fn3_mut(u8vec(), Ty::Int, Ty::FixedInt(ty), Ty::Int)
                }
            }
        }
    }
}

fn checked_result_ty(int_ty: FixedIntTy) -> Ty {
    let mut fields = BTreeMap::new();
    fields.insert("ok".to_string(), Ty::Bool);
    fields.insert("value".to_string(), Ty::FixedInt(int_ty));
    Ty::Record(fields)
}

fn fixed_fn(args: &[Ty], ret: Ty) -> Ty {
    args.iter().rev().cloned().fold(ret, |acc, arg| {
        Ty::Fn(Box::new(arg), Box::new(acc), FnEff::pure_())
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

/// True if `name` is an IO-producing primitive (from libc-effects.toml + ADR 0067).
pub fn is_io_prim(name: &str) -> bool {
    matches!(
        name,
        "write" | "read" | "exit" | "stdin-slurp" | "write-range"
    )
}

/// True if `name` is an Alloc-producing primitive.
pub fn is_alloc_prim(name: &str) -> bool {
    if let Some(VecPrim::Vec {
        op: VecOp::Alloc, ..
    }) = parse_vec_prim(name)
    {
        return true;
    }
    matches!(name, "buf-alloc" | "buf-alloc-dyn" | "i64-alloc")
}

/// True if `name` is a Mut-producing primitive (ADR 0047, 0085).
pub fn is_mut_prim(name: &str) -> bool {
    if let Some(prim) = parse_vec_prim(name) {
        return matches!(
            prim,
            VecPrim::Vec { op: VecOp::Set, .. }
                | VecPrim::U8Vec(U8VecOp::Fill | U8VecOp::Copy)
                | VecPrim::U8VecBus(U8VecBusOp::Store { .. })
        );
    }
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
            | "sort-i64"
            | "sort-ranges-by-bytes"
            | "stable-sort-pairs-i64"
            | "count-equal-ranges"
            | "dedup-adjacent-ranges"
            | "stdin-slurp"
            | "buf-rev"
            | "utf8-encode"
            | "map"
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

fn full_eff() -> FnEff {
    FnEff::from_set(EffSet::of([
        EffAtom::Alloc,
        EffAtom::Div,
        EffAtom::IO,
        EffAtom::Mut,
    ]))
}

fn map_i64_ty() -> Ty {
    let callback = Ty::Fn(Box::new(Ty::Int), Box::new(Ty::Int), full_eff());
    Ty::Fn(
        Box::new(Ty::I64Vec),
        Box::new(Ty::Fn(
            Box::new(Ty::Int),
            Box::new(Ty::Fn(
                Box::new(callback),
                Box::new(Ty::Fn(Box::new(Ty::I64Vec), Box::new(Ty::Int), full_eff())),
                FnEff::pure_(),
            )),
            FnEff::pure_(),
        )),
        FnEff::pure_(),
    )
}

fn fold_i64_ty() -> Ty {
    let callback = Ty::Fn(
        Box::new(Ty::Int),
        Box::new(Ty::Fn(Box::new(Ty::Int), Box::new(Ty::Int), full_eff())),
        FnEff::pure_(),
    );
    Ty::Fn(
        Box::new(Ty::I64Vec),
        Box::new(Ty::Fn(
            Box::new(Ty::Int),
            Box::new(Ty::Fn(
                Box::new(Ty::Int),
                Box::new(Ty::Fn(Box::new(callback), Box::new(Ty::Int), full_eff())),
                FnEff::pure_(),
            )),
            FnEff::pure_(),
        )),
        FnEff::pure_(),
    )
}

/// Standalone (partial-application) type for `@loop` (ADR 0093).
/// Real applications go through `infer_loop_app` so the state type can be
/// unified with `init` and the step callback effect can be threaded into
/// the loop's overall effect; this entry only exists so a bare `@loop`
/// reference or partial application has *some* type before reaching the
/// special-case path.
fn loop_ty() -> Ty {
    let state = Ty::Unknown;
    let step_ret = loop_result_ty(&state);
    let callback = Ty::Fn(Box::new(state.clone()), Box::new(step_ret), full_eff());
    Ty::Fn(
        Box::new(state.clone()),
        Box::new(Ty::Fn(Box::new(callback), Box::new(state), full_eff())),
        FnEff::pure_(),
    )
}

/// Standalone type for `@loop-step` / `@loop-exit` (ADR 0093).
/// Both take a value and return the loop-directive record `{tag, value}`.
fn loop_directive_ty() -> Ty {
    Ty::Fn(
        Box::new(Ty::Unknown),
        Box::new(loop_result_ty(&Ty::Unknown)),
        FnEff::pure_(),
    )
}

/// Loop directive record `{ tag : Int, value : S }` (ADR 0093).
pub fn loop_result_ty(state: &Ty) -> Ty {
    let mut fields = BTreeMap::new();
    fields.insert("tag".to_string(), Ty::Int);
    fields.insert("value".to_string(), state.clone());
    Ty::Record(fields)
}

fn for_each_i64_ty() -> Ty {
    let callback = Ty::Fn(Box::new(Ty::Int), Box::new(Ty::Int), full_eff());
    Ty::Fn(
        Box::new(Ty::I64Vec),
        Box::new(Ty::Fn(
            Box::new(Ty::Int),
            Box::new(Ty::Fn(Box::new(callback), Box::new(Ty::Int), full_eff())),
            FnEff::pure_(),
        )),
        FnEff::pure_(),
    )
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

/// Binary function, {IO, Mut} at the innermost (fully-applied) step.
fn fn2_mut_io(a: Ty, b: Ty, r: Ty) -> Ty {
    Ty::Fn(
        Box::new(a),
        Box::new(Ty::Fn(Box::new(b), Box::new(r), io_mut_eff())),
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

/// Binary function, {Mut} at the innermost (fully-applied) step.
fn fn2_mut(a: Ty, b: Ty, r: Ty) -> Ty {
    Ty::Fn(
        Box::new(a),
        Box::new(Ty::Fn(Box::new(b), Box::new(r), mut_eff())),
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

/// Quaternary function, IO only at the innermost (fully-applied) step.
fn fn4_io(a: Ty, b: Ty, c: Ty, d: Ty, r: Ty) -> Ty {
    Ty::Fn(
        Box::new(a),
        Box::new(Ty::Fn(
            Box::new(b),
            Box::new(Ty::Fn(
                Box::new(c),
                Box::new(Ty::Fn(Box::new(d), Box::new(r), io_eff())),
                FnEff::pure_(),
            )),
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

/// Quaternary function, {Mut} at the innermost (fully-applied) step.
fn fn4_mut(a: Ty, b: Ty, c: Ty, d: Ty, r: Ty) -> Ty {
    Ty::Fn(
        Box::new(a),
        Box::new(Ty::Fn(
            Box::new(b),
            Box::new(Ty::Fn(
                Box::new(c),
                Box::new(Ty::Fn(Box::new(d), Box::new(r), mut_eff())),
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

    #[test]
    fn stdin_slurp_has_io_and_mut() {
        let t = prim_type("stdin-slurp").unwrap();
        // stdin-slurp :: Buf → Int → Int / {IO, Mut}
        if let Ty::Fn(a, mid, _) = &t {
            assert_eq!(a.as_ref(), &Ty::Buf);
            if let Ty::Fn(b, r, eff) = mid.as_ref() {
                assert_eq!(b.as_ref(), &Ty::Int);
                assert_eq!(r.as_ref(), &Ty::Int);
                assert_eq!(
                    eff,
                    &FnEff::from_set(EffSet::of([EffAtom::IO, EffAtom::Mut]))
                );
                return;
            }
        }
        panic!("unexpected type shape for stdin-slurp");
    }

    #[test]
    fn write_range_has_io_at_innermost() {
        let t = prim_type("write-range").unwrap();
        // write-range :: Int → Buf → Int → Int → Int / {IO}
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
        assert_eq!(args, vec![Ty::Int, Ty::Buf, Ty::Int, Ty::Int]);
        assert_eq!(eff, FnEff::from_set(EffSet::of([EffAtom::IO])));
    }

    #[test]
    fn buf_rev_has_mut_only() {
        let t = prim_type("buf-rev").unwrap();
        // buf-rev :: Buf → Int → Int → Int / {Mut}
        if let Ty::Fn(a, mid, _) = &t {
            assert_eq!(a.as_ref(), &Ty::Buf);
            if let Ty::Fn(_, inner, _) = mid.as_ref() {
                if let Ty::Fn(_, r, eff) = inner.as_ref() {
                    assert_eq!(r.as_ref(), &Ty::Int);
                    assert_eq!(eff, &FnEff::from_set(EffSet::of([EffAtom::Mut])));
                    return;
                }
            }
        }
        panic!("unexpected type shape for buf-rev");
    }

    #[test]
    fn bundle_e_classification() {
        assert!(is_io_prim("stdin-slurp"));
        assert!(is_io_prim("write-range"));
        assert!(!is_io_prim("buf-rev"));
        assert!(is_mut_prim("stdin-slurp"));
        assert!(!is_mut_prim("write-range"));
        assert!(is_mut_prim("buf-rev"));
    }

    #[test]
    fn ascii_primitives_are_pure_int_to_int() {
        for name in [
            "ascii-tolower",
            "ascii-toupper",
            "ascii-is-alpha",
            "ascii-is-digit",
            "ascii-is-space",
        ] {
            let t = prim_type(name).unwrap();
            match &t {
                Ty::Fn(a, r, eff) => {
                    assert_eq!(a.as_ref(), &Ty::Int, "{name} arg");
                    assert_eq!(r.as_ref(), &Ty::Int, "{name} return");
                    assert_eq!(eff, &FnEff::pure_(), "{name} effect");
                }
                _ => panic!("{name}: expected Fn"),
            }
        }
    }

    #[test]
    fn bundle_f_classification() {
        for name in [
            "ascii-tolower",
            "ascii-toupper",
            "ascii-is-alpha",
            "ascii-is-digit",
            "ascii-is-space",
        ] {
            assert!(!is_io_prim(name), "{name} should not be IO");
            assert!(!is_mut_prim(name), "{name} should not be Mut");
            assert!(!is_alloc_prim(name), "{name} should not be Alloc");
        }
    }

    #[test]
    fn utf8_decode_is_pure_buf_int_to_int() {
        let t = prim_type("utf8-decode").unwrap();
        // utf8-decode :: Buf → Int → Int (pure)
        if let Ty::Fn(a, mid, e1) = &t {
            assert_eq!(a.as_ref(), &Ty::Buf);
            assert_eq!(e1, &FnEff::pure_());
            if let Ty::Fn(b, r, e2) = mid.as_ref() {
                assert_eq!(b.as_ref(), &Ty::Int);
                assert_eq!(r.as_ref(), &Ty::Int);
                assert_eq!(e2, &FnEff::pure_());
                return;
            }
        }
        panic!("unexpected type shape for utf8-decode");
    }

    #[test]
    fn utf8_encode_has_mut_at_innermost() {
        let t = prim_type("utf8-encode").unwrap();
        // utf8-encode :: Buf → Int → Int → Int / {Mut}
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
        assert_eq!(args, vec![Ty::Buf, Ty::Int, Ty::Int]);
        assert_eq!(eff, FnEff::from_set(EffSet::of([EffAtom::Mut])));
    }

    #[test]
    fn utf8_len_is_pure_int_to_int() {
        let t = prim_type("utf8-len").unwrap();
        match &t {
            Ty::Fn(a, r, eff) => {
                assert_eq!(a.as_ref(), &Ty::Int);
                assert_eq!(r.as_ref(), &Ty::Int);
                assert_eq!(eff, &FnEff::pure_());
            }
            _ => panic!("expected Fn for utf8-len"),
        }
    }

    #[test]
    fn bundle_g_classification() {
        assert!(!is_io_prim("utf8-decode"));
        assert!(!is_io_prim("utf8-encode"));
        assert!(!is_io_prim("utf8-len"));
        assert!(!is_mut_prim("utf8-decode"));
        assert!(is_mut_prim("utf8-encode"));
        assert!(!is_mut_prim("utf8-len"));
        assert!(!is_alloc_prim("utf8-decode"));
        assert!(!is_alloc_prim("utf8-encode"));
        assert!(!is_alloc_prim("utf8-len"));
    }

    #[test]
    fn ordering_primitives_have_expected_shapes_and_mut() {
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

        for (name, expected_args) in [
            ("sort-i64", vec![Ty::I64Vec, Ty::Int]),
            ("sort-ranges-by-bytes", vec![Ty::Buf, Ty::I64Vec, Ty::Int]),
            (
                "stable-sort-pairs-i64",
                vec![Ty::I64Vec, Ty::I64Vec, Ty::Int],
            ),
            ("lower-bound-i64", vec![Ty::I64Vec, Ty::Int, Ty::Int]),
        ] {
            let mut args = Vec::new();
            let eff = args_and_eff(&prim_type(name).unwrap(), &mut args);
            assert_eq!(args, expected_args);
            let expected_eff = if name == "lower-bound-i64" {
                FnEff::pure_()
            } else {
                FnEff::from_set(EffSet::of([EffAtom::Mut]))
            };
            assert_eq!(eff, expected_eff);
        }

        for (name, expected_args) in [
            (
                "count-equal-ranges",
                vec![Ty::Buf, Ty::I64Vec, Ty::Int, Ty::I64Vec],
            ),
            (
                "dedup-adjacent-ranges",
                vec![Ty::Buf, Ty::I64Vec, Ty::Int, Ty::I64Vec],
            ),
        ] {
            let mut args = Vec::new();
            let eff = args_and_eff(&prim_type(name).unwrap(), &mut args);
            assert_eq!(args, expected_args);
            assert_eq!(eff, FnEff::from_set(EffSet::of([EffAtom::Mut])));
        }
    }
}
