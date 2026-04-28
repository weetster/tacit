//! Structured diagnostic type and JSON serialization per ADR 0041.

use serde::Serialize;
use serde_json::Value;

use crate::ty::{EffSet, FnEff, Ty};

/// Top-level diagnostic output envelope.
#[derive(Debug, Serialize)]
pub struct DiagOutput {
    pub schema_version: String,
    pub errors: Vec<Diagnostic>,
}

impl DiagOutput {
    pub fn new(errors: Vec<Diagnostic>) -> Self {
        DiagOutput {
            schema_version: "p2.0".to_string(),
            errors,
        }
    }

    pub fn to_json_string(&self) -> String {
        serde_json::to_string_pretty(self).expect("DiagOutput serialization is infallible")
    }

    pub fn has_errors(&self) -> bool {
        self.errors.iter().any(|d| d.severity == "error")
    }
}

/// A single structured diagnostic per ADR 0041.
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub kind: String,
    pub severity: String,
    pub location: Location,
    pub message: String,
    pub expected: Option<Value>,
    pub actual: Option<Value>,
    pub fix: Option<Fix>,
    pub related: Vec<Diagnostic>,
}

impl Diagnostic {
    fn new(kind: &str, severity: &str, path: &[usize], message: String) -> Self {
        Diagnostic {
            kind: kind.to_string(),
            severity: severity.to_string(),
            location: Location::from_path(path),
            message,
            expected: None,
            actual: None,
            fix: None,
            related: Vec::new(),
        }
    }

    pub fn type_mismatch(path: &[usize], expected: &Ty, actual: &Ty) -> Self {
        let mut d = Self::new(
            "type-mismatch",
            "error",
            path,
            format!("type mismatch: expected {}, got {}", expected, actual),
        );
        d.expected = Some(ty_to_json(expected));
        d.actual = Some(ty_to_json(actual));
        d
    }

    pub fn unbound_type_variable(path: &[usize], index: u64) -> Self {
        Self::new(
            "unbound-type-variable",
            "error",
            path,
            format!("type variable {} has no enclosing forall", index),
        )
    }

    pub fn type_arity_mismatch(path: &[usize], name: &str, expected: usize, got: usize) -> Self {
        Self::new(
            "type-arity-mismatch",
            "error",
            path,
            format!(
                "type constructor '{}' expects {} argument(s), got {}",
                name, expected, got
            ),
        )
    }

    pub fn unresolved_type(path: &[usize], name: &str) -> Self {
        let msg = if RESERVED_TYPE_NAMES.contains(&name) {
            format!(
                "type '{}' is reserved for Phase 3+, not yet implemented",
                name
            )
        } else {
            format!("unknown type '{}'", name)
        };
        Self::new("unresolved-type", "error", path, msg)
    }

    pub fn module_missing_annotation(path: &[usize], binding_idx: usize) -> Self {
        Self::new(
            "module-missing-annotation",
            "warning",
            path,
            format!(
                "exported binding {} has no type+effect signature",
                binding_idx
            ),
        )
    }

    pub fn operator_overload_failure(path: &[usize], op: &str, left: &Ty, right: &Ty) -> Self {
        let mut d = Self::new(
            "operator-overload-failure",
            "error",
            path,
            format!(
                "operator '{}' cannot be applied to operands of types {} and {}",
                op, left, right
            ),
        );
        d.expected = Some(ty_to_json(left));
        d.actual = Some(ty_to_json(right));
        d
    }

    pub fn buf_escape(path: &[usize]) -> Self {
        Self::new(
            "buf-escape",
            "error",
            path,
            "buffer handle used outside its let scope".to_string(),
        )
    }

    pub fn hole_diagnostic(path: &[usize], diag_id: &str, message: &str) -> Self {
        Self::new(diag_id, "error", path, message.to_string())
    }

    pub fn effect_violation(path: &[usize], declared: &str, inferred: &str) -> Self {
        Self::new(
            "effect-violation",
            "error",
            path,
            format!(
                "inferred effect set {{{}}} is not a subset of declared {{{}}}",
                inferred, declared
            ),
        )
    }

    pub fn unbound_effect_variable(path: &[usize], index: u64) -> Self {
        Self::new(
            "unbound-effect-variable",
            "error",
            path,
            format!("effect variable {} has no enclosing forall", index),
        )
    }

    pub fn effect_set_mismatch(path: &[usize], expected: &EffSet, actual: &EffSet) -> Self {
        let mut d = Self::new(
            "effect-violation",
            "error",
            path,
            format!("effect set mismatch: expected {}, got {}", expected, actual),
        );
        d.expected = Some(eff_set_to_json(expected));
        d.actual = Some(eff_set_to_json(actual));
        d
    }
}

/// Reserved type names that Phase 3+ will implement (ADR 0042).
const RESERVED_TYPE_NAMES: &[&str] = &["Int32", "Int16", "Int8", "Int64", "Nat", "Float64"];

#[derive(Debug, Clone, Serialize)]
pub struct Location {
    pub ast_path: Vec<PathStep>,
    pub source_span: Option<SourceSpan>,
}

impl Location {
    pub fn from_path(path: &[usize]) -> Self {
        Location {
            ast_path: path.iter().map(|&c| PathStep { child: c }).collect(),
            source_span: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PathStep {
    pub child: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceSpan {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Fix {
    pub description: String,
    pub edits: Vec<Edit>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Edit {
    pub location: Location,
    pub replacement: String,
}

/// Convert a `Ty` to the JSON representation specified by ADR 0041.
pub fn ty_to_json(ty: &Ty) -> Value {
    match ty {
        Ty::Int => serde_json::json!({"sym": "Int"}),
        Ty::Bool => serde_json::json!({"sym": "Bool"}),
        Ty::Str => serde_json::json!({"sym": "Str"}),
        Ty::Fn(a, b, eff) => serde_json::json!({
            "fn-ty": {
                "arg": ty_to_json(a),
                "ret": ty_to_json(b),
                "eff": fn_eff_to_json(eff),
            }
        }),
        Ty::Record(fields) => {
            let pairs: Vec<Value> = fields
                .iter()
                .map(|(k, v)| Value::Array(vec![Value::String(k.clone()), ty_to_json(v)]))
                .collect();
            serde_json::json!({"record": pairs})
        }
        Ty::App(f, a) => serde_json::json!({
            "app": {"fn": ty_to_json(f), "arg": ty_to_json(a)}
        }),
        Ty::Meta(id) => serde_json::json!({"meta": id}),
        Ty::Unknown => Value::Null,
        Ty::Buf => serde_json::json!({"sym": "Buf"}),
    }
}

/// Convert a `FnEff` to JSON.
pub fn fn_eff_to_json(eff: &FnEff) -> Value {
    match eff {
        FnEff::Concrete(set) => eff_set_to_json(set),
        FnEff::Meta(id) => serde_json::json!({"eff-meta": id}),
    }
}

/// Convert an `EffSet` to JSON: an array of sorted atom strings.
pub fn eff_set_to_json(eff: &EffSet) -> Value {
    let atoms: Vec<Value> = eff
        .atoms
        .iter()
        .map(|a| Value::String(a.to_string()))
        .collect();
    Value::Array(atoms)
}
