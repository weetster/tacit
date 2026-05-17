use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const SCHEMA_KEYS: [&str; 7] = [
    "canonical",
    "lockfile",
    "package",
    "test_results",
    "interface",
    "toolchain_release",
    "toolchain_pin",
];

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("tacit-cli is under crates/")
        .to_path_buf();
    let metadata_path = workspace_root.join("tacit-toolchain-release.toml");

    println!("cargo:rerun-if-changed={}", metadata_path.display());
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=TACIT_GIT_REV");
    emit_git_rerun_instructions(&workspace_root);

    let metadata_text = fs::read_to_string(&metadata_path)
        .unwrap_or_else(|err| panic!("{}: {}", metadata_path.display(), err));
    let metadata = ReleaseMetadata::parse(&metadata_text)
        .unwrap_or_else(|err| panic!("{}: {}", metadata_path.display(), err));

    println!(
        "cargo:rustc-env=TACIT_TOOLCHAIN_VERSION={}",
        metadata.toolchain_version
    );

    let git_rev = env::var("TACIT_GIT_REV")
        .unwrap_or_else(|_| git_rev(&workspace_root).unwrap_or("unknown".into()));
    let codegen = env::var_os("CARGO_FEATURE_LLVM").is_some();
    let manifest = render_manifest(&metadata, &git_rev, codegen);

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    fs::write(out_dir.join("toolchain-release.json"), manifest).expect("write release manifest");
}

#[derive(Default)]
struct ReleaseMetadata {
    format: String,
    toolchain_version: String,
    llvm_feature: String,
    llvm_version: String,
    schemas: BTreeMap<String, String>,
    distribution_kind: String,
    distribution_layout: String,
}

impl ReleaseMetadata {
    fn parse(input: &str) -> Result<Self, String> {
        let mut metadata = ReleaseMetadata::default();
        let mut section = String::new();

        for (line_index, raw_line) in input.lines().enumerate() {
            let line_number = line_index + 1;
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                section = name.trim().to_string();
                continue;
            }

            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| format!("line {line_number}: expected key = \"value\""))?;
            let key = key.trim();
            let value = parse_quoted_value(value.trim())
                .ok_or_else(|| format!("line {line_number}: expected quoted string value"))?;

            match (section.as_str(), key) {
                ("", "format") => metadata.format = value,
                ("toolchain", "version") => metadata.toolchain_version = value,
                ("llvm", "feature") => metadata.llvm_feature = value,
                ("llvm", "version") => metadata.llvm_version = value,
                ("schemas", _) => {
                    metadata.schemas.insert(key.to_string(), value);
                }
                ("distribution", "kind") => metadata.distribution_kind = value,
                ("distribution", "layout") => metadata.distribution_layout = value,
                _ => {
                    return Err(format!(
                        "line {line_number}: unexpected key `{key}` in section `{section}`"
                    ));
                }
            }
        }

        metadata.validate()?;
        Ok(metadata)
    }

    fn validate(&self) -> Result<(), String> {
        require_eq(
            "format",
            &self.format,
            "tacit-toolchain-release-metadata-v1",
        )?;
        require_present("toolchain.version", &self.toolchain_version)?;
        require_present("llvm.feature", &self.llvm_feature)?;
        require_present("llvm.version", &self.llvm_version)?;
        require_present("distribution.kind", &self.distribution_kind)?;
        require_present("distribution.layout", &self.distribution_layout)?;
        for key in SCHEMA_KEYS {
            if !self.schemas.contains_key(key) {
                return Err(format!("missing schemas.{key}"));
            }
        }
        Ok(())
    }
}

fn parse_quoted_value(value: &str) -> Option<String> {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .map(|value| value.to_string())
}

fn require_present(name: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        Err(format!("missing {name}"))
    } else {
        Ok(())
    }
}

fn require_eq(name: &str, actual: &str, expected: &str) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        Err(format!("expected {name} = {expected:?}, got {actual:?}"))
    }
}

fn git_rev(workspace_root: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(workspace_root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let rev = String::from_utf8(output.stdout).ok()?.trim().to_string();
    if rev.is_empty() {
        None
    } else {
        Some(rev)
    }
}

fn emit_git_rerun_instructions(workspace_root: &Path) {
    let git_dir = workspace_root.join(".git");
    let head_path = git_dir.join("HEAD");
    println!("cargo:rerun-if-changed={}", head_path.display());
    println!(
        "cargo:rerun-if-changed={}",
        git_dir.join("packed-refs").display()
    );
    if let Ok(head) = fs::read_to_string(&head_path) {
        if let Some(reference) = head.trim().strip_prefix("ref: ") {
            println!(
                "cargo:rerun-if-changed={}",
                git_dir.join(reference).display()
            );
        }
    }
}

fn render_manifest(metadata: &ReleaseMetadata, git_rev: &str, codegen: bool) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    json_field(&mut out, 1, "format", "tacit-toolchain-release-v1", true);
    json_field(
        &mut out,
        1,
        "toolchain_version",
        &metadata.toolchain_version,
        true,
    );
    json_field(&mut out, 1, "git_rev", git_rev, true);
    out.push_str("  \"llvm\": {\n");
    json_field(&mut out, 2, "feature", &metadata.llvm_feature, true);
    json_field(&mut out, 2, "version", &metadata.llvm_version, true);
    json_bool_field(&mut out, 2, "codegen", codegen, false);
    out.push_str("  },\n");
    out.push_str("  \"schemas\": {\n");
    for (index, key) in SCHEMA_KEYS.iter().enumerate() {
        let value = metadata
            .schemas
            .get(*key)
            .expect("schema key validated before rendering");
        json_field(&mut out, 2, key, value, index + 1 != SCHEMA_KEYS.len());
    }
    out.push_str("  },\n");
    out.push_str("  \"distribution\": {\n");
    json_field(&mut out, 2, "kind", &metadata.distribution_kind, true);
    json_field(&mut out, 2, "layout", &metadata.distribution_layout, false);
    out.push_str("  }\n");
    out.push_str("}\n");
    out
}

fn json_field(out: &mut String, indent: usize, key: &str, value: &str, comma: bool) {
    json_indent(out, indent);
    out.push('"');
    out.push_str(&json_escape(key));
    out.push_str("\": \"");
    out.push_str(&json_escape(value));
    out.push('"');
    if comma {
        out.push(',');
    }
    out.push('\n');
}

fn json_bool_field(out: &mut String, indent: usize, key: &str, value: bool, comma: bool) {
    json_indent(out, indent);
    out.push('"');
    out.push_str(&json_escape(key));
    out.push_str("\": ");
    out.push_str(if value { "true" } else { "false" });
    if comma {
        out.push(',');
    }
    out.push('\n');
}

fn json_indent(out: &mut String, indent: usize) {
    for _ in 0..indent {
        out.push_str("  ");
    }
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            c if c.is_control() => escaped.push_str(&format!("\\u{:04x}", c as u32)),
            c => escaped.push(c),
        }
    }
    escaped
}
