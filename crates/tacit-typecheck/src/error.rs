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

    pub fn record_type_mismatch(path: &[usize], expected: &Ty, actual: &Ty) -> Self {
        let mut d = Self::new(
            "record-type-mismatch",
            "error",
            path,
            format!(
                "record type mismatch: expected {}, got {}",
                expected, actual
            ),
        );
        d.expected = Some(ty_to_json(expected));
        d.actual = Some(ty_to_json(actual));
        d
    }

    pub fn duplicate_record_field(path: &[usize], field: &str) -> Self {
        Self::new(
            "duplicate-field",
            "error",
            path,
            format!("record field '{}' is defined more than once", field),
        )
    }

    pub fn missing_record_field(path: &[usize], field: &str, record: &Ty) -> Self {
        let mut d = Self::new(
            "missing-field",
            "error",
            path,
            format!("record field '{}' does not exist on {}", field, record),
        );
        d.actual = Some(ty_to_json(record));
        d
    }

    pub fn invalid_projection(path: &[usize], field: &str, actual: &Ty) -> Self {
        let mut d = Self::new(
            "invalid-projection",
            "error",
            path,
            format!(
                "cannot project field '{}' from non-record type {}",
                field, actual
            ),
        );
        d.expected = Some(serde_json::json!({"record": []}));
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

    pub fn invalid_unit_artifact(path: &[usize]) -> Self {
        Self::new(
            "invalid-unit-artifact",
            "error",
            path,
            "expected canonical unit artifact".to_string(),
        )
    }

    pub fn module_missing_annotation(path: &[usize], binding_idx: usize) -> Self {
        Self::new(
            "module-missing-annotation",
            "warning",
            path,
            format!(
                "module binding {} has no type+effect signature",
                binding_idx
            ),
        )
    }

    pub fn missing_import(path: &[usize], hash: &str, alias: Option<&str>) -> Self {
        let mut d = Self::new(
            "missing-import",
            "error",
            path,
            format!(
                "imported definition {} cannot be resolved",
                hash_display(hash, alias)
            ),
        );
        d.expected = Some(hash_json(hash, alias));
        d
    }

    pub fn hash_mismatch(
        path: &[usize],
        expected: &str,
        actual: &str,
        alias: Option<&str>,
    ) -> Self {
        let mut d = Self::new(
            "hash-mismatch",
            "error",
            path,
            format!(
                "artifact hash mismatch: expected {}, got {}",
                hash_display(expected, alias),
                blake3_display(actual)
            ),
        );
        d.expected = Some(hash_json(expected, alias));
        d.actual = Some(serde_json::json!({"hash": actual}));
        d
    }

    pub fn signature_mismatch(path: &[usize], subject: &str, expected: &str, actual: &str) -> Self {
        let mut d = Self::new(
            "signature-mismatch",
            "error",
            path,
            format!(
                "{} signature mismatch: expected {}, got {}",
                subject, expected, actual
            ),
        );
        d.expected = Some(serde_json::json!({"signature": expected}));
        d.actual = Some(serde_json::json!({"signature": actual}));
        d
    }

    pub fn visibility_violation(
        path: &[usize],
        hash: &str,
        visibility: &str,
        alias: Option<&str>,
    ) -> Self {
        let mut d = Self::new(
            "visibility-violation",
            "error",
            path,
            format!(
                "definition {} is not importable with {} visibility",
                hash_display(hash, alias),
                visibility
            ),
        );
        d.actual = Some(match alias {
            Some(alias) => serde_json::json!({
                "hash": hash,
                "alias": alias,
                "visibility": visibility,
            }),
            None => serde_json::json!({
                "hash": hash,
                "visibility": visibility,
            }),
        });
        d
    }

    pub fn cyclic_dependency(path: &[usize], cycle: &[String]) -> Self {
        let mut d = Self::new(
            "cyclic-dependency",
            "error",
            path,
            format!("definition dependency cycle: {}", cycle.join(" -> ")),
        );
        d.actual = Some(serde_json::json!({"cycle": cycle}));
        d
    }

    pub fn duplicate_import(path: &[usize], hash: &str, alias: Option<&str>) -> Self {
        Self::new(
            "duplicate-import",
            "error",
            path,
            format!("unit imports {} more than once", hash_display(hash, alias)),
        )
    }

    pub fn duplicate_export(path: &[usize], hash: &str, alias: Option<&str>) -> Self {
        Self::new(
            "duplicate-export",
            "error",
            path,
            format!("unit exports {} more than once", hash_display(hash, alias)),
        )
    }

    pub fn dangling_export(path: &[usize], hash: &str, alias: Option<&str>) -> Self {
        Self::new(
            "dangling-export",
            "error",
            path,
            format!(
                "unit exports {} but no local def has that hash",
                hash_display(hash, alias)
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

    pub fn apply_non_function(path: &[usize], actual: &Ty) -> Self {
        let mut d = Self::new(
            "apply-non-function",
            "error",
            path,
            format!("cannot apply non-function value of type {}", actual),
        );
        d.expected = Some(serde_json::json!({"fn-ty": "function"}));
        d.actual = Some(ty_to_json(actual));
        d
    }

    pub fn invalid_capture(path: &[usize], index: u64, actual: &Ty) -> Self {
        let mut d = Self::new(
            "invalid-capture",
            "error",
            path,
            format!(
                "closure capture at DeBruijn index {} has non-capturable type {}",
                index, actual
            ),
        );
        d.expected = Some(serde_json::json!({"capturable": true}));
        d.actual = Some(serde_json::json!({
            "capture": {
                "index": index,
                "type": ty_to_json(actual),
            }
        }));
        d
    }

    pub fn callback_type_mismatch(
        path: &[usize],
        combinator: &str,
        expected: &Ty,
        actual: &Ty,
    ) -> Self {
        let mut d = Self::new(
            "callback-type-mismatch",
            "error",
            path,
            format!(
                "@{} callback type mismatch: expected {}, got {}",
                combinator, expected, actual
            ),
        );
        d.expected = Some(ty_to_json(expected));
        d.actual = Some(ty_to_json(actual));
        d
    }

    pub fn callback_effect_mismatch(
        path: &[usize],
        combinator: &str,
        expected: &EffSet,
        actual: &EffSet,
    ) -> Self {
        let mut d = Self::new(
            "callback-effect-mismatch",
            "error",
            path,
            format!(
                "@{} callback effect mismatch: expected {}, got {}",
                combinator, expected, actual
            ),
        );
        d.expected = Some(eff_set_to_json(expected));
        d.actual = Some(eff_set_to_json(actual));
        d
    }

    pub fn invalid_accumulator_shape(
        path: &[usize],
        combinator: &str,
        expected: &Ty,
        actual: &Ty,
    ) -> Self {
        let mut d = Self::new(
            "invalid-accumulator-shape",
            "error",
            path,
            format!(
                "@{} accumulator shape mismatch: expected {}, got {}",
                combinator, expected, actual
            ),
        );
        d.expected = Some(ty_to_json(expected));
        d.actual = Some(ty_to_json(actual));
        d
    }

    pub fn unsupported_collection_shape(
        path: &[usize],
        combinator: &str,
        expected: &Ty,
        actual: &Ty,
    ) -> Self {
        let mut d = Self::new(
            "unsupported-collection-shape",
            "error",
            path,
            format!(
                "@{} collection shape is unsupported: expected {}, got {}",
                combinator, expected, actual
            ),
        );
        d.expected = Some(ty_to_json(expected));
        d.actual = Some(ty_to_json(actual));
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

    pub fn package_error(kind: &str, message: String, details: Value) -> Self {
        let mut d = Self::new(kind, "error", &[], message);
        d.actual = Some(details);
        d
    }
}

fn blake3_display(hash: &str) -> String {
    format!("blake3:{}", hash)
}

fn hash_display(hash: &str, alias: Option<&str>) -> String {
    alias
        .map(|alias| format!("{} ({})", alias, blake3_display(hash)))
        .unwrap_or_else(|| blake3_display(hash))
}

fn hash_json(hash: &str, alias: Option<&str>) -> Value {
    match alias {
        Some(alias) => serde_json::json!({"hash": hash, "alias": alias}),
        None => serde_json::json!({"hash": hash}),
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
        Ty::Buf => serde_json::json!({"sym": "Buf"}),
        Ty::I64Vec => serde_json::json!({"sym": "I64Vec"}),
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
