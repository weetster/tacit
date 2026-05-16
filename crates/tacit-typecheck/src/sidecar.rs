//! Type-expectation sidecars (`.tacd`) per ADR 0071.
//!
//! Checks `type_hint` / `effect_hint` on the root display node against the
//! inferred type and eval-effect of the top-level expression.

use tacit_canonical::ast::Node;
use tacit_views;

use crate::error::Diagnostic;
use crate::infer::infer;
use crate::ty::{EffAtom, EffSet, Subst, Ty};

/// Check an AST against `type_hint` / `effect_hint` on the root display node of a `.tacd` sidecar.
///
/// Runs type inference on `ast` and compares the result against the hints.
/// Returns `Ok(())` when both hints are absent (nothing to check), on success,
/// or `Err(diags)` on mismatch or type error.
pub fn check_against_tacd(
    ast: &Node,
    sidecar: &tacit_views::sidecar::Sidecar,
) -> Result<(), Vec<Diagnostic>> {
    let mut subst = Subst::default();
    let mut diags: Vec<Diagnostic> = Vec::new();

    let (inferred, eval_eff_fn) = infer(&[], ast, &mut subst, &[], &mut diags);
    let inferred = subst.apply(&inferred);
    let eval_eff = subst.resolve_eff(&eval_eff_fn);

    if !diags.is_empty() {
        return Err(diags);
    }

    let display = &sidecar.display;

    if let Some(type_str) = &display.type_hint {
        let expected_ty = match parse_type_str(type_str) {
            Ok(t) => t,
            Err(e) => {
                diags.push(Diagnostic::unresolved_type(
                    &[],
                    &format!("sidecar type_hint parse error: {}", e),
                ));
                Ty::Unknown
            }
        };
        if !types_match(&inferred, &expected_ty) {
            diags.push(Diagnostic::type_mismatch(&[], &expected_ty, &inferred));
        }
    }

    if let Some(effect_atoms) = &display.effect_hint {
        let expected_eff = parse_effect_list(effect_atoms);
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
        fixed if crate::ty::FixedIntTy::parse_name(fixed).is_some() => Ok(Ty::FixedInt(
            crate::ty::FixedIntTy::parse_name(fixed).unwrap(),
        )),
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
        (Ty::IntLit, Ty::Int)
        | (Ty::Int, Ty::IntLit)
        | (Ty::IntLit, Ty::IntLit)
        | (Ty::Int, Ty::Int)
        | (Ty::Bool, Ty::Bool)
        | (Ty::Str, Ty::Str)
        | (Ty::Buf, Ty::Buf)
        | (Ty::I64Vec, Ty::I64Vec) => true,
        (Ty::FixedInt(a), Ty::FixedInt(b)) => a == b,
        (Ty::Int, Ty::FixedInt(fixed)) | (Ty::FixedInt(fixed), Ty::Int) if fixed.is_i64() => true,
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

    // ── check_against_tacd tests ──────────────────────────────────────────────

    fn make_tacd(
        type_hint: Option<&str>,
        effect_hint: Option<Vec<&str>>,
    ) -> tacit_views::sidecar::Sidecar {
        use tacit_views::sidecar::{Sidecar, SidecarNode};
        let display = SidecarNode {
            type_hint: type_hint.map(str::to_owned),
            effect_hint: effect_hint.map(|v| v.into_iter().map(str::to_owned).collect()),
            ..Default::default()
        };
        Sidecar::new(b"(int 0)", display)
    }

    #[test]
    fn tacd_no_hints_passes() {
        let ast = Node::Int {
            value: "42".to_string(),
        };
        let sidecar = make_tacd(None, None);
        assert!(check_against_tacd(&ast, &sidecar).is_ok());
    }

    #[test]
    fn tacd_type_hint_match() {
        let ast = Node::Int {
            value: "42".to_string(),
        };
        let sidecar = make_tacd(Some("Int"), Some(vec![]));
        assert!(check_against_tacd(&ast, &sidecar).is_ok());
    }

    #[test]
    fn tacd_type_hint_mismatch() {
        let ast = Node::Str {
            value: "hello".to_string(),
        };
        let sidecar = make_tacd(Some("Int"), None);
        let diags = check_against_tacd(&ast, &sidecar).unwrap_err();
        assert!(diags.iter().any(|d| d.kind == "type-mismatch"));
    }

    #[test]
    fn tacd_effect_hint_mismatch() {
        let ast = Node::Int {
            value: "0".to_string(),
        };
        let sidecar = make_tacd(Some("Int"), Some(vec!["IO"]));
        let diags = check_against_tacd(&ast, &sidecar).unwrap_err();
        assert!(diags.iter().any(|d| d.kind == "effect-violation"));
    }
}
