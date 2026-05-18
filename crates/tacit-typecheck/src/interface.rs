//! Phase 6 constrained host-interface ABI metadata and binding generation.
#![allow(clippy::result_large_err)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::PathBuf;

use serde::Serialize;
use tacit_canonical::ast::Node;
use tacit_canonical::hash_node;

use crate::error::Diagnostic;
use crate::package::{check_package, PackageGraph};
use crate::ty::{EffAtom, EffSet, FnEff, Subst, Ty};
use crate::type_from_node::{child_path, eff_from_node, type_from_node};

const INTERFACE_FORMAT: &str = "tacit-interface-v1";
const HOST_ABI: &str = "tacit-host-abi-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostTarget {
    Native,
    Wasm,
}

#[derive(Debug, Clone)]
pub struct HostInterfaceOutputs {
    pub metadata_path: PathBuf,
    pub c_header_path: PathBuf,
    pub rust_bindings_path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostInterface {
    pub format: String,
    pub abi: String,
    pub package_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<InterfaceInstance>,
    pub exports: Vec<InterfaceExport>,
    pub imports: Vec<InterfaceImport>,
    pub records: Vec<InterfaceRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InterfaceExport {
    pub hash: String,
    pub symbol: String,
    #[serde(skip_serializing_if = "is_false")]
    pub instance_method: bool,
    pub parameters: Vec<AbiType>,
    pub result: AbiType,
    pub effects: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InterfaceInstance {
    pub type_hash: String,
    pub create_symbol: String,
    pub destroy_symbol: String,
    pub state_fields: Vec<InterfaceStateField>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InterfaceStateField {
    pub name: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub fields: Vec<InterfaceStateField>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InterfaceImport {
    pub hash: String,
    pub declaring_package_hash: String,
    pub capability: String,
    pub operation: String,
    pub callback: String,
    pub parameters: Vec<AbiType>,
    pub result: AbiType,
    pub effects: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AbiType {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InterfaceRecord {
    pub hash: String,
    pub symbol: String,
    pub fields: Vec<InterfaceRecordField>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InterfaceRecordField {
    pub name: String,
    pub c_name: String,
    pub type_: AbiType,
}

#[derive(Debug, Clone)]
struct FunctionAbi {
    parameters: Vec<Ty>,
    result: Ty,
    effects: EffSet,
}

#[derive(Debug, Clone)]
struct HostImportDecl {
    declaring_package_hash: String,
    capability: String,
    operation: String,
    sig: Node,
}

#[derive(Debug, Default)]
struct RecordCollector {
    records: BTreeMap<String, InterfaceRecord>,
}

impl HostInterface {
    pub fn to_json_string(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

pub fn generate_host_interface(
    package: &PackageGraph,
    target: HostTarget,
) -> Result<HostInterface, Vec<Diagnostic>> {
    if target == HostTarget::Wasm {
        return Err(vec![Diagnostic::abi_unsupported_target(
            "wasm",
            "Phase 6 host interfaces support only native C ABI artifacts",
        )]);
    }

    check_package(package)?;

    let package_hash = package.package_hash.clone();
    let package64 = short_hash(&package_hash);
    let instance = package_instance(package, &package64)?;
    let instance_method = instance.is_some();
    let definitions = linked_definitions(package);
    let host_imports = host_import_declarations(package);
    let public_exports = public_export_hashes(package);
    let mut reachable_host_imports = BTreeSet::new();
    let mut collector = RecordCollector::default();
    let mut diags = Vec::new();

    let mut exports = Vec::new();
    for hash in public_exports {
        let Some(def) = definitions.get(&hash) else {
            diags.push(Diagnostic::abi_inexpressible_export(
                &hash,
                "public export has no definition artifact",
            ));
            continue;
        };
        collect_reachable_host_imports(
            &hash,
            &definitions,
            &host_imports,
            &mut reachable_host_imports,
        );
        let Some(sig) = definition_sig(def) else {
            diags.push(Diagnostic::abi_inexpressible_export(
                &hash,
                "public export definition has no signature",
            ));
            continue;
        };
        match function_abi_from_sig(sig, true, &hash) {
            Ok(function) => {
                let mut parameters = Vec::new();
                for param in &function.parameters {
                    match collector.abi_type_for_boundary(param, BoundaryPosition::Parameter, &hash)
                    {
                        Ok(abi) => parameters.push(abi),
                        Err(diag) => diags.push(diag),
                    }
                }
                let result = match collector.abi_type_for_boundary(
                    &function.result,
                    BoundaryPosition::Result,
                    &hash,
                ) {
                    Ok(abi) => abi,
                    Err(diag) => {
                        diags.push(diag);
                        AbiType::unit()
                    }
                };
                exports.push(InterfaceExport {
                    hash: prefixed(&hash),
                    symbol: format!("tacit_p_{}_e_{}", package64, short_hash(&hash)),
                    instance_method,
                    parameters,
                    result,
                    effects: effect_strings(&function.effects),
                });
            }
            Err(diag) => diags.push(diag),
        }
    }

    let mut imports = Vec::new();
    for hash in reachable_host_imports {
        let Some(import) = host_imports.get(&hash) else {
            continue;
        };
        match function_abi_from_sig(&import.sig, false, &hash) {
            Ok(function) => {
                let mut parameters = Vec::new();
                for param in &function.parameters {
                    match collector.abi_type_for_boundary(param, BoundaryPosition::Parameter, &hash)
                    {
                        Ok(abi) => parameters.push(abi),
                        Err(diag) => diags.push(diag),
                    }
                }
                let result = match collector.abi_type_for_boundary(
                    &function.result,
                    BoundaryPosition::Result,
                    &hash,
                ) {
                    Ok(abi) => abi,
                    Err(diag) => {
                        diags.push(diag);
                        AbiType::unit()
                    }
                };
                imports.push(InterfaceImport {
                    hash: prefixed(&hash),
                    declaring_package_hash: prefixed(&import.declaring_package_hash),
                    capability: import.capability.clone(),
                    operation: import.operation.clone(),
                    callback: format!("tacit_p_{}_i_{}", package64, short_hash(&hash)),
                    parameters,
                    result,
                    effects: effect_strings(&function.effects),
                });
            }
            Err(diag) => diags.push(diag),
        }
    }

    if !diags.is_empty() {
        return Err(diags);
    }

    exports.sort_by(|a, b| a.hash.cmp(&b.hash));
    imports.sort_by(|a, b| a.hash.cmp(&b.hash));
    let records = collector.records.into_values().collect();

    Ok(HostInterface {
        format: INTERFACE_FORMAT.to_string(),
        abi: HOST_ABI.to_string(),
        package_hash: prefixed(&package_hash),
        instance,
        exports,
        imports,
        records,
    })
}

pub fn write_host_interface(
    package: &PackageGraph,
    target: HostTarget,
) -> Result<(HostInterface, HostInterfaceOutputs), Vec<Diagnostic>> {
    let interface = generate_host_interface(package, target)?;
    let cache_dir = package
        .root
        .root
        .join(".tacit")
        .join("cache")
        .join("packages")
        .join(&package.package_hash);
    let derived_dir = package
        .root
        .root
        .join(".tacit")
        .join("derived")
        .join(format!("package-{}", package.package_hash))
        .join("host");

    std::fs::create_dir_all(&cache_dir).map_err(|source| {
        vec![io_diag(
            "interface-write-failed",
            cache_dir.clone(),
            source,
            "failed to create interface cache directory",
        )]
    })?;
    std::fs::create_dir_all(&derived_dir).map_err(|source| {
        vec![io_diag(
            "interface-write-failed",
            derived_dir.clone(),
            source,
            "failed to create host derived directory",
        )]
    })?;

    let metadata_path = cache_dir.join("interface.json");
    let c_header_path = derived_dir.join("tacit_host.h");
    let rust_bindings_path = derived_dir.join("tacit_host.rs");

    let metadata_json = interface.to_json_string().map_err(|source| {
        vec![Diagnostic::package_error(
            "interface-serialization",
            format!("failed to serialize host interface metadata: {}", source),
            serde_json::json!({"package_hash": prefixed(&package.package_hash)}),
        )]
    })?;
    write_file(&metadata_path, metadata_json.as_bytes())?;
    write_file(&c_header_path, emit_c_header(&interface).as_bytes())?;
    write_file(
        &rust_bindings_path,
        emit_rust_bindings(&interface).as_bytes(),
    )?;

    Ok((
        interface,
        HostInterfaceOutputs {
            metadata_path,
            c_header_path,
            rust_bindings_path,
        },
    ))
}

pub fn emit_c_header(interface: &HostInterface) -> String {
    let guard = format!(
        "TACIT_HOST_{}_H",
        interface
            .package_hash
            .trim_start_matches("blake3:")
            .chars()
            .take(16)
            .collect::<String>()
            .to_ascii_uppercase()
    );
    let pkg = interface
        .exports
        .first()
        .map(|export| package_prefix_from_symbol(&export.symbol))
        .or_else(|| {
            interface
                .imports
                .first()
                .map(|import| package_prefix_from_symbol(&import.callback))
        })
        .or_else(|| {
            interface
                .instance
                .as_ref()
                .map(|instance| package_prefix_from_symbol(&instance.create_symbol))
        })
        .unwrap_or_else(|| "tacit_p_unknown".to_string());

    let mut out = String::new();
    out.push_str(&format!("#ifndef {guard}\n#define {guard}\n\n"));
    out.push_str("#include <stdint.h>\n\n");
    out.push_str("#ifdef __cplusplus\nextern \"C\" {\n#endif\n\n");
    out.push_str("typedef enum tacit_status {\n");
    out.push_str("  TACIT_STATUS_OK = 0,\n");
    out.push_str("  TACIT_STATUS_BAD_ARGUMENT = 1,\n");
    out.push_str("  TACIT_STATUS_MISSING_IMPORT = 2,\n");
    out.push_str("  TACIT_STATUS_HOST_ERROR = 3,\n");
    out.push_str("  TACIT_STATUS_OUT_OF_MEMORY = 4\n");
    out.push_str("} tacit_status;\n\n");
    out.push_str("typedef struct tacit_unit { uint8_t _tacit_unit; } tacit_unit;\n\n");
    for name in ["i8", "u8", "i16", "u16", "i32", "u32", "i64", "u64"] {
        out.push_str(&format!(
            "typedef struct tacit_{name}vec {{ {} *data; uint64_t len; }} tacit_{name}vec;\n",
            c_scalar_for_name(name)
        ));
    }
    out.push('\n');

    for record in &interface.records {
        out.push_str(&format!("typedef struct {} {{\n", record.symbol));
        if record.fields.is_empty() {
            out.push_str("  uint8_t _tacit_unit;\n");
        } else {
            for field in &record.fields {
                out.push_str(&format!("  {} {};\n", c_type(&field.type_), field.c_name));
            }
        }
        out.push_str(&format!("}} {};\n\n", record.symbol));
    }

    out.push_str(&format!("typedef struct {pkg}_callbacks {{\n"));
    if interface.imports.is_empty() {
        out.push_str("  uint8_t _unused;\n");
    } else {
        for import in &interface.imports {
            out.push_str(&format!(
                "  tacit_status (*{})({});\n",
                import.callback,
                callback_params(import)
            ));
        }
    }
    out.push_str(&format!("}} {pkg}_callbacks;\n\n"));
    out.push_str(&format!("typedef struct {pkg}_context {{\n"));
    out.push_str("  void *user;\n");
    out.push_str(&format!("  const {pkg}_callbacks *callbacks;\n"));
    out.push_str(&format!("}} {pkg}_context;\n\n"));

    if let Some(instance) = &interface.instance {
        out.push_str(&format!(
            "typedef struct {pkg}_instance {pkg}_instance;\n\n"
        ));
        out.push_str(&format!(
            "tacit_status {}({pkg}_context *ctx, {pkg}_instance **out);\n",
            instance.create_symbol
        ));
        out.push_str(&format!(
            "tacit_status {}({pkg}_context *ctx, {pkg}_instance *instance);\n\n",
            instance.destroy_symbol
        ));
    }

    for export in &interface.exports {
        out.push_str(&format!(
            "tacit_status {}({});\n",
            export.symbol,
            export_params(&pkg, export)
        ));
    }
    out.push('\n');
    out.push_str("#ifdef __cplusplus\n}\n#endif\n\n");
    out.push_str(&format!("#endif /* {guard} */\n"));
    out
}

pub fn emit_rust_bindings(interface: &HostInterface) -> String {
    let pkg = interface
        .exports
        .first()
        .map(|export| package_prefix_from_symbol(&export.symbol))
        .or_else(|| {
            interface
                .imports
                .first()
                .map(|import| package_prefix_from_symbol(&import.callback))
        })
        .or_else(|| {
            interface
                .instance
                .as_ref()
                .map(|instance| package_prefix_from_symbol(&instance.create_symbol))
        })
        .unwrap_or_else(|| "tacit_p_unknown".to_string());
    let mut out = String::new();
    out.push_str("#![allow(non_camel_case_types)]\n#![allow(non_snake_case)]\n\n");
    out.push_str("use core::ffi::c_void;\n\n");
    out.push_str("#[repr(C)]\n#[derive(Clone, Copy, Debug, PartialEq, Eq)]\n");
    out.push_str("pub enum tacit_status {\n");
    out.push_str("    TACIT_STATUS_OK = 0,\n");
    out.push_str("    TACIT_STATUS_BAD_ARGUMENT = 1,\n");
    out.push_str("    TACIT_STATUS_MISSING_IMPORT = 2,\n");
    out.push_str("    TACIT_STATUS_HOST_ERROR = 3,\n");
    out.push_str("    TACIT_STATUS_OUT_OF_MEMORY = 4,\n");
    out.push_str("}\n\n");
    out.push_str("#[repr(C)]\n#[derive(Clone, Copy, Default)]\npub struct tacit_unit { pub _tacit_unit: u8 }\n\n");
    for name in ["i8", "u8", "i16", "u16", "i32", "u32", "i64", "u64"] {
        out.push_str(&format!(
            "#[repr(C)]\n#[derive(Clone, Copy)]\npub struct tacit_{name}vec {{ pub data: *mut {}, pub len: u64 }}\n\n",
            rust_scalar_for_name(name)
        ));
    }
    for record in &interface.records {
        out.push_str("#[repr(C)]\n#[derive(Clone, Copy, Default)]\n");
        out.push_str(&format!("pub struct {} {{\n", record.symbol));
        if record.fields.is_empty() {
            out.push_str("    pub _tacit_unit: u8,\n");
        } else {
            for field in &record.fields {
                out.push_str(&format!(
                    "    pub {}: {},\n",
                    field.c_name,
                    rust_type(&field.type_)
                ));
            }
        }
        out.push_str("}\n\n");
    }
    out.push_str("#[repr(C)]\n");
    out.push_str(&format!("pub struct {pkg}_callbacks {{\n"));
    if interface.imports.is_empty() {
        out.push_str("    pub _unused: u8,\n");
    } else {
        for import in &interface.imports {
            out.push_str(&format!(
                "    pub {}: Option<unsafe extern \"C\" fn({}) -> tacit_status>,\n",
                import.callback,
                rust_callback_params(import)
            ));
        }
    }
    out.push_str("}\n\n");
    out.push_str("#[repr(C)]\n");
    out.push_str(&format!("pub struct {pkg}_context {{\n"));
    out.push_str("    pub user: *mut c_void,\n");
    out.push_str(&format!("    pub callbacks: *const {pkg}_callbacks,\n"));
    out.push_str("}\n\n");
    if interface.instance.is_some() {
        out.push_str("#[repr(C)]\n");
        out.push_str(&format!(
            "pub struct {pkg}_instance {{ _private: [u8; 0] }}\n\n"
        ));
    }
    out.push_str("extern \"C\" {\n");
    if let Some(instance) = &interface.instance {
        out.push_str(&format!(
            "    pub fn {}(ctx: *mut {pkg}_context, out: *mut *mut {pkg}_instance) -> tacit_status;\n",
            instance.create_symbol
        ));
        out.push_str(&format!(
            "    pub fn {}(ctx: *mut {pkg}_context, instance: *mut {pkg}_instance) -> tacit_status;\n",
            instance.destroy_symbol
        ));
    }
    for export in &interface.exports {
        out.push_str(&format!(
            "    pub fn {}({}) -> tacit_status;\n",
            export.symbol,
            rust_export_params(&pkg, export)
        ));
    }
    out.push_str("}\n");
    if let Some(instance) = &interface.instance {
        out.push_str(&format!(
            "\npub struct Instance<'ctx> {{\n    ctx: &'ctx mut {pkg}_context,\n    raw: *mut {pkg}_instance,\n}}\n\n"
        ));
        out.push_str("impl<'ctx> Instance<'ctx> {\n");
        out.push_str("    pub unsafe fn new(ctx: &'ctx mut ");
        out.push_str(&pkg);
        out.push_str("_context) -> Result<Self, tacit_status> {\n");
        out.push_str("        let mut raw = core::ptr::null_mut();\n");
        out.push_str(&format!(
            "        let status = {}(ctx as *mut _, &mut raw);\n",
            instance.create_symbol
        ));
        out.push_str("        if status == tacit_status::TACIT_STATUS_OK { Ok(Self { ctx, raw }) } else { Err(status) }\n");
        out.push_str("    }\n");
        out.push_str("    pub fn as_raw(&mut self) -> *mut ");
        out.push_str(&pkg);
        out.push_str("_instance { self.raw }\n");
        out.push_str("}\n\n");
        out.push_str("impl Drop for Instance<'_> {\n");
        out.push_str("    fn drop(&mut self) {\n");
        out.push_str("        unsafe {\n");
        out.push_str(&format!(
            "            let _ = {}(self.ctx as *mut _, self.raw);\n",
            instance.destroy_symbol
        ));
        out.push_str("        }\n");
        out.push_str("    }\n");
        out.push_str("}\n");
    }
    out
}

impl AbiType {
    fn scalar(name: impl Into<String>) -> Self {
        Self {
            kind: "scalar".to_string(),
            name: Some(name.into()),
            hash: None,
            element: None,
        }
    }

    fn unit() -> Self {
        Self {
            kind: "unit".to_string(),
            name: None,
            hash: None,
            element: None,
        }
    }

    fn record(hash: String) -> Self {
        Self {
            kind: "record".to_string(),
            name: None,
            hash: Some(prefixed(&hash)),
            element: None,
        }
    }

    fn borrowed_vector(element: String) -> Self {
        Self {
            kind: "borrowed_vector".to_string(),
            name: None,
            hash: None,
            element: Some(element),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoundaryPosition {
    Parameter,
    Result,
    RecordField,
}

impl RecordCollector {
    fn abi_type_for_boundary(
        &mut self,
        ty: &Ty,
        position: BoundaryPosition,
        owner_hash: &str,
    ) -> Result<AbiType, Diagnostic> {
        match ty {
            Ty::IntLit | Ty::Int => Ok(AbiType::scalar("i64")),
            Ty::Bool => Ok(AbiType::scalar("bool")),
            Ty::FixedInt(int_ty) => Ok(AbiType::scalar(int_ty.to_string())),
            Ty::Record(fields) if fields.is_empty() => Ok(AbiType::unit()),
            Ty::Record(fields) => {
                let record_hash = record_type_hash(fields);
                let mut record_fields = Vec::new();
                for (name, field_ty) in fields {
                    if matches!(field_ty, Ty::Vec(_)) {
                        return Err(Diagnostic::abi_vector_position(
                            owner_hash,
                            "typed vector handle appears in a record field",
                        ));
                    }
                    let field_abi = self.abi_type_for_boundary(
                        field_ty,
                        BoundaryPosition::RecordField,
                        owner_hash,
                    )?;
                    record_fields.push(InterfaceRecordField {
                        name: name.clone(),
                        c_name: sanitize_c_ident(name),
                        type_: field_abi,
                    });
                }
                self.records
                    .entry(record_hash.clone())
                    .or_insert_with(|| InterfaceRecord {
                        hash: prefixed(&record_hash),
                        symbol: format!("tacit_r_{}", short_hash(&record_hash)),
                        fields: record_fields,
                    });
                Ok(AbiType::record(record_hash))
            }
            Ty::Vec(int_ty) if position == BoundaryPosition::Parameter => {
                Ok(AbiType::borrowed_vector(format!("{}vec", int_ty)))
            }
            Ty::Vec(_) => Err(Diagnostic::abi_vector_position(
                owner_hash,
                "typed vector handle is ABI-expressible only as a borrowed parameter",
            )),
            Ty::Fn(_, _, _) => Err(Diagnostic::abi_inexpressible_type(
                owner_hash,
                "function values cannot cross the Stage 10 host boundary",
            )),
            Ty::Str | Ty::Buf | Ty::I64Vec | Ty::App(_, _) | Ty::Meta(_) | Ty::Unknown => {
                Err(Diagnostic::abi_inexpressible_type(
                    owner_hash,
                    &format!("type {} is outside the Stage 10 host ABI subset", ty),
                ))
            }
        }
    }
}

fn function_abi_from_sig(
    sig: &Node,
    export: bool,
    owner_hash: &str,
) -> Result<FunctionAbi, Diagnostic> {
    let Node::Sig { type_, eval_eff } = sig else {
        return Err(Diagnostic::abi_inexpressible_export(
            owner_hash,
            "boundary artifact is missing a sig node",
        ));
    };
    let mut subst = Subst::default();
    let mut diags = Vec::new();
    let ty = type_from_node(type_, &[], &[], &mut subst, &[], &mut diags);
    let eval_eff = match eff_from_node(eval_eff, &[], &mut subst, &[], &mut diags) {
        FnEff::Concrete(set) => set,
        FnEff::Meta(_) => EffSet::empty(),
    };
    if let Some(diag) = diags.into_iter().find(|diag| diag.severity == "error") {
        return Err(Diagnostic::abi_inexpressible_type(
            owner_hash,
            &diag.message,
        ));
    }
    if export && !eval_eff.is_pure() {
        return Err(Diagnostic::abi_inexpressible_effect(
            owner_hash,
            "definition-evaluation effect must be pure for host exports",
        ));
    }

    let mut cur = subst.apply(&ty);
    let mut parameters = Vec::new();
    let mut effects = EffSet::empty();
    loop {
        match cur {
            Ty::Fn(arg, ret, FnEff::Concrete(call_effects)) => {
                reject_meta_ty(&arg, owner_hash)?;
                parameters.push(*arg);
                effects = effects.join(&call_effects);
                cur = subst.apply(&ret);
            }
            Ty::Fn(_, _, FnEff::Meta(_)) => {
                return Err(Diagnostic::abi_inexpressible_effect(
                    owner_hash,
                    "function crossing the host boundary uses an effect variable",
                ));
            }
            other => {
                if parameters.is_empty() {
                    return Err(Diagnostic::abi_inexpressible_export(
                        owner_hash,
                        "host boundary value must be a monomorphic function",
                    ));
                }
                reject_meta_ty(&other, owner_hash)?;
                return Ok(FunctionAbi {
                    parameters,
                    result: other,
                    effects,
                });
            }
        }
    }
}

fn reject_meta_ty(ty: &Ty, owner_hash: &str) -> Result<(), Diagnostic> {
    match ty {
        Ty::Meta(_) => Err(Diagnostic::abi_inexpressible_type(
            owner_hash,
            "polymorphic type variable cannot cross the host boundary",
        )),
        Ty::Fn(arg, ret, FnEff::Concrete(_)) => {
            reject_meta_ty(arg, owner_hash)?;
            reject_meta_ty(ret, owner_hash)
        }
        Ty::Fn(_, _, FnEff::Meta(_)) => Err(Diagnostic::abi_inexpressible_effect(
            owner_hash,
            "effect variable cannot cross the host boundary",
        )),
        Ty::Record(fields) => {
            for field in fields.values() {
                reject_meta_ty(field, owner_hash)?;
            }
            Ok(())
        }
        Ty::App(fun, arg) => {
            reject_meta_ty(fun, owner_hash)?;
            reject_meta_ty(arg, owner_hash)
        }
        _ => Ok(()),
    }
}

fn linked_definitions(package: &PackageGraph) -> BTreeMap<String, Node> {
    let mut definitions = BTreeMap::new();
    for (hash, definition) in &package.root.definitions {
        definitions.insert(hash.clone(), definition.def.clone());
    }
    for dependency in &package.dependencies {
        for (hash, definition) in &dependency.definitions {
            definitions
                .entry(hash.clone())
                .or_insert_with(|| definition.def.clone());
        }
    }
    definitions
}

fn host_import_declarations(package: &PackageGraph) -> BTreeMap<String, HostImportDecl> {
    let mut imports = BTreeMap::new();
    collect_host_imports_from_units(&package.root.units, &package.package_hash, &mut imports);
    for dependency in &package.dependencies {
        collect_host_imports_from_units(&dependency.units, &dependency.hash, &mut imports);
    }
    imports
}

fn collect_host_imports_from_units(
    units: &[crate::project::ProjectUnit],
    package_hash: &str,
    imports: &mut BTreeMap<String, HostImportDecl>,
) {
    for unit in units {
        let Node::Unit {
            imports: entries, ..
        } = &unit.node
        else {
            continue;
        };
        for entry in entries {
            if let Node::HostImport {
                capability,
                operation,
                sig,
            } = entry
            {
                let hash = hash_hex(entry);
                imports
                    .entry(hash.clone())
                    .or_insert_with(|| HostImportDecl {
                        declaring_package_hash: package_hash.to_string(),
                        capability: capability.clone(),
                        operation: operation.clone(),
                        sig: sig.as_ref().clone(),
                    });
            }
        }
    }
}

fn public_export_hashes(package: &PackageGraph) -> Vec<String> {
    let mut hashes: BTreeSet<String> = BTreeSet::new();
    for unit in &package.root.units {
        hashes.extend(unit.public_exports.iter().cloned());
    }
    hashes.into_iter().collect()
}

fn package_instance(
    package: &PackageGraph,
    package64: &str,
) -> Result<Option<InterfaceInstance>, Vec<Diagnostic>> {
    let mut found: Option<(String, Node)> = None;
    let mut diags = Vec::new();
    for (unit_index, unit) in package.root.units.iter().enumerate() {
        let Node::Unit { defs, .. } = &unit.node else {
            continue;
        };
        for (def_index, def) in defs.iter().enumerate() {
            let Node::State { type_, .. } = def else {
                continue;
            };
            if found.is_some() {
                diags.push(Diagnostic::state_multiple(&[unit_index, 2, def_index]));
                continue;
            }
            found = Some((hash_hex(def), type_.as_ref().clone()));
        }
    }
    if !diags.is_empty() {
        return Err(diags);
    }

    let Some((type_hash, type_node)) = found else {
        return Ok(None);
    };
    let mut subst = Subst::default();
    let mut type_diags = Vec::new();
    let ty = type_from_node(&type_node, &[], &[], &mut subst, &[], &mut type_diags);
    let Ty::Record(fields) = subst.apply(&ty) else {
        return Err(vec![Diagnostic::state_field_shape_invalid(&[], &ty)]);
    };
    let mut state_fields = Vec::new();
    for (name, field_ty) in fields {
        match state_field_metadata(&name, &field_ty, &[]) {
            Ok(field) => state_fields.push(field),
            Err(diag) => type_diags.push(diag),
        }
    }
    if !type_diags.is_empty() {
        return Err(type_diags);
    }
    Ok(Some(InterfaceInstance {
        type_hash: prefixed(&type_hash),
        create_symbol: format!("tacit_p_{package64}_create"),
        destroy_symbol: format!("tacit_p_{package64}_destroy"),
        state_fields,
    }))
}

fn state_field_metadata(
    name: &str,
    ty: &Ty,
    path: &[usize],
) -> Result<InterfaceStateField, Diagnostic> {
    match ty {
        Ty::Int | Ty::IntLit => Ok(InterfaceStateField {
            name: name.to_string(),
            kind: "scalar".to_string(),
            name_type: Some("Int".to_string()),
            element: None,
            fields: Vec::new(),
        }),
        Ty::Bool => Ok(InterfaceStateField {
            name: name.to_string(),
            kind: "scalar".to_string(),
            name_type: Some("bool".to_string()),
            element: None,
            fields: Vec::new(),
        }),
        Ty::FixedInt(int_ty) => Ok(InterfaceStateField {
            name: name.to_string(),
            kind: "scalar".to_string(),
            name_type: Some(int_ty.to_string()),
            element: None,
            fields: Vec::new(),
        }),
        Ty::Vec(int_ty) => Ok(InterfaceStateField {
            name: name.to_string(),
            kind: "vec".to_string(),
            name_type: None,
            element: Some(int_ty.to_string()),
            fields: Vec::new(),
        }),
        Ty::Record(fields) => {
            let mut rendered = Vec::new();
            for (i, (field_name, field_ty)) in fields.iter().enumerate() {
                rendered.push(state_field_metadata(
                    field_name,
                    field_ty,
                    &child_path(path, i * 2 + 1),
                )?);
            }
            Ok(InterfaceStateField {
                name: name.to_string(),
                kind: "record".to_string(),
                name_type: None,
                element: None,
                fields: rendered,
            })
        }
        other => Err(Diagnostic::state_field_shape_invalid(path, other)),
    }
}

fn collect_reachable_host_imports(
    start_hash: &str,
    definitions: &BTreeMap<String, Node>,
    host_imports: &BTreeMap<String, HostImportDecl>,
    out: &mut BTreeSet<String>,
) {
    fn visit(
        hash: &str,
        definitions: &BTreeMap<String, Node>,
        host_imports: &BTreeMap<String, HostImportDecl>,
        seen_defs: &mut BTreeSet<String>,
        out: &mut BTreeSet<String>,
    ) {
        if host_imports.contains_key(hash) {
            out.insert(hash.to_string());
            return;
        }
        if !seen_defs.insert(hash.to_string()) {
            return;
        }
        let Some(def) = definitions.get(hash) else {
            return;
        };
        let Some(body) = definition_body(def) else {
            return;
        };
        let mut refs = Vec::new();
        collect_refs(body, &mut refs);
        for dep in refs {
            visit(&dep, definitions, host_imports, seen_defs, out);
        }
    }

    visit(
        start_hash,
        definitions,
        host_imports,
        &mut BTreeSet::new(),
        out,
    );
}

fn collect_refs(node: &Node, out: &mut Vec<String>) {
    match node {
        Node::Ref { hash } => out.push(hash.clone()),
        Node::Lam { body } => collect_refs(body, out),
        Node::App { fn_, arg } => {
            collect_refs(fn_, out);
            collect_refs(arg, out);
        }
        Node::Let { rhs, body } => {
            collect_refs(rhs, out);
            collect_refs(body, out);
        }
        Node::Rec { bindings, body } => {
            for binding in bindings {
                collect_refs(binding, out);
            }
            collect_refs(body, out);
        }
        Node::Module { bindings } => {
            for binding in bindings {
                collect_refs(binding, out);
            }
        }
        Node::If { cond, then, else_ } => {
            collect_refs(cond, out);
            collect_refs(then, out);
            collect_refs(else_, out);
        }
        Node::Match { scrutinee, arms } => {
            collect_refs(scrutinee, out);
            for arm in arms {
                collect_refs(arm, out);
            }
        }
        Node::Arm { body, .. } => collect_refs(body, out),
        Node::Record { fields } => {
            for (_, value) in fields {
                collect_refs(value, out);
            }
        }
        Node::Proj { record, .. } => collect_refs(record, out),
        Node::Ctor { args, .. } => {
            for arg in args {
                collect_refs(arg, out);
            }
        }
        Node::Ann { expr, .. } => collect_refs(expr, out),
        Node::Def { body, .. } => collect_refs(body, out),
        _ => {}
    }
}

fn definition_sig(def: &Node) -> Option<&Node> {
    match def {
        Node::Def { sig, .. } => Some(sig),
        _ => None,
    }
}

fn definition_body(def: &Node) -> Option<&Node> {
    match def {
        Node::Def { body, .. } => Some(body),
        _ => None,
    }
}

fn record_type_hash(fields: &BTreeMap<String, Ty>) -> String {
    let node = Node::Record {
        fields: fields
            .iter()
            .map(|(name, ty)| (name.clone(), ty_to_type_node(ty)))
            .collect(),
    };
    hash_hex(&node)
}

fn ty_to_type_node(ty: &Ty) -> Node {
    match ty {
        Ty::IntLit | Ty::Int => Node::Sym { name: "Int".into() },
        Ty::Bool => Node::Sym {
            name: "Bool".into(),
        },
        Ty::Str => Node::Sym { name: "Str".into() },
        Ty::Buf => Node::Sym { name: "Buf".into() },
        Ty::I64Vec => Node::Sym {
            name: "I64Vec".into(),
        },
        Ty::FixedInt(int_ty) => Node::Sym {
            name: int_ty.to_string(),
        },
        Ty::Vec(int_ty) => Node::Sym {
            name: format!("{}vec", int_ty),
        },
        Ty::Fn(arg, ret, FnEff::Concrete(effects)) => Node::FnTy {
            arg: Box::new(ty_to_type_node(arg)),
            ret: Box::new(ty_to_type_node(ret)),
            eff: Box::new(Node::EffSet {
                atoms: effect_strings(effects),
            }),
        },
        Ty::Fn(arg, ret, FnEff::Meta(index)) => Node::FnTy {
            arg: Box::new(ty_to_type_node(arg)),
            ret: Box::new(ty_to_type_node(ret)),
            eff: Box::new(Node::EffVar {
                index: *index as u64,
            }),
        },
        Ty::Record(fields) => Node::Record {
            fields: fields
                .iter()
                .map(|(name, ty)| (name.clone(), ty_to_type_node(ty)))
                .collect(),
        },
        Ty::App(fun, arg) => Node::App {
            fn_: Box::new(ty_to_type_node(fun)),
            arg: Box::new(ty_to_type_node(arg)),
        },
        Ty::Meta(index) => Node::TyVar {
            index: *index as u64,
        },
        Ty::Unknown => Node::Hole {
            diag_id: "unknown-type".to_string(),
            payload: Box::new(Node::Str {
                value: "unknown type".to_string(),
            }),
        },
    }
}

fn effect_strings(effects: &EffSet) -> Vec<String> {
    let order = [EffAtom::Alloc, EffAtom::Div, EffAtom::IO, EffAtom::Mut];
    order
        .into_iter()
        .filter(|atom| effects.atoms.contains(atom))
        .map(|atom| atom.to_string())
        .collect()
}

fn c_type(ty: &AbiType) -> String {
    match ty.kind.as_str() {
        "scalar" => c_scalar_for_name(ty.name.as_deref().unwrap_or("i64")).to_string(),
        "unit" => "tacit_unit".to_string(),
        "record" => {
            let hash = ty
                .hash
                .as_deref()
                .unwrap_or(
                    "blake3:0000000000000000000000000000000000000000000000000000000000000000",
                )
                .trim_start_matches("blake3:");
            format!("tacit_r_{}", short_hash(hash))
        }
        "borrowed_vector" => format!("tacit_{}", ty.element.as_deref().unwrap_or("u8vec")),
        _ => "tacit_unit".to_string(),
    }
}

fn rust_type(ty: &AbiType) -> String {
    match ty.kind.as_str() {
        "scalar" => rust_scalar_for_name(ty.name.as_deref().unwrap_or("i64")).to_string(),
        "unit" => "tacit_unit".to_string(),
        "record" => {
            let hash = ty
                .hash
                .as_deref()
                .unwrap_or(
                    "blake3:0000000000000000000000000000000000000000000000000000000000000000",
                )
                .trim_start_matches("blake3:");
            format!("tacit_r_{}", short_hash(hash))
        }
        "borrowed_vector" => format!("tacit_{}", ty.element.as_deref().unwrap_or("u8vec")),
        _ => "tacit_unit".to_string(),
    }
}

fn c_scalar_for_name(name: &str) -> &'static str {
    match name {
        "bool" | "u8" => "uint8_t",
        "i8" => "int8_t",
        "u16" => "uint16_t",
        "i16" => "int16_t",
        "u32" => "uint32_t",
        "i32" => "int32_t",
        "u64" => "uint64_t",
        "i64" | "Int" => "int64_t",
        _ => "int64_t",
    }
}

fn rust_scalar_for_name(name: &str) -> &'static str {
    match name {
        "bool" | "u8" => "u8",
        "i8" => "i8",
        "u16" => "u16",
        "i16" => "i16",
        "u32" => "u32",
        "i32" => "i32",
        "u64" => "u64",
        "i64" | "Int" => "i64",
        _ => "i64",
    }
}

fn export_params(pkg: &str, export: &InterfaceExport) -> String {
    let mut params = vec![format!("{pkg}_context *ctx")];
    if export.instance_method {
        params.push(format!("{pkg}_instance *instance"));
    }
    for (i, param) in export.parameters.iter().enumerate() {
        params.push(format!("{} arg{}", c_type(param), i));
    }
    if export.result.kind != "unit" {
        params.push(format!("{} *out", c_type(&export.result)));
    }
    params.join(", ")
}

fn callback_params(import: &InterfaceImport) -> String {
    let mut params = vec!["void *user".to_string()];
    for (i, param) in import.parameters.iter().enumerate() {
        params.push(format!("{} arg{}", c_type(param), i));
    }
    if import.result.kind != "unit" {
        params.push(format!("{} *out", c_type(&import.result)));
    }
    params.join(", ")
}

fn rust_export_params(pkg: &str, export: &InterfaceExport) -> String {
    let mut params = vec![format!("ctx: *mut {pkg}_context")];
    if export.instance_method {
        params.push(format!("instance: *mut {pkg}_instance"));
    }
    for (i, param) in export.parameters.iter().enumerate() {
        params.push(format!("arg{}: {}", i, rust_type(param)));
    }
    if export.result.kind != "unit" {
        params.push(format!("out: *mut {}", rust_type(&export.result)));
    }
    params.join(", ")
}

fn rust_callback_params(import: &InterfaceImport) -> String {
    let mut params = vec!["user: *mut c_void".to_string()];
    for (i, param) in import.parameters.iter().enumerate() {
        params.push(format!("arg{}: {}", i, rust_type(param)));
    }
    if import.result.kind != "unit" {
        params.push(format!("out: *mut {}", rust_type(&import.result)));
    }
    params.join(", ")
}

fn package_prefix_from_symbol(symbol: &str) -> String {
    let mut parts = symbol.split('_');
    match (parts.next(), parts.next(), parts.next()) {
        (Some("tacit"), Some("p"), Some(pkg)) => format!("tacit_p_{pkg}"),
        _ => "tacit_p_unknown".to_string(),
    }
}

fn prefixed(hash: &str) -> String {
    if hash.starts_with("blake3:") {
        hash.to_string()
    } else {
        format!("blake3:{hash}")
    }
}

fn short_hash(hash: &str) -> String {
    hash.trim_start_matches("blake3:")
        .chars()
        .take(16)
        .collect()
}

fn hash_hex(node: &Node) -> String {
    hash_node(node)
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect()
}

fn sanitize_c_ident(name: &str) -> String {
    let mut out = String::new();
    for (i, ch) in name.chars().enumerate() {
        let valid = ch == '_' || ch.is_ascii_alphanumeric();
        if i == 0 && ch.is_ascii_digit() {
            out.push('_');
        }
        out.push(if valid { ch } else { '_' });
    }
    if out.is_empty() || matches!(out.as_str(), "type" | "match" | "struct" | "union") {
        out.push('_');
    }
    out
}

fn write_file(path: &PathBuf, bytes: &[u8]) -> Result<(), Vec<Diagnostic>> {
    std::fs::write(path, bytes).map_err(|source| {
        vec![io_diag(
            "interface-write-failed",
            path.clone(),
            source,
            "failed to write host interface output",
        )]
    })
}

fn io_diag(kind: &str, path: PathBuf, source: std::io::Error, message: &str) -> Diagnostic {
    Diagnostic::package_error(
        kind,
        format!("{} {}: {}", message, path.display(), source),
        serde_json::json!({"path": path}),
    )
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl fmt::Display for HostTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HostTarget::Native => write!(f, "native"),
            HostTarget::Wasm => write!(f, "wasm"),
        }
    }
}
