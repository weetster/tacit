//! Authoring and inspection views for the Tacit-Lite canonical AST.
//!
//! Phase 1 Stage 2: authoring view parser + emitter, sidecar I/O.
//! Reference: plans/candidates/authoring-bpe-compact.md, plans/sidecar-format.md.

pub mod authoring;
pub mod sidecar;

pub use sidecar::{Sidecar, SidecarNode};
