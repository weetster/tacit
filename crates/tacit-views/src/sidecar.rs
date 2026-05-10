//! Sidecar format (.tacd) types, JSON I/O, and staleness detection.
//!
//! Reference: plans/sidecar-format.md, decisions/0014-sidecar-format.md.

use std::collections::BTreeMap;
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

    /// Authoring-view type string for the node's inferred value type (e.g. `"Int -> Int"`).
    /// On the root display node this describes the program's evaluated type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_hint: Option<String>,

    /// Effect atoms expected at this node (e.g. `["IO"]`).
    /// On the root display node this describes the program's eval-effect set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effect_hint: Option<Vec<String>>,

    /// Child sidecar entries in canonical child order. Absent ≡ all-null.
    /// Internal entries may be None (≡ {} with implicit all-null subtree).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<Option<SidecarNode>>>,

    /// Unit display alias.
    #[serde(
        rename = "unit_alias",
        alias = "module_alias",
        skip_serializing_if = "Option::is_none"
    )]
    pub unit_alias: Option<String>,

    /// Unit definition hash → display alias map.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub definition_aliases: Option<BTreeMap<String, String>>,

    /// Unit import hash → display alias map.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub import_aliases: Option<BTreeMap<String, String>>,

    /// Unit exported definition hash → display alias map.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub export_aliases: Option<BTreeMap<String, String>>,
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
            && self.type_hint.is_none()
            && self.effect_hint.is_none()
            && self.unit_alias.is_none()
            && self.definition_aliases.is_none()
            && self.import_aliases.is_none()
            && self.export_aliases.is_none()
            && self.children.as_ref().is_none_or(|c| {
                c.iter()
                    .all(|opt| opt.as_ref().is_none_or(|n| n.is_empty()))
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sidecar {
    pub tacd_version: String,
    pub targets_hash_blake3: String,
    #[serde(
        rename = "unit_alias",
        alias = "module_alias",
        skip_serializing_if = "Option::is_none"
    )]
    pub unit_alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub definition_aliases: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub import_aliases: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub export_aliases: Option<BTreeMap<String, String>>,
    pub display: SidecarNode,
}

impl Sidecar {
    pub fn new(canonical_bytes: &[u8], mut display: SidecarNode) -> Self {
        let hash = hash_bytes(canonical_bytes);
        let hex = hash.iter().map(|b| format!("{:02x}", b)).collect();
        let unit_alias = display.unit_alias.take();
        let definition_aliases = display.definition_aliases.take();
        let import_aliases = display.import_aliases.take();
        let export_aliases = display.export_aliases.take();
        Sidecar {
            tacd_version: "1".to_string(),
            targets_hash_blake3: hex,
            unit_alias,
            definition_aliases,
            import_aliases,
            export_aliases,
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
        let mut sidecar: Sidecar = serde_json::from_slice(&data).map_err(SidecarError::Json)?;
        if sidecar.tacd_version != "1" {
            return Err(SidecarError::UnknownVersion(sidecar.tacd_version));
        }
        sidecar.merge_unit_metadata_into_display();
        Ok(sidecar)
    }

    pub fn write(&self, path: &Path) -> Result<(), SidecarError> {
        let json = serde_json::to_string_pretty(self).map_err(SidecarError::Json)?;
        std::fs::write(path, json.as_bytes()).map_err(SidecarError::Io)?;
        Ok(())
    }

    fn merge_unit_metadata_into_display(&mut self) {
        if self.display.unit_alias.is_none() {
            self.display.unit_alias = self.unit_alias.clone();
        }
        if self.display.definition_aliases.is_none() {
            self.display.definition_aliases = self.definition_aliases.clone();
        }
        if self.display.import_aliases.is_none() {
            self.display.import_aliases = self.import_aliases.clone();
        }
        if self.display.export_aliases.is_none() {
            self.display.export_aliases = self.export_aliases.clone();
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_empty_with_type_hint() {
        let node = SidecarNode {
            type_hint: Some("Int".to_string()),
            ..Default::default()
        };
        assert!(!node.is_empty());
    }

    #[test]
    fn is_empty_with_effect_hint() {
        let node = SidecarNode {
            effect_hint: Some(vec!["IO".to_string()]),
            ..Default::default()
        };
        assert!(!node.is_empty());
    }

    #[test]
    fn type_hint_effect_hint_round_trip() {
        let node = SidecarNode {
            type_hint: Some("Int -> Int".to_string()),
            effect_hint: Some(vec!["IO".to_string(), "Alloc".to_string()]),
            ..Default::default()
        };
        let sidecar = Sidecar::new(b"(int 1)", node.clone());
        let json = serde_json::to_string(&sidecar).unwrap();
        let back: Sidecar = serde_json::from_str(&json).unwrap();
        assert_eq!(back.display.type_hint, node.type_hint);
        assert_eq!(back.display.effect_hint, node.effect_hint);
    }

    #[test]
    fn absent_hints_omitted_from_json() {
        let node = SidecarNode {
            binder: Some("x".to_string()),
            ..Default::default()
        };
        let sidecar = Sidecar::new(b"(int 1)", node);
        let json = serde_json::to_string(&sidecar).unwrap();
        assert!(!json.contains("type_hint"), "type_hint should be absent");
        assert!(
            !json.contains("effect_hint"),
            "effect_hint should be absent"
        );
    }

    #[test]
    fn unit_alias_serializes_and_accepts_old_module_alias() {
        let node = SidecarNode {
            unit_alias: Some("Math".to_string()),
            ..Default::default()
        };
        let sidecar = Sidecar::new(b"(int 1)", node);
        let json = serde_json::to_string(&sidecar).unwrap();
        assert!(json.contains("unit_alias"));
        assert!(!json.contains("module_alias"));

        let old_json = r#"{
            "tacd_version": "1",
            "targets_hash_blake3": "abc",
            "module_alias": "Math",
            "display": {}
        }"#;
        let back: Sidecar = serde_json::from_str(old_json).unwrap();
        assert_eq!(back.unit_alias.as_deref(), Some("Math"));
    }
}
