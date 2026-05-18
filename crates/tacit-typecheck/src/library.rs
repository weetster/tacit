//! Host-library codegen specification.
//!
//! Translates a checked package plus its Stage 10 host interface into a
//! codegen-friendly description: per-export expanded bodies (with host import
//! refs left in place), ABI boundary types, and the host-import callback table
//! layout described by ADR 0088.

#![allow(clippy::result_large_err)]

use std::collections::{BTreeMap, BTreeSet};

use tacit_canonical::ast::Node;
use tacit_canonical::hash_node;

use crate::error::Diagnostic;
use crate::interface::{
    generate_host_interface, AbiType, HostInterface, HostTarget, InterfaceRecord,
};
use crate::package::PackageGraph;
use crate::project::{project_definition_expression_with_leaves, ProjectEntryError};
use crate::ty::FixedIntTy;

/// One ABI-expressible scalar at the host boundary.
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

/// One codegen-supported ABI boundary type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibAbiType {
    /// Empty record. Export wrappers omit the out-parameter for this result.
    Unit,
    Scalar(LibScalar),
    Record(LibRecord),
    /// Host-owned call-local borrowed typed vector parameter.
    BorrowedVector(FixedIntTy),
}

impl LibAbiType {
    pub fn is_unit(&self) -> bool {
        matches!(self, LibAbiType::Unit)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibRecord {
    /// Record type hash without `blake3:`.
    pub hash: String,
    pub symbol: String,
    pub fields: Vec<LibRecordField>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibRecordField {
    pub name: String,
    pub c_name: String,
    pub ty: LibAbiType,
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
    pub params: Vec<LibAbiType>,
    pub result: LibAbiType,
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
    pub params: Vec<LibAbiType>,
    pub result: LibAbiType,
}

#[derive(Debug, Clone)]
pub struct PackageLibrary {
    pub package_hash: String,
    /// Short identifier `tacit_p_<pkg64>` used for symbols and the per-package
    /// TLS context global.
    pub package_prefix: String,
    pub exports: Vec<LibraryExport>,
    pub imports: Vec<LibraryImport>,
    pub records: Vec<LibRecord>,
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
        let params = match library_params(&interface, &import.parameters, &hash) {
            Ok(params) => params,
            Err(diag) => {
                diags.push(diag);
                continue;
            }
        };
        let result = match library_type(&interface, &import.result, &hash) {
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
        let params = match library_params(&interface, &export.parameters, &hash) {
            Ok(params) => params,
            Err(diag) => {
                diags.push(diag);
                continue;
            }
        };
        let result = match library_type(&interface, &export.result, &hash) {
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
    let records = match interface
        .records
        .iter()
        .map(|record| library_record(&interface, record, &package_hash))
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(records) => records,
        Err(diag) => return Err(vec![diag]),
    };
    let library = PackageLibrary {
        package_hash,
        package_prefix,
        exports,
        imports,
        records,
    };
    Ok((interface, library))
}

fn library_params(
    interface: &HostInterface,
    abi_params: &[AbiType],
    owner_hash: &str,
) -> Result<Vec<LibAbiType>, Diagnostic> {
    let mut params = Vec::with_capacity(abi_params.len());
    for abi in abi_params {
        params.push(library_type(interface, abi, owner_hash)?);
    }
    Ok(params)
}

fn library_type(
    interface: &HostInterface,
    abi: &AbiType,
    owner_hash: &str,
) -> Result<LibAbiType, Diagnostic> {
    match abi.kind.as_str() {
        "unit" => Ok(LibAbiType::Unit),
        "scalar" => scalar_from_abi(abi)
            .map(LibAbiType::Scalar)
            .ok_or_else(|| unsupported_abi(owner_hash, abi)),
        "record" => {
            let hash = abi
                .hash
                .as_deref()
                .map(strip_prefix)
                .ok_or_else(|| unsupported_abi(owner_hash, abi))?;
            let record = interface
                .records
                .iter()
                .find(|record| strip_prefix(&record.hash) == hash)
                .ok_or_else(|| {
                    library_diag(
                        "abi-library-unsupported-type",
                        owner_hash,
                        format!("record type blake3:{hash} is missing from interface metadata"),
                    )
                })?;
            Ok(LibAbiType::Record(library_record(
                interface, record, owner_hash,
            )?))
        }
        "borrowed_vector" => {
            let element = abi
                .element
                .as_deref()
                .ok_or_else(|| unsupported_abi(owner_hash, abi))?;
            let int_name = element.strip_suffix("vec").ok_or_else(|| {
                library_diag(
                    "abi-library-unsupported-type",
                    owner_hash,
                    format!("borrowed vector element `{element}` is malformed"),
                )
            })?;
            let int_ty = FixedIntTy::parse_name(int_name).ok_or_else(|| {
                library_diag(
                    "abi-library-unsupported-type",
                    owner_hash,
                    format!("borrowed vector element `{element}` is outside the fixed-int set"),
                )
            })?;
            Ok(LibAbiType::BorrowedVector(int_ty))
        }
        _ => Err(unsupported_abi(owner_hash, abi)),
    }
}

fn library_record(
    interface: &HostInterface,
    record: &InterfaceRecord,
    owner_hash: &str,
) -> Result<LibRecord, Diagnostic> {
    let fields = record
        .fields
        .iter()
        .map(|field| {
            Ok(LibRecordField {
                name: field.name.clone(),
                c_name: field.c_name.clone(),
                ty: library_type(interface, &field.type_, owner_hash)?,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    Ok(LibRecord {
        hash: strip_prefix(&record.hash),
        symbol: record.symbol.clone(),
        fields,
    })
}

fn scalar_from_abi(abi: &AbiType) -> Option<LibScalar> {
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

fn unsupported_abi(owner_hash: &str, abi: &AbiType) -> Diagnostic {
    library_diag(
        "abi-library-unsupported-type",
        owner_hash,
        format!(
            "library codegen does not support ABI boundary type {}",
            abi_kind_description(abi)
        ),
    )
}

fn abi_kind_description(abi: &AbiType) -> String {
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
