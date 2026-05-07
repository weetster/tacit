//! Codegen diagnostics. Phase 1 keeps hard failures (per ADR 0023);
//! Hole-node recovery is deferred.

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

    /// Free DeBruijn index referencing a binder above the enclosing `Lam`.
    /// Phase 1 lambdas must be closed (ADR 0026 § 1).
    #[error("free variable in lambda: DeBruijn index {index}")]
    FreeVarInLambda { index: u64 },

    /// `App` whose function position is a non-function value, or a `Var`
    /// resolving to a non-`Lam` binding. ADR 0026 § 3.
    #[error("application of non-function value")]
    AppNonFunction,

    /// A direct Tacit function call supplied too few or too many arguments.
    /// Partial application remains unsupported by codegen.
    #[error("function expects arity {expected}, got {got}")]
    FunctionArity { expected: usize, got: usize },

    /// `Var` that resolves to a `Lam` binding but appears outside `App` head
    /// position. First-class function values are banned in Phase 1 (ADR 0026 § 4).
    #[error("first-class function value: lambdas may only appear at App head in Phase 1")]
    FirstClassFunction,

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

    /// AST node whose Phase 1 lowering is not yet implemented.
    #[error("Phase 1 codegen does not yet support {0}")]
    Unsupported(&'static str),

    /// A value appeared where codegen only supports integer-like values.
    #[error("expected integer value, got {actual}")]
    ExpectedIntValue { actual: String },

    /// A source-level value type has no Phase 4 Stage 2 LLVM representation.
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
