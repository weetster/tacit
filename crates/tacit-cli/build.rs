use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use tacit_canonical::ast::Node;

const SCHEMA_KEYS: [&str; 7] = [
    "canonical",
    "lockfile",
    "package",
    "test_results",
    "interface",
    "toolchain_release",
    "toolchain_pin",
];

const STDLIB_PACKAGES: [&str; 6] = ["core", "bytes", "array", "text", "collections", "io"];

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
    let primer_source_path = workspace_root.join(&metadata.primer_source_path);

    println!(
        "cargo:rustc-env=TACIT_TOOLCHAIN_VERSION={}",
        metadata.toolchain_version
    );
    println!("cargo:rerun-if-changed={}", primer_source_path.display());

    let git_rev = env::var("TACIT_GIT_REV")
        .unwrap_or_else(|_| git_rev(&workspace_root).unwrap_or("unknown".into()));
    let codegen = env::var_os("CARGO_FEATURE_LLVM").is_some();
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let primer = stage_primer_asset(&out_dir, &metadata, &primer_source_path);
    let workflow = stage_workflow_asset(&out_dir, &metadata, &workspace_root);
    let stdlib = stage_stdlib_assets(&out_dir, &workspace_root);
    let manifest = render_manifest(&metadata, &primer, &workflow, &stdlib, &git_rev, codegen);

    println!("cargo:rustc-env=TACIT_PRIMER_ID={}", metadata.primer_id);
    println!(
        "cargo:rustc-env=TACIT_PRIMER_VERSION={}",
        metadata.primer_version
    );
    println!(
        "cargo:rustc-env=TACIT_PRIMER_TOOLCHAIN_VERSION={}",
        metadata.primer_toolchain_version
    );
    println!("cargo:rustc-env=TACIT_PRIMER_HASH={}", primer.hash);
    println!(
        "cargo:rustc-env=TACIT_PRIMER_TOKENS={}",
        metadata.primer_tokens
    );
    println!(
        "cargo:rustc-env=TACIT_PRIMER_TOKENIZER={}",
        metadata.primer_tokenizer
    );
    println!("cargo:rustc-env=TACIT_PRIMER_PATH={}", metadata.primer_path);
    println!(
        "cargo:rustc-env=TACIT_PRIMER_METADATA_PATH={}",
        metadata.primer_metadata_path
    );

    println!(
        "cargo:rustc-env=TACIT_WORKFLOW_PATH={}",
        metadata.workflow_path
    );
    println!("cargo:rustc-env=TACIT_WORKFLOW_HASH={}", workflow.hash);
    fs::write(out_dir.join("toolchain-release.json"), &manifest).expect("write release manifest");
    let share_manifest_path = out_dir.join("share/tacit/toolchain-release.json");
    if let Some(parent) = share_manifest_path.parent() {
        fs::create_dir_all(parent).expect("create share/tacit asset directory");
    }
    fs::write(share_manifest_path, manifest).expect("write share/tacit release manifest");
}

#[derive(Default)]
struct ReleaseMetadata {
    format: String,
    toolchain_version: String,
    primer_id: String,
    primer_version: String,
    primer_toolchain_version: String,
    primer_source_path: String,
    primer_path: String,
    primer_metadata_path: String,
    primer_tokenizer: String,
    primer_tokens: u64,
    workflow_source_path: String,
    workflow_path: String,
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

            let (key, raw_value) = line
                .split_once('=')
                .ok_or_else(|| format!("line {line_number}: expected key = \"value\""))?;
            let key = key.trim();
            let value = parse_value(raw_value.trim())
                .ok_or_else(|| format!("line {line_number}: expected TOML scalar value"))?;

            match (section.as_str(), key, value) {
                ("", "format", MetadataValue::String(value)) => metadata.format = value,
                ("toolchain", "version", MetadataValue::String(value)) => {
                    metadata.toolchain_version = value;
                }
                ("primer", "id", MetadataValue::String(value)) => metadata.primer_id = value,
                ("primer", "version", MetadataValue::String(value)) => {
                    metadata.primer_version = value;
                }
                ("primer", "toolchain_version", MetadataValue::String(value)) => {
                    metadata.primer_toolchain_version = value;
                }
                ("primer", "source_path", MetadataValue::String(value)) => {
                    metadata.primer_source_path = value;
                }
                ("primer", "path", MetadataValue::String(value)) => metadata.primer_path = value,
                ("primer", "metadata_path", MetadataValue::String(value)) => {
                    metadata.primer_metadata_path = value;
                }
                ("primer", "tokenizer", MetadataValue::String(value)) => {
                    metadata.primer_tokenizer = value;
                }
                ("primer", "tokens", MetadataValue::Integer(value)) => {
                    metadata.primer_tokens = value;
                }
                ("workflow", "source_path", MetadataValue::String(value)) => {
                    metadata.workflow_source_path = value;
                }
                ("workflow", "path", MetadataValue::String(value)) => {
                    metadata.workflow_path = value;
                }
                ("llvm", "feature", MetadataValue::String(value)) => metadata.llvm_feature = value,
                ("llvm", "version", MetadataValue::String(value)) => metadata.llvm_version = value,
                ("schemas", _, value) => {
                    let MetadataValue::String(value) = value else {
                        return Err(format!(
                            "line {line_number}: schemas.{key} must be a string"
                        ));
                    };
                    metadata.schemas.insert(key.to_string(), value);
                }
                ("distribution", "kind", MetadataValue::String(value)) => {
                    metadata.distribution_kind = value;
                }
                ("distribution", "layout", MetadataValue::String(value)) => {
                    metadata.distribution_layout = value;
                }
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
        require_present("primer.id", &self.primer_id)?;
        require_present("primer.version", &self.primer_version)?;
        require_eq(
            "primer.version",
            &self.primer_version,
            &self.toolchain_version,
        )?;
        require_present("primer.toolchain_version", &self.primer_toolchain_version)?;
        require_eq(
            "primer.toolchain_version",
            &self.primer_toolchain_version,
            &self.toolchain_version,
        )?;
        require_present("primer.source_path", &self.primer_source_path)?;
        require_present("primer.path", &self.primer_path)?;
        require_present("primer.metadata_path", &self.primer_metadata_path)?;
        require_present("primer.tokenizer", &self.primer_tokenizer)?;
        if self.primer_tokens == 0 {
            return Err("missing primer.tokens".to_string());
        }
        require_present("workflow.source_path", &self.workflow_source_path)?;
        require_present("workflow.path", &self.workflow_path)?;
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

enum MetadataValue {
    String(String),
    Integer(u64),
}

fn parse_value(value: &str) -> Option<MetadataValue> {
    if let Some(value) = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    {
        return Some(MetadataValue::String(value.to_string()));
    }
    value.parse().ok().map(MetadataValue::Integer)
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

struct PrimerAsset {
    hash: String,
}

struct WorkflowAsset {
    hash: String,
}

struct StdlibPackageAsset {
    short_name: String,
    name: String,
    version: String,
    hash: String,
    unit_hash: String,
    source_path: String,
    manifest_path: String,
    cache_path: String,
    public_exports: BTreeMap<String, String>,
}

struct ParsedStdlibManifest {
    name: String,
    version: String,
    exports: BTreeMap<String, String>,
}

fn stage_primer_asset(
    out_dir: &Path,
    metadata: &ReleaseMetadata,
    source_path: &Path,
) -> PrimerAsset {
    let primer_bytes =
        fs::read(source_path).unwrap_or_else(|err| panic!("{}: {}", source_path.display(), err));
    let hash = blake3_prefixed(&primer_bytes);
    let primer_path = out_dir.join(&metadata.primer_path);
    let primer_metadata_path = out_dir.join(&metadata.primer_metadata_path);
    let primer_dir = primer_path.parent().expect("primer path has parent");
    fs::create_dir_all(primer_dir).expect("create primer asset directory");
    fs::write(&primer_path, &primer_bytes).expect("write primer asset");
    fs::write(
        &primer_metadata_path,
        render_primer_metadata(metadata, &hash),
    )
    .expect("write primer metadata");
    PrimerAsset { hash }
}

fn stage_workflow_asset(
    out_dir: &Path,
    metadata: &ReleaseMetadata,
    workspace_root: &Path,
) -> WorkflowAsset {
    let source_path = workspace_root.join(&metadata.workflow_source_path);
    println!("cargo:rerun-if-changed={}", source_path.display());
    let bytes =
        fs::read(&source_path).unwrap_or_else(|err| panic!("{}: {}", source_path.display(), err));
    let hash = blake3_prefixed(&bytes);
    let workflow_path = out_dir.join(&metadata.workflow_path);
    if let Some(parent) = workflow_path.parent() {
        fs::create_dir_all(parent).expect("create workflow asset directory");
    }
    fs::write(&workflow_path, &bytes).expect("write workflow asset");
    WorkflowAsset { hash }
}

fn stage_stdlib_assets(out_dir: &Path, workspace_root: &Path) -> Vec<StdlibPackageAsset> {
    let mut packages = Vec::new();
    for short_name in STDLIB_PACKAGES {
        let package_root = workspace_root.join("stdlib/tacit").join(short_name);
        let manifest_path = package_root.join("tacit.toml");
        let unit_path = package_root.join("src/lib.tac");
        println!("cargo:rerun-if-changed={}", manifest_path.display());
        println!("cargo:rerun-if-changed={}", unit_path.display());

        let manifest_text = fs::read_to_string(&manifest_path)
            .unwrap_or_else(|err| panic!("{}: {}", manifest_path.display(), err));
        let manifest = parse_stdlib_manifest(&manifest_text, &manifest_path);
        let source_bytes =
            fs::read(&unit_path).unwrap_or_else(|err| panic!("{}: {}", unit_path.display(), err));
        let node = tacit_canonical::parse(&source_bytes)
            .unwrap_or_else(|err| panic!("{}: {}", unit_path.display(), err));
        let canonical_bytes = tacit_canonical::emit(&node);
        let unit_hash = hash_hex_bytes(&canonical_bytes);
        let (definition_hashes, public_exports, package_exports) = unit_content_hashes(&node);
        let package_hash = graph_hash_with_tag("tacit-package-v1", [&unit_hash]);

        let expected_name = format!("tacit.{short_name}");
        if manifest.name != expected_name {
            panic!(
                "{}: expected [package].name = {:?}, got {:?}",
                manifest_path.display(),
                expected_name,
                manifest.name
            );
        }
        for (alias, hash) in &manifest.exports {
            if !public_exports.contains(hash) {
                panic!(
                    "{}: [exports].{} names blake3:{} which is not a public export",
                    manifest_path.display(),
                    alias,
                    hash
                );
            }
        }

        let cache_package_dir = out_dir
            .join("share/tacit/stdlib-cache/packages")
            .join(&package_hash);
        write_asset(
            &out_dir
                .join("share/tacit/stdlib-cache/objects/units")
                .join(format!("{unit_hash}.tac")),
            &canonical_bytes,
        );
        if let Node::Unit { defs, .. } = &node {
            for def in defs {
                let bytes = tacit_canonical::emit(def);
                let hash = hash_hex_bytes(&bytes);
                if !definition_hashes.contains(&hash) {
                    panic!("internal stdlib definition hash tracking mismatch");
                }
                write_asset(
                    &out_dir
                        .join("share/tacit/stdlib-cache/objects/defs")
                        .join(format!("{hash}.tac")),
                    &bytes,
                );
            }
        }
        write_asset(
            &cache_package_dir.join("package.json"),
            render_package_index(
                &package_hash,
                [&unit_hash],
                &public_exports,
                &package_exports,
            )
            .as_bytes(),
        );
        write_asset(
            &cache_package_dir.join("manifest.toml"),
            manifest_text.as_bytes(),
        );

        let release_source_dir = format!("share/tacit/stdlib-src/tacit/{short_name}");
        write_asset(
            &out_dir.join(&release_source_dir).join("tacit.toml"),
            manifest_text.as_bytes(),
        );
        write_asset(
            &out_dir.join(&release_source_dir).join("src/lib.tac"),
            &source_bytes,
        );

        packages.push(StdlibPackageAsset {
            short_name: short_name.to_string(),
            name: manifest.name,
            version: manifest.version,
            hash: blake3_prefix(&package_hash),
            unit_hash: blake3_prefix(&unit_hash),
            source_path: release_source_dir,
            manifest_path: format!("share/tacit/stdlib-src/tacit/{short_name}/tacit.toml"),
            cache_path: format!("share/tacit/stdlib-cache/packages/{package_hash}"),
            public_exports: manifest
                .exports
                .into_iter()
                .map(|(alias, hash)| (alias, blake3_prefix(&hash)))
                .collect(),
        });
    }
    packages
}

fn parse_stdlib_manifest(text: &str, path: &Path) -> ParsedStdlibManifest {
    let mut section = String::new();
    let mut name = String::new();
    let mut version = String::new();
    let mut exports = BTreeMap::new();

    for (line_index, raw_line) in text.lines().enumerate() {
        let line_number = line_index + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(section_name) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            section = section_name.trim().to_string();
            continue;
        }
        let (key, raw_value) = line
            .split_once('=')
            .unwrap_or_else(|| panic!("{}:{line_number}: expected key = value", path.display()));
        let key = key.trim();
        let value = parse_value(raw_value.trim()).unwrap_or_else(|| {
            panic!(
                "{}:{line_number}: expected TOML string or integer value",
                path.display()
            )
        });
        match (section.as_str(), key, value) {
            ("package", "name", MetadataValue::String(value)) => name = value,
            ("package", "version", MetadataValue::String(value)) => version = value,
            ("exports", alias, MetadataValue::String(value)) => {
                let hash = strip_blake3_hash(&value).unwrap_or_else(|| {
                    panic!(
                        "{}:{line_number}: [exports].{} must be blake3:<hash>",
                        path.display(),
                        alias
                    )
                });
                exports.insert(alias.to_string(), hash.to_string());
            }
            _ => {
                panic!(
                    "{}:{line_number}: unexpected key `{key}` in section `{section}`",
                    path.display()
                );
            }
        }
    }

    if name.is_empty() {
        panic!("{}: missing [package].name", path.display());
    }
    if version.is_empty() {
        panic!("{}: missing [package].version", path.display());
    }
    ParsedStdlibManifest {
        name,
        version,
        exports,
    }
}

fn unit_content_hashes(node: &Node) -> (BTreeSet<String>, BTreeSet<String>, BTreeSet<String>) {
    let Node::Unit { exports, defs, .. } = node else {
        panic!("stdlib source must be a unit artifact");
    };

    let definition_hashes: BTreeSet<String> = defs
        .iter()
        .map(|def| hash_hex_bytes(&tacit_canonical::emit(def)))
        .collect();
    let mut public_exports = BTreeSet::new();
    let mut package_exports = BTreeSet::new();
    for export in exports {
        let Node::Export { visibility, hash } = export else {
            panic!("unit export list contains a non-export node");
        };
        if !definition_hashes.contains(hash) {
            panic!("unit exports blake3:{hash} without a matching definition");
        }
        match visibility.as_str() {
            "public" => {
                public_exports.insert(hash.clone());
            }
            "package" => {
                package_exports.insert(hash.clone());
            }
            other => panic!("unsupported export visibility {other:?}"),
        }
    }
    (definition_hashes, public_exports, package_exports)
}

fn render_package_index<'a>(
    package_hash: &str,
    unit_hashes: impl IntoIterator<Item = &'a String>,
    public_exports: &BTreeSet<String>,
    package_exports: &BTreeSet<String>,
) -> String {
    let units: Vec<String> = unit_hashes
        .into_iter()
        .map(|hash| blake3_prefix(hash))
        .collect();
    format!(
        "{{\n  \"format\": \"tacit-package-v1\",\n  \"hash\": \"{}\",\n  \"units\": {},\n  \"public_exports\": {},\n  \"package_exports\": {}\n}}\n",
        blake3_prefix(package_hash),
        json_string_array(units.iter().map(String::as_str)),
        json_string_array(public_exports.iter().map(|hash| blake3_prefix(hash))),
        json_string_array(package_exports.iter().map(|hash| blake3_prefix(hash)))
    )
}

fn json_string_array<I, V>(values: I) -> String
where
    I: IntoIterator<Item = V>,
    V: AsRef<str>,
{
    let mut out = String::from("[");
    for (index, value) in values.into_iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        out.push('"');
        out.push_str(&json_escape(value.as_ref()));
        out.push('"');
    }
    out.push(']');
    out
}

fn graph_hash_with_tag<'a>(tag: &str, unit_hashes: impl IntoIterator<Item = &'a String>) -> String {
    let mut hashes: Vec<&String> = unit_hashes.into_iter().collect();
    hashes.sort();
    let mut bytes = tag.as_bytes().to_vec();
    bytes.push(b'\n');
    for hash in hashes {
        bytes.extend_from_slice(hash.as_bytes());
        bytes.push(b'\n');
    }
    hash_hex_bytes(&bytes)
}

fn hash_hex_bytes(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

fn blake3_prefix(hash: &str) -> String {
    format!("blake3:{hash}")
}

fn strip_blake3_hash(value: &str) -> Option<&str> {
    let raw = value.strip_prefix("blake3:")?;
    (raw.len() == 64 && raw.bytes().all(|byte| byte.is_ascii_hexdigit())).then_some(raw)
}

fn write_asset(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap_or_else(|err| panic!("{}: {}", parent.display(), err));
    }
    fs::write(path, bytes).unwrap_or_else(|err| panic!("{}: {}", path.display(), err));
}

fn render_primer_metadata(metadata: &ReleaseMetadata, hash: &str) -> String {
    format!(
        "id = \"{}\"\nversion = \"{}\"\ntoolchain_version = \"{}\"\nhash = \"{}\"\ntokenizer = \"{}\"\ntokens = {}\n",
        metadata.primer_id,
        metadata.primer_version,
        metadata.primer_toolchain_version,
        hash,
        metadata.primer_tokenizer,
        metadata.primer_tokens
    )
}

fn blake3_prefixed(bytes: &[u8]) -> String {
    format!("blake3:{}", blake3::hash(bytes).to_hex())
}

fn render_manifest(
    metadata: &ReleaseMetadata,
    primer: &PrimerAsset,
    workflow: &WorkflowAsset,
    stdlib: &[StdlibPackageAsset],
    git_rev: &str,
    codegen: bool,
) -> String {
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
    out.push_str("  \"stdlib\": {\n");
    for (index, package) in stdlib.iter().enumerate() {
        json_field(
            &mut out,
            2,
            &package.name,
            &package.hash,
            index + 1 != stdlib.len(),
        );
    }
    out.push_str("  },\n");
    out.push_str("  \"assets\": {\n");
    json_field(&mut out, 2, "root", "share/tacit", true);
    out.push_str("    \"primer\": {\n");
    json_field(&mut out, 3, "id", &metadata.primer_id, true);
    json_field(&mut out, 3, "version", &metadata.primer_version, true);
    json_field(
        &mut out,
        3,
        "toolchain_version",
        &metadata.primer_toolchain_version,
        true,
    );
    json_field(&mut out, 3, "path", &metadata.primer_path, true);
    json_field(
        &mut out,
        3,
        "metadata_path",
        &metadata.primer_metadata_path,
        true,
    );
    json_field(&mut out, 3, "hash", &primer.hash, true);
    json_field(&mut out, 3, "tokenizer", &metadata.primer_tokenizer, true);
    json_u64_field(&mut out, 3, "tokens", metadata.primer_tokens, false);
    out.push_str("    },\n");
    out.push_str("    \"workflow\": {\n");
    json_field(&mut out, 3, "path", &metadata.workflow_path, true);
    json_field(&mut out, 3, "hash", &workflow.hash, false);
    out.push_str("    },\n");
    out.push_str("    \"stdlib\": {\n");
    json_field(&mut out, 3, "cache_path", "share/tacit/stdlib-cache", true);
    json_field(
        &mut out,
        3,
        "source_path",
        "share/tacit/stdlib-src/tacit",
        true,
    );
    out.push_str("      \"packages\": [\n");
    for (package_index, package) in stdlib.iter().enumerate() {
        out.push_str("        {\n");
        json_field(&mut out, 5, "name", &package.name, true);
        json_field(&mut out, 5, "short_name", &package.short_name, true);
        json_field(&mut out, 5, "version", &package.version, true);
        json_field(&mut out, 5, "hash", &package.hash, true);
        json_field(&mut out, 5, "unit_hash", &package.unit_hash, true);
        json_field(&mut out, 5, "source_path", &package.source_path, true);
        json_field(&mut out, 5, "manifest_path", &package.manifest_path, true);
        json_field(&mut out, 5, "cache_path", &package.cache_path, true);
        out.push_str("          \"public_exports\": [\n");
        for (export_index, (alias, hash)) in package.public_exports.iter().enumerate() {
            out.push_str("            {\n");
            json_field(&mut out, 7, "alias", alias, true);
            json_field(&mut out, 7, "hash", hash, false);
            out.push_str("            }");
            if export_index + 1 != package.public_exports.len() {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str("          ]\n");
        out.push_str("        }");
        if package_index + 1 != stdlib.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("      ]\n");
    out.push_str("    }\n");
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

fn json_u64_field(out: &mut String, indent: usize, key: &str, value: u64, comma: bool) {
    json_indent(out, indent);
    out.push('"');
    out.push_str(&json_escape(key));
    out.push_str("\": ");
    out.push_str(&value.to_string());
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
