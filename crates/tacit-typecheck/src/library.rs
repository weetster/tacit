//! Stage 11 host-library specification.
//!
//! Translates a checked package plus its Stage 10 host interface into a
//! codegen-friendly description: per-export expanded bodies (with host import
//! refs left in place), scalar ABI types, and the host-import callback table
//! layout described by ADR 0088.

#![allow(clippy::result_large_err)]

use std::collections::{BTreeMap, BTreeSet};

use tacit_canonical::ast::Node;
use tacit_canonical::hash_node;

use crate::error::Diagnostic;
use crate::interface::{generate_host_interface, HostInterface, HostTarget};
use crate::package::PackageGraph;
use crate::project::{project_definition_expression_with_leaves, ProjectEntryError};

/// One ABI-expressible scalar at the host boundary.
///
/// Stage 11 codegen restricts boundary types to scalars and the unit-like
/// empty record. Records, borrowed vectors, and other ABI-expressible shapes
/// remain valid in `interface.json` and the generated headers/bindings, but
/// the linkable artifact rejects them with `abi-library-unsupported-type`
/// until later codegen lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibScalar {
    Bool,
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    Int,
}

impl LibScalar {
    pub fn width_bits(self) -> u32 {
        match self {
            LibScalar::Bool | LibScalar::I8 | LibScalar::U8 => 8,
            LibScalar::I16 | LibScalar::U16 => 16,
            LibScalar::I32 | LibScalar::U32 => 32,
            LibScalar::I64 | LibScalar::U64 | LibScalar::Int => 64,
        }
    }

    pub fn is_signed(self) -> bool {
        matches!(
            self,
            LibScalar::I8 | LibScalar::I16 | LibScalar::I32 | LibScalar::I64 | LibScalar::Int
        )
    }

    pub fn name(self) -> &'static str {
        match self {
            LibScalar::Bool => "bool",
            LibScalar::I8 => "i8",
            LibScalar::U8 => "u8",
            LibScalar::I16 => "i16",
            LibScalar::U16 => "u16",
            LibScalar::I32 => "i32",
            LibScalar::U32 => "u32",
            LibScalar::I64 => "i64",
            LibScalar::U64 => "u64",
            LibScalar::Int => "i64",
        }
    }
}

/// Result type at the host boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibReturn {
    /// Empty record. Generated wrapper omits the out-parameter.
    Unit,
    /// Single scalar written through the out-parameter.
    Scalar(LibScalar),
}

#[derive(Debug, Clone)]
pub struct LibraryExport {
    /// Definition hash (no `blake3:` prefix).
    pub hash: String,
    /// Stable hash-based extern "C" symbol.
    pub symbol: String,
    /// Definition body with internal refs expanded inline; host-import refs
    /// are left as `Ref` nodes so codegen can lower them through the
    /// callback table.
    pub body: Node,
    pub params: Vec<LibScalar>,
    pub result: LibReturn,
}

#[derive(Debug, Clone)]
pub struct LibraryImport {
    /// Host import declaration hash.
    pub hash: String,
    /// Stable hash-based callback field/symbol per ADR 0088.
    pub callback: String,
    /// Sorted-by-hash index inside the callbacks struct. Determines the GEP
    /// offset codegen uses to load the function pointer.
    pub index: usize,
    pub params: Vec<LibScalar>,
    pub result: LibReturn,
}

#[derive(Debug, Clone)]
pub struct PackageLibrary {
    pub package_hash: String,
    /// Short identifier `tacit_p_<pkg64>` used for symbols and the per-package
    /// TLS context global.
    pub package_prefix: String,
    pub exports: Vec<LibraryExport>,
    pub imports: Vec<LibraryImport>,
}

pub fn package_library(
    package: &PackageGraph,
    target: HostTarget,
) -> Result<(HostInterface, PackageLibrary), Vec<Diagnostic>> {
    let interface = generate_host_interface(package, target)?;
    let package_prefix = format!("tacit_p_{}", short_hash(&interface.package_hash));

    let host_import_hashes: BTreeSet<String> = interface
        .imports
        .iter()
        .map(|import| strip_prefix(&import.hash))
        .collect();

    let mut diags: Vec<Diagnostic> = Vec::new();

    let mut import_index: BTreeMap<String, usize> = BTreeMap::new();
    let mut imports: Vec<LibraryImport> = Vec::new();
    for (index, import) in interface.imports.iter().enumerate() {
        let hash = strip_prefix(&import.hash);
        let params = match library_param_scalars(&import.parameters, &hash) {
            Ok(params) => params,
            Err(diag) => {
                diags.push(diag);
                continue;
            }
        };
        let result = match library_result(&import.result, &hash) {
            Ok(result) => result,
            Err(diag) => {
                diags.push(diag);
                continue;
            }
        };
        import_index.insert(hash.clone(), index);
        imports.push(LibraryImport {
            hash,
            callback: import.callback.clone(),
            index,
            params,
            result,
        });
    }

    let mut exports: Vec<LibraryExport> = Vec::new();
    for export in &interface.exports {
        let hash = strip_prefix(&export.hash);
        let params = match library_param_scalars(&export.parameters, &hash) {
            Ok(params) => params,
            Err(diag) => {
                diags.push(diag);
                continue;
            }
        };
        let result = match library_result(&export.result, &hash) {
            Ok(result) => result,
            Err(diag) => {
                diags.push(diag);
                continue;
            }
        };
        match expanded_export_body(package, &hash, &host_import_hashes) {
            Ok(body) => exports.push(LibraryExport {
                hash,
                symbol: export.symbol.clone(),
                body,
                params,
                result,
            }),
            Err(error) => diags.push(library_diag(
                "abi-library-expansion-failed",
                &hash,
                error.to_string(),
            )),
        }
    }

    if !diags.is_empty() {
        return Err(diags);
    }

    let package_hash = strip_prefix(&interface.package_hash);
    let library = PackageLibrary {
        package_hash,
        package_prefix,
        exports,
        imports,
    };
    Ok((interface, library))
}

fn library_param_scalars(
    abi_params: &[crate::interface::AbiType],
    owner_hash: &str,
) -> Result<Vec<LibScalar>, Diagnostic> {
    let mut params = Vec::with_capacity(abi_params.len());
    for abi in abi_params {
        let scalar = scalar_from_abi(abi).ok_or_else(|| {
            library_diag(
                "abi-library-unsupported-type",
                owner_hash,
                format!(
                    "stage 11 library codegen supports only scalar boundary types; got {}",
                    abi_kind_description(abi)
                ),
            )
        })?;
        params.push(scalar);
    }
    Ok(params)
}

fn library_result(
    abi: &crate::interface::AbiType,
    owner_hash: &str,
) -> Result<LibReturn, Diagnostic> {
    if abi.kind == "unit" {
        return Ok(LibReturn::Unit);
    }
    let scalar = scalar_from_abi(abi).ok_or_else(|| {
        library_diag(
            "abi-library-unsupported-type",
            owner_hash,
            format!(
                "stage 11 library codegen supports only scalar boundary returns; got {}",
                abi_kind_description(abi)
            ),
        )
    })?;
    Ok(LibReturn::Scalar(scalar))
}

fn scalar_from_abi(abi: &crate::interface::AbiType) -> Option<LibScalar> {
    if abi.kind != "scalar" {
        return None;
    }
    Some(match abi.name.as_deref()? {
        "bool" => LibScalar::Bool,
        "i8" => LibScalar::I8,
        "u8" => LibScalar::U8,
        "i16" => LibScalar::I16,
        "u16" => LibScalar::U16,
        "i32" => LibScalar::I32,
        "u32" => LibScalar::U32,
        "i64" => LibScalar::I64,
        "u64" => LibScalar::U64,
        _ => return None,
    })
}

fn abi_kind_description(abi: &crate::interface::AbiType) -> String {
    match abi.kind.as_str() {
        "scalar" => abi.name.clone().unwrap_or_else(|| "scalar".to_string()),
        "record" => "record".to_string(),
        "borrowed_vector" => "borrowed vector".to_string(),
        "unit" => "unit".to_string(),
        other => other.to_string(),
    }
}

fn expanded_export_body(
    package: &PackageGraph,
    hash: &str,
    host_import_hashes: &BTreeSet<String>,
) -> Result<Node, ProjectEntryError> {
    let mut graph = package.root.clone();
    graph.graph_hash = package.package_hash.clone();
    for dependency in &package.dependencies {
        for (def_hash, definition) in &dependency.definitions {
            graph
                .definitions
                .entry(def_hash.clone())
                .or_insert_with(|| definition.clone());
        }
    }
    let entry = project_definition_expression_with_leaves(&graph, hash, host_import_hashes)?;
    Ok(entry.expression)
}

fn short_hash(hash: &str) -> String {
    hash.trim_start_matches("blake3:")
        .chars()
        .take(16)
        .collect()
}

fn strip_prefix(hash: &str) -> String {
    hash.trim_start_matches("blake3:").to_string()
}

fn library_diag(kind: &str, hash: &str, message: String) -> Diagnostic {
    Diagnostic::package_error(
        kind,
        format!("{}: {}", prefixed(hash), message),
        serde_json::json!({"hash": prefixed(hash)}),
    )
}

fn prefixed(hash: &str) -> String {
    if hash.starts_with("blake3:") {
        hash.to_string()
    } else {
        format!("blake3:{hash}")
    }
}

#[allow(dead_code)]
fn hash_hex(node: &Node) -> String {
    hash_node(node)
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect()
}
