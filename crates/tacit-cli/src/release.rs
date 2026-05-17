use std::env;
use std::fs;
use std::path::PathBuf;

use serde::Serialize;

const ASSET_ROOT_ENV: &str = "TACIT_TOOLCHAIN_ASSET_ROOT";

pub const TOOLCHAIN_VERSION: &str = env!("TACIT_TOOLCHAIN_VERSION");
pub const EMBEDDED_MANIFEST_BYTES: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/toolchain-release.json"));
pub const EMBEDDED_MANIFEST_TEXT: &str =
    include_str!(concat!(env!("OUT_DIR"), "/toolchain-release.json"));

#[derive(Serialize)]
pub struct VersionEnvelope {
    pub format: &'static str,
    pub toolchain_version: &'static str,
    pub release_hash: String,
    pub manifest: serde_json::Value,
    pub installed_manifest: InstalledManifest,
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
