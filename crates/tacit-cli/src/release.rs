use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

use serde::Serialize;
use tacit_canonical::ast::Node;
use tacit_typecheck::{
    seed_package_cache, CachedPackage, DefinitionVisibility, Diagnostic, ProjectDefinition,
    ProjectUnit,
};

const ASSET_ROOT_ENV: &str = "TACIT_TOOLCHAIN_ASSET_ROOT";

pub const TOOLCHAIN_VERSION: &str = env!("TACIT_TOOLCHAIN_VERSION");
pub const PRIMER_ID: &str = env!("TACIT_PRIMER_ID");
pub const PRIMER_VERSION: &str = env!("TACIT_PRIMER_VERSION");
pub const PRIMER_TOOLCHAIN_VERSION: &str = env!("TACIT_PRIMER_TOOLCHAIN_VERSION");
pub const PRIMER_HASH: &str = env!("TACIT_PRIMER_HASH");
pub const PRIMER_TOKENIZER: &str = env!("TACIT_PRIMER_TOKENIZER");
pub const PRIMER_TOKENS: u64 = parse_u64(env!("TACIT_PRIMER_TOKENS"));
pub const PRIMER_PATH: &str = env!("TACIT_PRIMER_PATH");
pub const PRIMER_METADATA_PATH: &str = env!("TACIT_PRIMER_METADATA_PATH");
pub const EMBEDDED_MANIFEST_BYTES: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/toolchain-release.json"));
pub const EMBEDDED_MANIFEST_TEXT: &str =
    include_str!(concat!(env!("OUT_DIR"), "/toolchain-release.json"));
pub const PRIMER_BYTES: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/share/tacit/primer/tacit-lite.md"
));

const STDLIB_ASSETS: [StdlibSourceAsset; 6] = [
    StdlibSourceAsset {
        short_name: "core",
        manifest_bytes: include_bytes!("../../../stdlib/tacit/core/tacit.toml"),
        unit_bytes: include_bytes!("../../../stdlib/tacit/core/src/lib.tac"),
    },
    StdlibSourceAsset {
        short_name: "bytes",
        manifest_bytes: include_bytes!("../../../stdlib/tacit/bytes/tacit.toml"),
        unit_bytes: include_bytes!("../../../stdlib/tacit/bytes/src/lib.tac"),
    },
    StdlibSourceAsset {
        short_name: "array",
        manifest_bytes: include_bytes!("../../../stdlib/tacit/array/tacit.toml"),
        unit_bytes: include_bytes!("../../../stdlib/tacit/array/src/lib.tac"),
    },
    StdlibSourceAsset {
        short_name: "text",
        manifest_bytes: include_bytes!("../../../stdlib/tacit/text/tacit.toml"),
        unit_bytes: include_bytes!("../../../stdlib/tacit/text/src/lib.tac"),
    },
    StdlibSourceAsset {
        short_name: "collections",
        manifest_bytes: include_bytes!("../../../stdlib/tacit/collections/tacit.toml"),
        unit_bytes: include_bytes!("../../../stdlib/tacit/collections/src/lib.tac"),
    },
    StdlibSourceAsset {
        short_name: "io",
        manifest_bytes: include_bytes!("../../../stdlib/tacit/io/tacit.toml"),
        unit_bytes: include_bytes!("../../../stdlib/tacit/io/src/lib.tac"),
    },
];

struct StdlibSourceAsset {
    short_name: &'static str,
    manifest_bytes: &'static [u8],
    unit_bytes: &'static [u8],
}

#[derive(Serialize)]
pub struct VersionEnvelope {
    pub format: &'static str,
    pub toolchain_version: &'static str,
    pub release_hash: String,
    pub manifest: serde_json::Value,
    pub installed_manifest: InstalledManifest,
}

#[derive(Serialize)]
pub struct PrimerMetadata {
    pub format: &'static str,
    pub id: &'static str,
    pub version: &'static str,
    pub toolchain_version: &'static str,
    pub path: &'static str,
    pub metadata_path: &'static str,
    pub hash: &'static str,
    pub tokenizer: &'static str,
    pub tokens: u64,
}

#[derive(Serialize)]
pub struct PrimerCheckEnvelope {
    pub format: &'static str,
    pub path: String,
    pub expected_hash: &'static str,
    pub actual_hash: String,
    pub ok: bool,
}

#[derive(Serialize)]
pub struct StdlibListEnvelope {
    pub format: &'static str,
    pub toolchain_version: &'static str,
    pub cache_path: &'static str,
    pub source_path: &'static str,
    pub packages: Vec<StdlibPackageInfo>,
}

#[derive(Clone, Serialize)]
pub struct StdlibPackageInfo {
    pub name: String,
    pub short_name: String,
    pub version: String,
    pub hash: String,
    pub unit_hash: String,
    pub source_path: String,
    pub manifest_path: String,
    pub cache_path: String,
    pub public_exports: Vec<StdlibExportInfo>,
}

#[derive(Clone, Serialize)]
pub struct StdlibExportInfo {
    pub alias: String,
    pub hash: String,
}

#[derive(Serialize)]
pub struct StdlibSeedEnvelope {
    pub format: &'static str,
    pub toolchain_version: &'static str,
    pub root: String,
    pub cache_root: String,
    pub packages: Vec<StdlibPackageInfo>,
}

#[derive(Serialize)]
pub struct InstalledManifest {
    pub status: &'static str,
    pub path: Option<String>,
    pub hash: Option<String>,
    pub error: Option<String>,
}

struct ParsedStdlibManifest {
    name: String,
    version: String,
    exports: BTreeMap<String, String>,
}

struct BundledStdlibPackage {
    info: StdlibPackageInfo,
    cached: CachedPackage,
    manifest_text: String,
}

pub fn version_envelope() -> Result<VersionEnvelope, serde_json::Error> {
    let manifest = serde_json::from_str(EMBEDDED_MANIFEST_TEXT)?;
    Ok(VersionEnvelope {
        format: "tacit-version-v1",
        toolchain_version: TOOLCHAIN_VERSION,
        release_hash: blake3_prefixed(EMBEDDED_MANIFEST_BYTES),
        manifest,
        installed_manifest: installed_manifest(),
    })
}

pub fn primer_metadata() -> PrimerMetadata {
    PrimerMetadata {
        format: "tacit-primer-v1",
        id: PRIMER_ID,
        version: PRIMER_VERSION,
        toolchain_version: PRIMER_TOOLCHAIN_VERSION,
        path: PRIMER_PATH,
        metadata_path: PRIMER_METADATA_PATH,
        hash: PRIMER_HASH,
        tokenizer: PRIMER_TOKENIZER,
        tokens: PRIMER_TOKENS,
    }
}

pub fn primer_check(path: PathBuf) -> io::Result<PrimerCheckEnvelope> {
    let bytes = fs::read(&path)?;
    let actual_hash = blake3_prefixed(&bytes);
    let ok = actual_hash == PRIMER_HASH;
    Ok(PrimerCheckEnvelope {
        format: "tacit-primer-check-v1",
        path: path.display().to_string(),
        expected_hash: PRIMER_HASH,
        actual_hash,
        ok,
    })
}

pub fn stdlib_list() -> Result<StdlibListEnvelope, Vec<Diagnostic>> {
    let packages = bundled_stdlib_packages()?
        .into_iter()
        .map(|package| package.info)
        .collect();
    Ok(StdlibListEnvelope {
        format: "tacit-stdlib-v1",
        toolchain_version: TOOLCHAIN_VERSION,
        cache_path: "share/tacit/stdlib-cache",
        source_path: "share/tacit/stdlib-src/tacit",
        packages,
    })
}

pub fn seed_stdlib(root: PathBuf) -> Result<StdlibSeedEnvelope, Vec<Diagnostic>> {
    let packages = bundled_stdlib_packages()?;
    let mut infos = Vec::new();
    for package in &packages {
        seed_package_cache(&root, &package.cached, Some(&package.manifest_text))
            .map_err(|diag| vec![diag])?;
        infos.push(package.info.clone());
    }
    Ok(StdlibSeedEnvelope {
        format: "tacit-stdlib-seed-v1",
        toolchain_version: TOOLCHAIN_VERSION,
        cache_root: root.join(".tacit").join("cache").display().to_string(),
        root: root.display().to_string(),
        packages: infos,
    })
}

fn installed_manifest() -> InstalledManifest {
    let Some(asset_root) = asset_root() else {
        return InstalledManifest {
            status: "missing",
            path: None,
            hash: None,
            error: Some("could not resolve current executable path".to_string()),
        };
    };
    let path = asset_root.join("toolchain-release.json");
    match fs::read(&path) {
        Ok(bytes) => {
            let hash = blake3_prefixed(&bytes);
            let status = if bytes == EMBEDDED_MANIFEST_BYTES {
                "matched"
            } else {
                "mismatch"
            };
            InstalledManifest {
                status,
                path: Some(path.display().to_string()),
                hash: Some(hash),
                error: None,
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => InstalledManifest {
            status: "missing",
            path: Some(path.display().to_string()),
            hash: None,
            error: None,
        },
        Err(error) => InstalledManifest {
            status: "unreadable",
            path: Some(path.display().to_string()),
            hash: None,
            error: Some(error.to_string()),
        },
    }
}

fn bundled_stdlib_packages() -> Result<Vec<BundledStdlibPackage>, Vec<Diagnostic>> {
    STDLIB_ASSETS.iter().map(bundle_stdlib_package).collect()
}

fn bundle_stdlib_package(
    asset: &StdlibSourceAsset,
) -> Result<BundledStdlibPackage, Vec<Diagnostic>> {
    let manifest_text = std::str::from_utf8(asset.manifest_bytes)
        .map_err(|err| vec![stdlib_diag("stdlib-bundle-invalid", err.to_string())])?
        .to_string();
    let manifest = parse_stdlib_manifest(&manifest_text, asset.short_name)?;
    let node = tacit_canonical::parse(asset.unit_bytes).map_err(|err| {
        vec![stdlib_diag(
            "stdlib-bundle-invalid",
            format!(
                "stdlib package {} does not parse: {}",
                asset.short_name, err
            ),
        )]
    })?;
    let unit_bytes = tacit_canonical::emit(&node);
    let unit_hash = hex_hash_bytes(&unit_bytes);
    let Node::Unit { exports, defs, .. } = &node else {
        return Err(vec![stdlib_diag(
            "stdlib-bundle-invalid",
            format!("stdlib package {} is not a unit artifact", asset.short_name),
        )]);
    };

    let mut definition_hashes = BTreeSet::new();
    let mut definitions = BTreeMap::new();
    for def in defs {
        let hash = hex_hash_bytes(&tacit_canonical::emit(def));
        definition_hashes.insert(hash.clone());
        definitions.insert(
            hash.clone(),
            ProjectDefinition {
                hash,
                def: def.clone(),
                visibility: DefinitionVisibility::Private,
                unit_hashes: BTreeSet::from([unit_hash.clone()]),
            },
        );
    }

    let mut public_exports = BTreeSet::new();
    let mut package_exports = BTreeSet::new();
    for export in exports {
        let Node::Export { visibility, hash } = export else {
            return Err(vec![stdlib_diag(
                "stdlib-bundle-invalid",
                format!(
                    "stdlib package {} contains a non-export entry",
                    asset.short_name
                ),
            )]);
        };
        if !definition_hashes.contains(hash) {
            return Err(vec![stdlib_diag(
                "stdlib-bundle-invalid",
                format!(
                    "stdlib package {} exports blake3:{} without a matching definition",
                    asset.short_name, hash
                ),
            )]);
        }
        match visibility.as_str() {
            "public" => {
                public_exports.insert(hash.clone());
                if let Some(definition) = definitions.get_mut(hash) {
                    definition.visibility = DefinitionVisibility::Public;
                }
            }
            "package" => {
                package_exports.insert(hash.clone());
                if let Some(definition) = definitions.get_mut(hash) {
                    definition.visibility = DefinitionVisibility::Package;
                }
            }
            other => {
                return Err(vec![stdlib_diag(
                    "stdlib-bundle-invalid",
                    format!(
                        "stdlib package {} uses unsupported export visibility {}",
                        asset.short_name, other
                    ),
                )]);
            }
        }
    }

    for (alias, hash) in &manifest.exports {
        if !public_exports.contains(hash) {
            return Err(vec![stdlib_diag(
                "stdlib-bundle-invalid",
                format!(
                    "stdlib package {} manifest export {} names blake3:{} which is not public",
                    asset.short_name, alias, hash
                ),
            )]);
        }
    }

    let package_hash = package_hash_from_unit_hashes([unit_hash.as_str()]);
    let source_path = format!("share/tacit/stdlib-src/tacit/{}", asset.short_name);
    let manifest_path = format!("{source_path}/tacit.toml");
    let cache_path = format!("share/tacit/stdlib-cache/packages/{package_hash}");
    let public_export_infos = manifest
        .exports
        .iter()
        .map(|(alias, hash)| StdlibExportInfo {
            alias: alias.clone(),
            hash: prefixed(hash),
        })
        .collect();
    let info = StdlibPackageInfo {
        name: manifest.name,
        short_name: asset.short_name.to_string(),
        version: manifest.version,
        hash: prefixed(&package_hash),
        unit_hash: prefixed(&unit_hash),
        source_path,
        manifest_path,
        cache_path,
        public_exports: public_export_infos,
    };

    let unit = ProjectUnit {
        hash: unit_hash,
        node,
        sidecar: None,
        source_paths: vec![PathBuf::from(&info.source_path).join("src/lib.tac")],
        definition_hashes: definition_hashes.into_iter().collect(),
        public_exports,
        package_exports,
    };
    let cached = CachedPackage {
        hash: package_hash,
        units: vec![unit],
        definitions,
    };
    Ok(BundledStdlibPackage {
        info,
        cached,
        manifest_text,
    })
}

fn parse_stdlib_manifest(
    text: &str,
    short_name: &str,
) -> Result<ParsedStdlibManifest, Vec<Diagnostic>> {
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
        let Some((key, raw_value)) = line.split_once('=') else {
            return Err(vec![stdlib_diag(
                "stdlib-bundle-invalid",
                format!("stdlib package {short_name} line {line_number}: expected key = value"),
            )]);
        };
        let key = key.trim();
        let value = parse_toml_string(raw_value.trim()).ok_or_else(|| {
            vec![stdlib_diag(
                "stdlib-bundle-invalid",
                format!("stdlib package {short_name} line {line_number}: expected string value"),
            )]
        })?;
        match (section.as_str(), key) {
            ("package", "name") => name = value.to_string(),
            ("package", "version") => version = value.to_string(),
            ("exports", alias) => {
                let Some(hash) = strip_hash_prefix(value) else {
                    return Err(vec![stdlib_diag(
                        "stdlib-bundle-invalid",
                        format!("stdlib package {short_name} export {alias} must be blake3:<hash>"),
                    )]);
                };
                exports.insert(alias.to_string(), hash.to_string());
            }
            _ => {
                return Err(vec![stdlib_diag(
                    "stdlib-bundle-invalid",
                    format!(
                        "stdlib package {short_name} line {line_number}: unexpected key {key} in section {section}"
                    ),
                )]);
            }
        }
    }

    if name.is_empty() || version.is_empty() {
        return Err(vec![stdlib_diag(
            "stdlib-bundle-invalid",
            format!("stdlib package {short_name} is missing package metadata"),
        )]);
    }
    Ok(ParsedStdlibManifest {
        name,
        version,
        exports,
    })
}

fn parse_toml_string(value: &str) -> Option<&str> {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
}

fn strip_hash_prefix(value: &str) -> Option<&str> {
    let raw = value.strip_prefix("blake3:")?;
    (raw.len() == 64 && raw.bytes().all(|byte| byte.is_ascii_hexdigit())).then_some(raw)
}

fn package_hash_from_unit_hashes<'a>(unit_hashes: impl IntoIterator<Item = &'a str>) -> String {
    let mut hashes: Vec<_> = unit_hashes.into_iter().collect();
    hashes.sort();
    let mut bytes = b"tacit-package-v1\n".to_vec();
    for hash in hashes {
        bytes.extend_from_slice(hash.as_bytes());
        bytes.push(b'\n');
    }
    hex_hash_bytes(&bytes)
}

fn hex_hash_bytes(bytes: &[u8]) -> String {
    let hash = tacit_canonical::hash_bytes(bytes);
    let mut out = String::new();
    for byte in hash {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn prefixed(hash: &str) -> String {
    if hash.starts_with("blake3:") {
        hash.to_string()
    } else {
        format!("blake3:{hash}")
    }
}

fn stdlib_diag(kind: &str, message: String) -> Diagnostic {
    Diagnostic::package_error(kind, message, serde_json::json!({"asset": "stdlib-bundle"}))
}

fn asset_root() -> Option<PathBuf> {
    if let Ok(root) = env::var(ASSET_ROOT_ENV) {
        if !root.trim().is_empty() {
            return Some(PathBuf::from(root));
        }
    }
    let exe = env::current_exe().ok()?;
    let bin_dir = exe.parent()?;
    let prefix = bin_dir.parent()?;
    Some(prefix.join("share").join("tacit"))
}

fn blake3_prefixed(bytes: &[u8]) -> String {
    let hash = tacit_canonical::hash_bytes(bytes);
    let mut out = String::from("blake3:");
    for byte in hash {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

const fn parse_u64(value: &str) -> u64 {
    let bytes = value.as_bytes();
    let mut index = 0;
    let mut out = 0u64;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte < b'0' || byte > b'9' {
            panic!("invalid integer literal");
        }
        out = out * 10 + (byte - b'0') as u64;
        index += 1;
    }
    out
}
