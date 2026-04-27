//! Type-expectation sidecars (`.tac.sidecar.toml`) per ADR 0043.
//!
//! Each smoke program may carry a `[types.<name>]` table with expected type
//! and effect annotations. The `check_against_sidecar` function verifies the
//! inferred type of the "main" expression matches the sidecar expectation.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use tacit_canonical::ast::Node;

use crate::error::Diagnostic;
use crate::infer::infer;
use crate::ty::{Subst, Ty};

/// A type expectation entry for one named binding.
#[derive(Debug, Clone, Deserialize)]
pub struct TypeEntry {
    /// Authoring-view type string, e.g. `"Int -> Int"`.
    #[serde(rename = "type")]
    pub type_str: String,
    /// Sorted list of effect atoms.
    pub effects: Vec<String>,
}

/// TOML-format type sidecar for a program.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct TypeSidecar {
    /// `[types.<name>]` table.
    #[serde(default)]
    pub types: BTreeMap<String, TypeEntry>,
}

impl TypeSidecar {
    /// Load from a `.tac.sidecar.toml` file. Returns `Ok(Default::default())`
    /// if the file does not exist (missing sidecar = no expectations).
    pub fn load(path: &Path) -> Result<TypeSidecar, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let outer: TomlOuter = toml::from_str(&text)
                    .map_err(|e| format!("TOML parse error in {:?}: {}", path, e))?;
                Ok(TypeSidecar {
                    types: outer.types.unwrap_or_default(),
                })
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(TypeSidecar::default()),
            Err(e) => Err(format!("I/O error reading {:?}: {}", path, e)),
        }
    }

    pub fn get(&self, name: &str) -> Option<&TypeEntry> {
        self.types.get(name)
    }
}

/// Internal TOML deserialization wrapper.
#[derive(Deserialize)]
struct TomlOuter {
    types: Option<BTreeMap<String, TypeEntry>>,
}

/// Check an AST against the `[types.main]` entry in a type sidecar.
///
/// Runs type inference on `ast` and compares the result against the sidecar's
/// expected type. Returns `Ok(())` on success, `Err(diags)` on mismatch or
/// type error.
///
/// Stage 2 only checks the "main" type (overall program type). Per-binding
/// checks (e.g. `[types.factorial]`) are validated by dedicated test helpers
/// in `tests/smoke.rs`.
pub fn check_against_sidecar(
    ast: &Node,
    type_sidecar: &TypeSidecar,
) -> Result<(), Vec<Diagnostic>> {
    let mut subst = Subst::default();
    let mut diags: Vec<Diagnostic> = Vec::new();

    let inferred = infer(&[], ast, &mut subst, &[], &mut diags);
    let inferred = subst.apply(&inferred);

    if !diags.is_empty() {
        return Err(diags);
    }

    // Check "main" type expectation if present.
    if let Some(entry) = type_sidecar.get("main") {
        let expected = parse_type_str(&entry.type_str);
        match expected {
            Ok(expected_ty) => {
                if !types_match(&inferred, &expected_ty) {
                    diags.push(Diagnostic::type_mismatch(&[], &expected_ty, &inferred));
                }
            }
            Err(e) => {
                diags.push(Diagnostic::unresolved_type(&[], &format!("sidecar parse error: {}", e)));
            }
        }
    }

    if diags.is_empty() {
        Ok(())
    } else {
        Err(diags)
    }
}

/// Parse a simple authoring-view type string (e.g. `"Int -> Int"`) to a `Ty`.
/// Supports: `Int`, `Bool`, `Str`, `T -> U` (right-associative), `(T)`.
pub fn parse_type_str(s: &str) -> Result<Ty, String> {
    let s = s.trim();
    parse_fn_type(s)
}

fn parse_fn_type(s: &str) -> Result<Ty, String> {
    // Find a `->` that is not inside parentheses.
    let bytes = s.as_bytes();
    let mut depth = 0i32;
    let mut arrow_pos: Option<usize> = None;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b'-' if depth == 0 && i + 1 < bytes.len() && bytes[i + 1] == b'>' => {
                arrow_pos = Some(i);
                break; // rightmost? no, we want leftmost for right-assoc; first works.
            }
            _ => {}
        }
        i += 1;
    }

    if let Some(pos) = arrow_pos {
        let left = s[..pos].trim();
        // Skip past `->`; also skip optional `/ {effects}` before the arrow for future compat.
        let right_start = pos + 2;
        let right = s[right_start..].trim();
        let arg = parse_atom_type(left)?;
        let ret = parse_fn_type(right)?;
        Ok(Ty::Fn(Box::new(arg), Box::new(ret)))
    } else {
        parse_atom_type(s)
    }
}

fn parse_atom_type(s: &str) -> Result<Ty, String> {
    let s = s.trim();
    match s {
        "Int" => Ok(Ty::Int),
        "Bool" => Ok(Ty::Bool),
        "Str" => Ok(Ty::Str),
        other if other.starts_with('(') && other.ends_with(')') => {
            parse_fn_type(&other[1..other.len() - 1])
        }
        other => Err(format!("unknown type '{}' in sidecar", other)),
    }
}

/// True if `inferred` matches `expected` structurally (ignoring metavariables).
fn types_match(inferred: &Ty, expected: &Ty) -> bool {
    match (inferred, expected) {
        (Ty::Int, Ty::Int) | (Ty::Bool, Ty::Bool) | (Ty::Str, Ty::Str) => true,
        (Ty::Fn(a1, b1), Ty::Fn(a2, b2)) => types_match(a1, a2) && types_match(b1, b2),
        (Ty::Unknown, _) | (_, Ty::Unknown) => true, // Unknown is compatible with anything.
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_int() {
        assert_eq!(parse_type_str("Int").unwrap(), Ty::Int);
    }

    #[test]
    fn parse_fn() {
        assert_eq!(
            parse_type_str("Int -> Int").unwrap(),
            Ty::Fn(Box::new(Ty::Int), Box::new(Ty::Int))
        );
    }

    #[test]
    fn parse_higher_order() {
        // (Int -> Int) -> Int
        let t = parse_type_str("(Int -> Int) -> Int").unwrap();
        assert_eq!(
            t,
            Ty::Fn(
                Box::new(Ty::Fn(Box::new(Ty::Int), Box::new(Ty::Int))),
                Box::new(Ty::Int)
            )
        );
    }

    #[test]
    fn parse_right_associative() {
        // Int -> Int -> Int = Int -> (Int -> Int)
        let t = parse_type_str("Int -> Int -> Int").unwrap();
        assert_eq!(
            t,
            Ty::Fn(
                Box::new(Ty::Int),
                Box::new(Ty::Fn(Box::new(Ty::Int), Box::new(Ty::Int)))
            )
        );
    }
}
