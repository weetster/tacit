//! Type-expectation sidecars (`.tac.sidecar.toml`) per ADR 0043.
//!
//! Stage 3: also checks the `effects` field against the inferred eval-effect
//! of the top-level expression.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use tacit_canonical::ast::Node;

use crate::error::Diagnostic;
use crate::infer::infer;
use crate::ty::{EffAtom, EffSet, Subst, Ty};

/// A type+effect expectation entry for one named binding.
#[derive(Debug, Clone, Deserialize)]
pub struct TypeEntry {
    /// Authoring-view type string, e.g. `"Int -> Int"`.
    #[serde(rename = "type")]
    pub type_str: String,
    /// Sorted list of effect atoms, e.g. `["IO"]`.
    pub effects: Vec<String>,
}

/// TOML-format type sidecar for a program.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct TypeSidecar {
    #[serde(default)]
    pub types: BTreeMap<String, TypeEntry>,
}

impl TypeSidecar {
    /// Load from a `.tac.sidecar.toml` file.
    /// Returns `Ok(Default::default())` if the file does not exist.
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

#[derive(Deserialize)]
struct TomlOuter {
    types: Option<BTreeMap<String, TypeEntry>>,
}

/// Check an AST against the `[types.main]` entry in a type sidecar.
///
/// Runs type inference on `ast` and compares the result against the sidecar's
/// expected type and effect set. Returns `Ok(())` on success, `Err(diags)` on
/// mismatch or type error.
pub fn check_against_sidecar(
    ast: &Node,
    type_sidecar: &TypeSidecar,
) -> Result<(), Vec<Diagnostic>> {
    let mut subst = Subst::default();
    let mut diags: Vec<Diagnostic> = Vec::new();

    let (inferred, eval_eff_fn) = infer(&[], ast, &mut subst, &[], &mut diags);
    let inferred = subst.apply(&inferred);
    let eval_eff = subst.resolve_eff(&eval_eff_fn);

    if !diags.is_empty() {
        return Err(diags);
    }

    if let Some(entry) = type_sidecar.get("main") {
        // Check value type.
        let expected_ty = match parse_type_str(&entry.type_str) {
            Ok(t) => t,
            Err(e) => {
                diags.push(Diagnostic::unresolved_type(
                    &[],
                    &format!("sidecar type parse error: {}", e),
                ));
                Ty::Unknown
            }
        };
        if !types_match(&inferred, &expected_ty) {
            diags.push(Diagnostic::type_mismatch(&[], &expected_ty, &inferred));
        }

        // Check effect set.
        let expected_eff = parse_effect_list(&entry.effects);
        if eval_eff != expected_eff {
            diags.push(Diagnostic::effect_set_mismatch(
                &[],
                &expected_eff,
                &eval_eff,
            ));
        }
    }

    if diags.is_empty() {
        Ok(())
    } else {
        Err(diags)
    }
}

// ── Type string parser ────────────────────────────────────────────────────────

/// Parse a simple authoring-view type string to a `Ty`.
/// Supports: `Int`, `Bool`, `Str`, `T -> U` (right-associative), `(T)`.
pub fn parse_type_str(s: &str) -> Result<Ty, String> {
    parse_fn_type(s.trim())
}

fn parse_fn_type(s: &str) -> Result<Ty, String> {
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
                break;
            }
            _ => {}
        }
        i += 1;
    }

    if let Some(pos) = arrow_pos {
        let left = s[..pos].trim();
        let right = s[pos + 2..].trim();
        let arg = parse_atom_type(left)?;
        let ret = parse_fn_type(right)?;
        // Sidecar type strings don't carry effect annotations; assume pure.
        Ok(Ty::Fn(
            Box::new(arg),
            Box::new(ret),
            crate::ty::FnEff::pure_(),
        ))
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
        "Buf" => Ok(Ty::Buf),
        "I64Vec" => Ok(Ty::I64Vec),
        other if other.starts_with('(') && other.ends_with(')') => {
            parse_fn_type(&other[1..other.len() - 1])
        }
        other => Err(format!("unknown type '{}' in sidecar", other)),
    }
}

// ── Effect list parser ────────────────────────────────────────────────────────

/// Parse a list of effect atom strings (e.g. `["IO", "Div"]`) into an `EffSet`.
pub fn parse_effect_list(effects: &[String]) -> EffSet {
    let mut set = EffSet::empty();
    for atom in effects {
        match atom.as_str() {
            "Alloc" => {
                set.atoms.insert(EffAtom::Alloc);
            }
            "Div" => {
                set.atoms.insert(EffAtom::Div);
            }
            "IO" => {
                set.atoms.insert(EffAtom::IO);
            }
            "Mut" => {
                set.atoms.insert(EffAtom::Mut);
            }
            _ => {} // unknown atoms silently ignored in sidecar parsing
        }
    }
    set
}

// ── Type matching ─────────────────────────────────────────────────────────────

/// Structural type matching ignoring effect annotations (effects are checked separately).
fn types_match(inferred: &Ty, expected: &Ty) -> bool {
    match (inferred, expected) {
        (Ty::Int, Ty::Int)
        | (Ty::Bool, Ty::Bool)
        | (Ty::Str, Ty::Str)
        | (Ty::Buf, Ty::Buf)
        | (Ty::I64Vec, Ty::I64Vec) => true,
        (Ty::Fn(a1, b1, _), Ty::Fn(a2, b2, _)) => types_match(a1, a2) && types_match(b1, b2),
        (Ty::Unknown, _) | (_, Ty::Unknown) => true,
        _ => false,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ty::FnEff;

    #[test]
    fn parse_int() {
        assert_eq!(parse_type_str("Int").unwrap(), Ty::Int);
    }

    #[test]
    fn parse_fn() {
        assert_eq!(
            parse_type_str("Int -> Int").unwrap(),
            Ty::Fn(Box::new(Ty::Int), Box::new(Ty::Int), FnEff::pure_())
        );
    }

    #[test]
    fn parse_higher_order() {
        let t = parse_type_str("(Int -> Int) -> Int").unwrap();
        assert_eq!(
            t,
            Ty::Fn(
                Box::new(Ty::Fn(Box::new(Ty::Int), Box::new(Ty::Int), FnEff::pure_())),
                Box::new(Ty::Int),
                FnEff::pure_(),
            )
        );
    }

    #[test]
    fn parse_right_associative() {
        let t = parse_type_str("Int -> Int -> Int").unwrap();
        assert_eq!(
            t,
            Ty::Fn(
                Box::new(Ty::Int),
                Box::new(Ty::Fn(Box::new(Ty::Int), Box::new(Ty::Int), FnEff::pure_())),
                FnEff::pure_(),
            )
        );
    }

    #[test]
    fn parse_effect_list_io() {
        let eff = parse_effect_list(&["IO".to_string()]);
        assert!(eff.atoms.contains(&EffAtom::IO));
        assert_eq!(eff.atoms.len(), 1);
    }

    #[test]
    fn parse_effect_list_div() {
        let eff = parse_effect_list(&["Div".to_string()]);
        assert!(eff.atoms.contains(&EffAtom::Div));
    }

    #[test]
    fn parse_effect_list_empty() {
        let eff = parse_effect_list(&[]);
        assert!(eff.is_pure());
    }

    #[test]
    fn sidecar_type_mismatch() {
        use std::collections::BTreeMap;
        let ast = Node::Str {
            value: "hello".to_string(),
        };
        let mut types = BTreeMap::new();
        types.insert(
            "main".to_string(),
            TypeEntry {
                type_str: "Int".to_string(),
                effects: vec![],
            },
        );
        let sidecar = TypeSidecar { types };
        let result = check_against_sidecar(&ast, &sidecar);
        let diags = result.unwrap_err();
        assert!(diags.iter().any(|d| d.kind == "type-mismatch"));
    }

    #[test]
    fn sidecar_effect_mismatch() {
        use std::collections::BTreeMap;
        // Expression is pure (0), but sidecar expects IO.
        let ast = Node::Int {
            value: "0".to_string(),
        };
        let mut types = BTreeMap::new();
        types.insert(
            "main".to_string(),
            TypeEntry {
                type_str: "Int".to_string(),
                effects: vec!["IO".to_string()],
            },
        );
        let sidecar = TypeSidecar { types };
        let result = check_against_sidecar(&ast, &sidecar);
        let diags = result.unwrap_err();
        assert!(diags.iter().any(|d| d.kind == "effect-violation"));
    }
}
