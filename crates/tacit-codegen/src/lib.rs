//! LLVM codegen for the Tacit-Lite subset implemented so far.
//!
//! Two layers:
//!
//! - **Analysis** (`analysis`, `error`, `primitives`) — pure AST checks
//!   and primitive classification. No LLVM dependency; testable in
//!   isolation.
//! - **Emission** (`compile`, behind the `llvm` feature) — `inkwell`-based
//!   IR construction, object-file emission, and error wrapping. Gated so
//!   the analysis layer compiles without an installed LLVM library.
//!
//! Pick exactly one of the per-version features (`llvm15-0`, `llvm16-0`,
//! `llvm17-0`, `llvm18-1`, `llvm19-1`) to enable the emission layer.
//! `llvm19-1` is the pinned version per ADR 0032 § 1; the older feature
//! flags remain for contributors who already have a different LLVM
//! library installed locally. CI builds against `llvm19-1`.
//!
//! Backing ADRs (emission layer):
//! - [ADR 0024](../../decisions/0024-llvm-bindings-inkwell.md) — `inkwell`.
//! - [ADR 0025](../../decisions/0025-phase-1-libc-surface.md) — libc set.
//! - [ADR 0026](../../decisions/0026-phase-1-closed-lambdas.md) — original direct-call lambda baseline.
//! - [ADR 0027](../../decisions/0027-phase-1-rec-lowering.md) — forward-declare-then-define under `ccc`.
//! - [ADR 0028](../../decisions/0028-phase-1-libc-call-surface.md) — `@name` primitive surface.
//! - [ADR 0030](../../decisions/0030-phase-1-arith-primitives.md) — `@add`/`@sub`/.../`@lt`/... intrinsics.
//! - [ADR 0031](../../decisions/0031-llvm-distribution-and-self-hosting.md) — distribution + self-hosting model.
//! - [ADR 0032](../../decisions/0032-stage-4-frozen.md) — Stage 4 freeze; pins LLVM 19.
//! - [ADR 0073](../../decisions/0073-p4-function-values-and-closures.md) — first-class function values and closures.

pub mod analysis;
pub mod error;
pub mod primitives;

#[cfg(feature = "llvm")]
pub mod compile;

pub use error::CodegenError;

#[cfg(feature = "llvm")]
pub use compile::{
    compile_library_to_ir_string, compile_library_to_object, compile_to_ir_string,
    compile_to_object, Compiler,
};
