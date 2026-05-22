//! Project-level toolchain pin file (`tacit-toolchain.toml`).
//!
//! Implements the `tacit-toolchain-pin-v1` schema from ADR 0090 and enforces
//! present-but-mismatched pins as hard errors. A missing pin is a warning for
//! the first export per ADR 0090's "Missing pin behavior" section.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::json;
use tacit_typecheck::error::{Edit, Fix, Location, SourceSpan};
use tacit_typecheck::Diagnostic;

use crate::release;

pub const PIN_FILE_NAME: &str = "tacit-toolchain.toml";
pub const PIN_SCHEMA: &str = "tacit-toolchain-pin-v1";

pub fn pin_path(root: &Path) -> PathBuf {
    root.join(PIN_FILE_NAME)
}

pub fn render_toolchain_pin(release_hash: &str, stdlib: &release::StdlibListEnvelope) -> String {
    let mut out = String::new();
    out.push_str("format = \"tacit-toolchain-pin-v1\"\n\n");
    out.push_str("[toolchain]\n");
    out.push_str(&format!("version = \"{}\"\n", release::TOOLCHAIN_VERSION));
    out.push_str(&format!("release_hash = \"{}\"\n\n", release_hash));
    out.push_str("[primer]\n");
    out.push_str(&format!("id = \"{}\"\n", release::PRIMER_ID));
    out.push_str(&format!("version = \"{}\"\n", release::PRIMER_VERSION));
    out.push_str(&format!(
        "toolchain_version = \"{}\"\n",
        release::PRIMER_TOOLCHAIN_VERSION
    ));
    out.push_str(&format!("hash = \"{}\"\n\n", release::PRIMER_HASH));
    out.push_str("[stdlib]\n");
    for package in &stdlib.packages {
        out.push_str(&format!(
            "\"{}\" = \"{}\"\n",
            toml_escape(&package.name),
            package.hash
        ));
    }
    out
}

#[derive(Debug, Clone)]
pub struct ToolchainPin {
    pub toolchain_version: String,
    pub release_hash: String,
    pub primer_id: String,
    pub primer_version: String,
    pub primer_toolchain_version: String,
    pub primer_hash: String,
    pub stdlib: BTreeMap<String, String>,
}

/// Enforce the project's toolchain pin against the installed release metadata.
///
/// Returns `Ok(())` when the pin file is absent (after emitting a warning) or
/// when a present pin matches the installed toolchain. Returns `Err` when the
/// pin file is malformed or mismatched.
pub fn enforce_pin(root: &Path) -> Result<(), Vec<Diagnostic>> {
    let path = pin_path(root);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            eprintln!(
                "warning: {}: missing tacit-toolchain.toml; reproducibility checks skipped (see ADR 0090)",
                path.display()
            );
            return Ok(());
        }
        Err(error) => {
            return Err(vec![pin_diag(
                "toolchain-pin-unreadable",
                format!("{}: {}", path.display(), error),
                None,
            )]);
        }
    };

    let text = std::str::from_utf8(&bytes).map_err(|error| {
        vec![pin_diag(
            "toolchain-pin-malformed",
            format!("{}: pin file is not valid UTF-8: {}", path.display(), error),
            None,
        )]
    })?;

    let pin = parse_pin(text, &path)?;
    let release = release::version_envelope().map_err(|error| {
        vec![pin_diag(
            "toolchain-pin-release-unreadable",
            format!("could not read installed release metadata: {}", error),
            None,
        )]
    })?;
    let stdlib = release::stdlib_list()?;
    let diags = validate_pin(&pin, text, &release, &stdlib, &path);
    if diags.is_empty() {
        Ok(())
    } else {
        Err(diags)
    }
}

fn validate_pin(
    pin: &ToolchainPin,
    text: &str,
    release: &release::VersionEnvelope,
    stdlib: &release::StdlibListEnvelope,
    path: &Path,
) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let repin_fix = pin_repin_fix(text, release, stdlib);

    if pin.toolchain_version != release.toolchain_version {
        diags.push(pin_mismatch(
            "toolchain-pin-version-mismatch",
            "toolchain.version",
            &pin.toolchain_version,
            release.toolchain_version,
            path,
            Some(repin_fix.clone()),
        ));
    }
    if pin.release_hash != release.release_hash {
        diags.push(pin_mismatch(
            "toolchain-pin-release-hash-mismatch",
            "toolchain.release_hash",
            &pin.release_hash,
            &release.release_hash,
            path,
            Some(repin_fix.clone()),
        ));
    }

    let primer = release::primer_metadata();
    if pin.primer_id != primer.id {
        diags.push(pin_mismatch(
            "toolchain-pin-primer-mismatch",
            "primer.id",
            &pin.primer_id,
            primer.id,
            path,
            Some(repin_fix.clone()),
        ));
    }
    if pin.primer_version != primer.version {
        diags.push(pin_mismatch(
            "toolchain-pin-primer-mismatch",
            "primer.version",
            &pin.primer_version,
            primer.version,
            path,
            Some(repin_fix.clone()),
        ));
    }
    if pin.primer_toolchain_version != primer.toolchain_version {
        diags.push(pin_mismatch(
            "toolchain-pin-primer-mismatch",
            "primer.toolchain_version",
            &pin.primer_toolchain_version,
            primer.toolchain_version,
            path,
            Some(repin_fix.clone()),
        ));
    }
    if pin.primer_hash != primer.hash {
        diags.push(pin_mismatch(
            "toolchain-pin-primer-mismatch",
            "primer.hash",
            &pin.primer_hash,
            primer.hash,
            path,
            Some(repin_fix.clone()),
        ));
    }

    let installed: BTreeMap<&str, &str> = stdlib
        .packages
        .iter()
        .map(|package| (package.name.as_str(), package.hash.as_str()))
        .collect();

    for (name, hash) in &pin.stdlib {
        match installed.get(name.as_str()) {
            Some(expected) if *expected == hash.as_str() => {}
            Some(expected) => {
                diags.push(pin_mismatch(
                    "toolchain-pin-stdlib-mismatch",
                    &format!("stdlib.\"{}\"", name),
                    hash,
                    expected,
                    path,
                    Some(repin_fix.clone()),
                ));
            }
            None => {
                diags.push(pin_diag(
                    "toolchain-pin-stdlib-unknown",
                    format!(
                        "{}: stdlib entry \"{}\" is not bundled with the installed toolchain",
                        path.display(),
                        name
                    ),
                    Some(json!({"field": format!("stdlib.\"{}\"", name), "path": path.display().to_string()})),
                ));
            }
        }
    }
    for installed_name in installed.keys() {
        if !pin.stdlib.contains_key(*installed_name) {
            diags.push(pin_diag(
                "toolchain-pin-stdlib-missing",
                format!(
                    "{}: pin is missing required stdlib entry \"{}\"",
                    path.display(),
                    installed_name
                ),
                Some(json!({
                    "field": format!("stdlib.\"{}\"", installed_name),
                    "path": path.display().to_string()
                })),
            ));
        }
    }

    diags
}

fn parse_pin(text: &str, path: &Path) -> Result<ToolchainPin, Vec<Diagnostic>> {
    let mut diags = Vec::new();
    let mut format_value: Option<String> = None;
    let mut section: Option<String> = None;
    let mut toolchain: BTreeMap<String, String> = BTreeMap::new();
    let mut primer: BTreeMap<String, String> = BTreeMap::new();
    let mut stdlib: BTreeMap<String, String> = BTreeMap::new();

    for (line_index, raw_line) in text.lines().enumerate() {
        let line_number = line_index + 1;
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix('[') {
            let Some(name) = rest.strip_suffix(']') else {
                diags.push(pin_parse_error(
                    path,
                    line_number,
                    "expected closing ']' on section header",
                ));
                continue;
            };
            let name = name.trim();
            if !matches!(name, "toolchain" | "primer" | "stdlib") {
                diags.push(pin_parse_error(
                    path,
                    line_number,
                    format!("unknown section [{}]", name),
                ));
                continue;
            }
            section = Some(name.to_string());
            continue;
        }
        let Some((raw_key, raw_value)) = line.split_once('=') else {
            diags.push(pin_parse_error(path, line_number, "expected `key = value`"));
            continue;
        };
        let raw_key = raw_key.trim();
        let raw_value = raw_value.trim();
        let (key, key_was_quoted) = match parse_key(raw_key) {
            Ok(parsed) => parsed,
            Err(error) => {
                diags.push(pin_parse_error(path, line_number, error));
                continue;
            }
        };
        let value = match parse_string_value(raw_value) {
            Ok(value) => value,
            Err(error) => {
                diags.push(pin_parse_error(path, line_number, error));
                continue;
            }
        };

        match section.as_deref() {
            None => {
                if key != "format" || key_was_quoted {
                    diags.push(pin_parse_error(
                        path,
                        line_number,
                        "only `format = \"tacit-toolchain-pin-v1\"` is allowed before any section",
                    ));
                    continue;
                }
                if format_value.is_some() {
                    diags.push(pin_parse_error(path, line_number, "duplicate `format` key"));
                    continue;
                }
                format_value = Some(value);
            }
            Some("toolchain") => {
                if key_was_quoted {
                    diags.push(pin_parse_error(
                        path,
                        line_number,
                        "quoted keys are not allowed in [toolchain]",
                    ));
                    continue;
                }
                if !matches!(key.as_str(), "version" | "release_hash") {
                    diags.push(pin_parse_error(
                        path,
                        line_number,
                        format!("unknown key `{}` in [toolchain]", key),
                    ));
                    continue;
                }
                if toolchain.insert(key.clone(), value).is_some() {
                    diags.push(pin_parse_error(
                        path,
                        line_number,
                        format!("duplicate key `{}` in [toolchain]", key),
                    ));
                }
            }
            Some("primer") => {
                if key_was_quoted {
                    diags.push(pin_parse_error(
                        path,
                        line_number,
                        "quoted keys are not allowed in [primer]",
                    ));
                    continue;
                }
                if !matches!(
                    key.as_str(),
                    "id" | "version" | "toolchain_version" | "hash"
                ) {
                    diags.push(pin_parse_error(
                        path,
                        line_number,
                        format!("unknown key `{}` in [primer]", key),
                    ));
                    continue;
                }
                if primer.insert(key.clone(), value).is_some() {
                    diags.push(pin_parse_error(
                        path,
                        line_number,
                        format!("duplicate key `{}` in [primer]", key),
                    ));
                }
            }
            Some("stdlib") => {
                if !key_was_quoted {
                    diags.push(pin_parse_error(
                        path,
                        line_number,
                        "stdlib keys must be quoted package names (see ADR 0090)",
                    ));
                    continue;
                }
                if !is_valid_hash(&value) {
                    diags.push(pin_parse_error(
                        path,
                        line_number,
                        format!("stdlib entry \"{}\" must be a blake3:<64-hex> hash", key),
                    ));
                    continue;
                }
                if stdlib.insert(key.clone(), value).is_some() {
                    diags.push(pin_parse_error(
                        path,
                        line_number,
                        format!("duplicate stdlib entry \"{}\"", key),
                    ));
                }
            }
            Some(other) => {
                diags.push(pin_parse_error(
                    path,
                    line_number,
                    format!("unexpected section [{}]", other),
                ));
            }
        }
    }

    match format_value.as_deref() {
        Some(value) if value == PIN_SCHEMA => {}
        Some(other) => diags.push(pin_diag(
            "toolchain-pin-schema-mismatch",
            format!(
                "{}: unsupported pin schema `{}`; expected `{}`",
                path.display(),
                other,
                PIN_SCHEMA
            ),
            Some(json!({"expected": PIN_SCHEMA, "actual": other})),
        )),
        None => diags.push(pin_diag(
            "toolchain-pin-schema-missing",
            format!(
                "{}: missing top-level `format = \"{}\"`",
                path.display(),
                PIN_SCHEMA
            ),
            None,
        )),
    }

    let toolchain_version = require_field(&toolchain, "toolchain.version", path, &mut diags);
    let release_hash = require_field(&toolchain, "toolchain.release_hash", path, &mut diags);
    if let Some(value) = release_hash.as_deref() {
        if !is_valid_hash(value) {
            diags.push(pin_diag(
                "toolchain-pin-malformed",
                format!(
                    "{}: toolchain.release_hash must be blake3:<64-hex>, got `{}`",
                    path.display(),
                    value
                ),
                None,
            ));
        }
    }

    let primer_id = require_field(&primer, "primer.id", path, &mut diags);
    let primer_version = require_field(&primer, "primer.version", path, &mut diags);
    let primer_toolchain_version =
        require_field(&primer, "primer.toolchain_version", path, &mut diags);
    let primer_hash = require_field(&primer, "primer.hash", path, &mut diags);
    if let Some(value) = primer_hash.as_deref() {
        if !is_valid_hash(value) {
            diags.push(pin_diag(
                "toolchain-pin-malformed",
                format!(
                    "{}: primer.hash must be blake3:<64-hex>, got `{}`",
                    path.display(),
                    value
                ),
                None,
            ));
        }
    }

    if !diags.is_empty() {
        return Err(diags);
    }

    Ok(ToolchainPin {
        toolchain_version: toolchain_version.expect("required field validated"),
        release_hash: release_hash.expect("required field validated"),
        primer_id: primer_id.expect("required field validated"),
        primer_version: primer_version.expect("required field validated"),
        primer_toolchain_version: primer_toolchain_version.expect("required field validated"),
        primer_hash: primer_hash.expect("required field validated"),
        stdlib,
    })
}

fn require_field(
    map: &BTreeMap<String, String>,
    name: &str,
    path: &Path,
    diags: &mut Vec<Diagnostic>,
) -> Option<String> {
    let key = name.rsplit_once('.').map(|(_, k)| k).unwrap_or(name);
    match map.get(key) {
        Some(value) => Some(value.clone()),
        None => {
            diags.push(pin_diag(
                "toolchain-pin-missing-field",
                format!("{}: missing required field `{}`", path.display(), name),
                Some(json!({"field": name})),
            ));
            None
        }
    }
}

fn parse_key(raw: &str) -> Result<(String, bool), String> {
    if let Some(rest) = raw.strip_prefix('"') {
        let key = rest
            .strip_suffix('"')
            .ok_or_else(|| "unterminated quoted key".to_string())?;
        if key.is_empty() {
            return Err("empty quoted key".to_string());
        }
        Ok((key.to_string(), true))
    } else {
        if raw.is_empty() {
            return Err("empty key".to_string());
        }
        if !raw
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
        {
            return Err(format!("invalid bare key `{}`", raw));
        }
        Ok((raw.to_string(), false))
    }
}

fn parse_string_value(raw: &str) -> Result<String, String> {
    let inner = raw
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .ok_or_else(|| format!("expected quoted string value, got `{}`", raw))?;
    if inner.contains('\\') || inner.contains('"') {
        return Err("pin values do not support TOML escapes".to_string());
    }
    Ok(inner.to_string())
}

fn pin_repin_fix(
    text: &str,
    release: &release::VersionEnvelope,
    stdlib: &release::StdlibListEnvelope,
) -> Fix {
    Fix {
        description: "Rewrite tacit-toolchain.toml to match the installed toolchain pin"
            .to_string(),
        edits: vec![Edit {
            location: Location {
                ast_path: Vec::new(),
                source_span: Some(SourceSpan {
                    start: 0,
                    end: text.len(),
                }),
            },
            replacement: render_toolchain_pin(&release.release_hash, stdlib),
        }],
    }
}

fn strip_comment(line: &str) -> &str {
    // A `#` outside a string starts a comment. The pin schema does not embed
    // `#` inside any string value, so this minimal rule is enough.
    let mut in_string = false;
    for (index, ch) in line.char_indices() {
        match ch {
            '"' => in_string = !in_string,
            '#' if !in_string => return &line[..index],
            _ => {}
        }
    }
    line
}

fn is_valid_hash(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("blake3:") else {
        return false;
    };
    rest.len() == 64 && rest.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn pin_parse_error(path: &Path, line: usize, message: impl Into<String>) -> Diagnostic {
    let message = message.into();
    pin_diag(
        "toolchain-pin-malformed",
        format!("{}:{}: {}", path.display(), line, message),
        Some(json!({"path": path.display().to_string(), "line": line})),
    )
}

fn pin_mismatch(
    kind: &str,
    field: &str,
    actual: &str,
    expected: &str,
    path: &Path,
    fix: Option<Fix>,
) -> Diagnostic {
    let mut diag = pin_diag(
        kind,
        format!(
            "{}: {} expected `{}` from installed toolchain but pin records `{}`",
            path.display(),
            field,
            expected,
            actual
        ),
        Some(json!({
            "field": field,
            "expected": expected,
            "actual": actual,
            "path": path.display().to_string(),
        })),
    );
    diag.fix = fix;
    diag
}

fn pin_diag(kind: &str, message: String, details: Option<serde_json::Value>) -> Diagnostic {
    let details = details.unwrap_or_else(|| json!({}));
    Diagnostic::package_error(kind, message, details)
}

fn toml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_path() -> PathBuf {
        PathBuf::from("tacit-toolchain.toml")
    }

    fn good_pin_text() -> String {
        "format = \"tacit-toolchain-pin-v1\"\n\n\
         [toolchain]\n\
         version = \"0.7.0\"\n\
         release_hash = \"blake3:0000000000000000000000000000000000000000000000000000000000000000\"\n\n\
         [primer]\n\
         id = \"tacit-lite\"\n\
         version = \"0.7.0\"\n\
         toolchain_version = \"0.7.0\"\n\
         hash = \"blake3:1111111111111111111111111111111111111111111111111111111111111111\"\n\n\
         [stdlib]\n\
         \"tacit.core\" = \"blake3:2222222222222222222222222222222222222222222222222222222222222222\"\n"
            .to_string()
    }

    #[test]
    fn parses_well_formed_pin() {
        let pin = parse_pin(&good_pin_text(), &dummy_path()).expect("parse");
        assert_eq!(pin.toolchain_version, "0.7.0");
        assert_eq!(pin.primer_id, "tacit-lite");
        assert_eq!(pin.stdlib.len(), 1);
        assert!(pin.stdlib.contains_key("tacit.core"));
    }

    #[test]
    fn rejects_wrong_schema_marker() {
        let text = good_pin_text().replace("tacit-toolchain-pin-v1", "tacit-toolchain-pin-v2");
        let err = parse_pin(&text, &dummy_path()).expect_err("should fail");
        assert!(err
            .iter()
            .any(|d| d.kind == "toolchain-pin-schema-mismatch"));
    }

    #[test]
    fn rejects_missing_required_fields() {
        let text = "format = \"tacit-toolchain-pin-v1\"\n[toolchain]\nversion = \"0.7.0\"\n";
        let err = parse_pin(text, &dummy_path()).expect_err("missing fields");
        let kinds: Vec<&str> = err.iter().map(|d| d.kind.as_str()).collect();
        assert!(kinds.contains(&"toolchain-pin-missing-field"));
    }

    #[test]
    fn rejects_unknown_section() {
        let mut text = good_pin_text();
        text.push_str("\n[bonus]\nfoo = \"bar\"\n");
        let err = parse_pin(&text, &dummy_path()).expect_err("unknown section");
        assert!(err.iter().any(|d| d.message.contains("unknown section")));
    }

    #[test]
    fn rejects_unknown_key_in_known_section() {
        let mut text = good_pin_text();
        text.push_str("[toolchain]\nflavour = \"vanilla\"\n");
        let err = parse_pin(&text, &dummy_path()).expect_err("unknown key");
        assert!(err
            .iter()
            .any(|d| d.message.contains("unknown key `flavour`")));
    }

    #[test]
    fn rejects_unquoted_stdlib_key() {
        let text = "format = \"tacit-toolchain-pin-v1\"\n\
                    [toolchain]\nversion = \"0.7.0\"\n\
                    release_hash = \"blake3:0000000000000000000000000000000000000000000000000000000000000000\"\n\
                    [primer]\nid = \"tacit-lite\"\nversion = \"0.7.0\"\ntoolchain_version = \"0.7.0\"\n\
                    hash = \"blake3:1111111111111111111111111111111111111111111111111111111111111111\"\n\
                    [stdlib]\n\
                    tacit-core = \"blake3:2222222222222222222222222222222222222222222222222222222222222222\"\n";
        let err = parse_pin(text, &dummy_path()).expect_err("unquoted stdlib key");
        assert!(
            err.iter()
                .any(|d| d.message.contains("stdlib keys must be quoted")),
            "{:?}",
            err.iter().map(|d| &d.message).collect::<Vec<_>>()
        );

        let dotted = "format = \"tacit-toolchain-pin-v1\"\n\
                      [toolchain]\nversion = \"0.7.0\"\n\
                      release_hash = \"blake3:0000000000000000000000000000000000000000000000000000000000000000\"\n\
                      [primer]\nid = \"tacit-lite\"\nversion = \"0.7.0\"\ntoolchain_version = \"0.7.0\"\n\
                      hash = \"blake3:1111111111111111111111111111111111111111111111111111111111111111\"\n\
                      [stdlib]\n\
                      tacit.core = \"blake3:2222222222222222222222222222222222222222222222222222222222222222\"\n";
        let err = parse_pin(dotted, &dummy_path()).expect_err("dotted bare key");
        assert!(
            err.iter().any(|d| d.message.contains("invalid bare key")),
            "{:?}",
            err.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn rejects_malformed_hash() {
        let text = good_pin_text().replace(
            "blake3:1111111111111111111111111111111111111111111111111111111111111111",
            "deadbeef",
        );
        let err = parse_pin(&text, &dummy_path()).expect_err("bad hash");
        assert!(err
            .iter()
            .any(|d| d.message.contains("primer.hash must be blake3")));
    }

    #[test]
    fn rejects_stdlib_with_non_blake3_value() {
        let text = good_pin_text().replace(
            "blake3:2222222222222222222222222222222222222222222222222222222222222222",
            "not-a-hash",
        );
        let err = parse_pin(&text, &dummy_path()).expect_err("bad stdlib hash");
        assert!(err.iter().any(|d| d.message.contains("must be a blake3")));
    }
}
