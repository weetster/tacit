//! Codegen diagnostics. Codegen still hard-fails on Hole nodes per ADR 0023,
//! while Phase 4 closure failures use explicit variants from ADR 0073.

use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CodegenError {
    /// `Sym` head at `App` position whose name is not in any allowlist
    /// (LIBC ∪ ARITH ∪ CMP). See ADR 0028 + ADR 0030.
    #[error("unknown primitive @{name}")]
    UnknownPrimitive { name: String },

    /// A primitive expects a fixed arity; user supplied a different count.
    /// Per ADR 0030 § Arity, all Phase 1 primitives are exactly binary
    /// or fixed-arity; libc primitives have their own arities.
    #[error("primitive @{name} expects arity {expected}, got {got}")]
    PrimitiveArity {
        name: String,
        expected: usize,
        got: usize,
    },

    /// Free DeBruijn index with no runtime binding available at the current
    /// lowering point.
    #[error("free variable in lambda: DeBruijn index {index}")]
    FreeVarInLambda { index: u64 },

    /// `App` whose function position is not function-typed.
    #[error("application of non-function value")]
    AppNonFunction,

    /// A direct Tacit function call supplied the wrong number of arguments.
    /// Reifiable functions use closure application for partial calls.
    #[error("function expects arity {expected}, got {got}")]
    FunctionArity { expected: usize, got: usize },

    /// Compatibility alias retained for older callers that matched the Phase 1
    /// first-class-function restriction. Stage 4 should prefer closure values
    /// or `UnsupportedClosureEscape`.
    #[error("unsupported first-class function value")]
    FirstClassFunction,

    /// A first-class closure capture set includes a non-escapable value such
    /// as a Buf or I64Vec handle (ADR 0073).
    #[error("invalid closure capture of non-escapable {kind} handle at DeBruijn index {index}")]
    InvalidCapture { index: u64, kind: &'static str },

    /// A function binding cannot be reified at the requested escape site.
    #[error("unsupported closure escape: {0}")]
    UnsupportedClosureEscape(&'static str),

    /// Internal closure-conversion guard: a DeBruijn index addressed an outer
    /// binding that was intentionally not loaded into the closure environment.
    #[error("internal closure conversion error: uncaptured outer binding {index} was read")]
    UnavailableCapture { index: u64 },

    /// `Rec` group emission failed in member at `failing_index`.
    /// ADR 0027 § 1c: groups are emitted atomically.
    #[error("rec group lowering failed at member {failing_index}: {cause}")]
    RecGroupFailed {
        failing_index: usize,
        cause: Box<CodegenError>,
    },

    /// Non-numeric argument to an arithmetic / comparison primitive.
    #[error("primitive @{name} expects integer arguments")]
    NonIntegerArg { name: String },

    /// AST node whose lowering is not yet implemented.
    #[error("codegen does not yet support {0}")]
    Unsupported(&'static str),

    /// A value appeared where codegen only supports integer-like values.
    #[error("expected integer value, got {actual}")]
    ExpectedIntValue { actual: String },

    /// A source-level value type has no LLVM representation in this codegen
    /// subset.
    #[error("unsupported value type in codegen: {ty}")]
    UnsupportedValueType { ty: String },

    /// Two source-level values with incompatible structural types met at a
    /// codegen join or call boundary.
    #[error("value type mismatch: expected {expected}, got {actual}")]
    ValueTypeMismatch { expected: String, actual: String },

    /// Projection from a value that is not a record.
    #[error("invalid projection .{field} from non-record value {actual}")]
    InvalidProjection { field: String, actual: String },

    /// Projection of a field that is absent from the record shape.
    #[error("record field .{field} does not exist")]
    MissingField { field: String },

    /// Integer literal that does not fit in `i64`.
    #[error("integer literal out of i64 range: {value}")]
    IntegerOverflow { value: String },

    /// LLVM-side error surfaced through `inkwell` (verifier failure, target
    /// machine init, object emission, etc.).
    #[error("LLVM error: {0}")]
    Llvm(String),

    /// Hole node encountered. Phase 1 fails hard (ADR 0023).
    #[error("hole node in codegen: diag-id {diag_id}")]
    Hole { diag_id: String },

    /// `match` arm pattern outside the Phase 1 supported subset.
    /// Phase 1 supports literal-integer arms and a single trailing wildcard
    /// (smoke corpus #7).
    #[error("unsupported match arm pattern")]
    UnsupportedMatchPattern,

    /// `match` exhaustiveness: scrutinee fell through every arm with no
    /// wildcard catch-all.
    #[error("non-exhaustive match: scrutinee value {value} matched no arm")]
    NonExhaustiveMatch { value: i64 },
}
