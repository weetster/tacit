//! Authoring and inspection views for the Tacit-Lite canonical AST.
//!
//! Phase 1 Stage 2: authoring view parser + emitter, sidecar I/O.
//! Phase 1 Stage 3: inspection-view renderer (L0/L1/L2).
//! Reference: plans/candidates/authoring-bpe-compact.md, plans/sidecar-format.md,
//!            plans/inspection-view.md.

pub mod authoring;
pub mod inspection;
pub mod sidecar;

pub use inspection::{emit_inspection, InspectFlags};
pub use sidecar::{Sidecar, SidecarNode};
