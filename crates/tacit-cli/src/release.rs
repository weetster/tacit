use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

use serde::Serialize;

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
pub struct InstalledManifest {
    pub status: &'static str,
    pub path: Option<String>,
    pub hash: Option<String>,
    pub error: Option<String>,
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
