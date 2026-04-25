//! Sidecar format (.tacd) types, JSON I/O, and staleness detection.
//!
//! Reference: plans/sidecar-format.md, decisions/0014-sidecar-format.md.

use std::path::Path;

use serde::{Deserialize, Serialize};
use tacit_canonical::hash_bytes;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SidecarNode {
    /// Binder name for lam, let, pat-var nodes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binder: Option<String>,

    /// Binder names for rec and module nodes (parallel to canonical child order).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binders: Option<Vec<String>>,

    /// Advisory comment; shown by inspection view, skipped by authoring view.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,

    /// Authoring-order permutation for record fields (§ 3.3).
    /// field_order[i] = canonical index rendered at authoring position i.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field_order: Option<Vec<usize>>,

    /// Child sidecar entries in canonical child order. Absent ≡ all-null.
    /// Internal entries may be None (≡ {} with implicit all-null subtree).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<Option<SidecarNode>>>,
}

impl SidecarNode {
    /// Fetch canonical child i, treating absent/null as an empty node.
    pub fn child(&self, i: usize) -> Option<&SidecarNode> {
        self.children
            .as_ref()
            .and_then(|v| v.get(i))
            .and_then(|opt| opt.as_ref())
    }

    /// True if this node and all descendants carry no metadata.
    pub fn is_empty(&self) -> bool {
        self.binder.is_none()
            && self.binders.is_none()
            && self.comment.is_none()
            && self.field_order.is_none()
            && self.children.as_ref().map_or(true, |c| {
                c.iter().all(|opt| opt.as_ref().map_or(true, |n| n.is_empty()))
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sidecar {
    pub tacd_version: String,
    pub targets_hash_blake3: String,
    pub display: SidecarNode,
}

impl Sidecar {
    pub fn new(canonical_bytes: &[u8], display: SidecarNode) -> Self {
        let hash = hash_bytes(canonical_bytes);
        let hex = hash.iter().map(|b| format!("{:02x}", b)).collect();
        Sidecar {
            tacd_version: "1".to_string(),
            targets_hash_blake3: hex,
            display,
        }
    }

    /// True iff the sidecar's hash matches the given canonical bytes.
    pub fn is_fresh(&self, canonical_bytes: &[u8]) -> bool {
        let hash = hash_bytes(canonical_bytes);
        let expected: String = hash.iter().map(|b| format!("{:02x}", b)).collect();
        self.targets_hash_blake3 == expected
    }

    pub fn read(path: &Path) -> Result<Sidecar, SidecarError> {
        let data = std::fs::read(path).map_err(SidecarError::Io)?;
        let sidecar: Sidecar = serde_json::from_slice(&data).map_err(SidecarError::Json)?;
        if sidecar.tacd_version != "1" {
            return Err(SidecarError::UnknownVersion(sidecar.tacd_version));
        }
        Ok(sidecar)
    }

    pub fn write(&self, path: &Path) -> Result<(), SidecarError> {
        let json = serde_json::to_string_pretty(self).map_err(SidecarError::Json)?;
        std::fs::write(path, json.as_bytes()).map_err(SidecarError::Io)?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum SidecarError {
    Io(std::io::Error),
    Json(serde_json::Error),
    UnknownVersion(String),
}

impl std::fmt::Display for SidecarError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SidecarError::Io(e) => write!(f, "I/O error: {}", e),
            SidecarError::Json(e) => write!(f, "JSON error: {}", e),
            SidecarError::UnknownVersion(v) => write!(f, "unknown sidecar version {:?}", v),
        }
    }
}

impl std::error::Error for SidecarError {}
