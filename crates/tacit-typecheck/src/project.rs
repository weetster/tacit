//! Whole-project graph loading and checking.
//!
//! Stage 2 keeps project composition manifestless: a project root contributes
//! canonical `unit` artifacts from `.tac` files, then the graph is indexed and
//! checked by content hash rather than file path.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tacit_canonical::ast::Node;
use tacit_canonical::{emit, hash_bytes, hash_node, parse, ParseError};
use tacit_views::sidecar::{Sidecar, SidecarNode};
use tacit_views::{emit_inspection, InspectFlags};

use crate::error::Diagnostic;
use crate::ty::{Subst, Ty};
use crate::type_from_node::type_from_node;
use crate::units::{
    check_unit_with_sidecar, CheckedUnit, DefinitionEnv, DefinitionVisibility, ProvidedDefinition,
};

#[derive(Debug, Clone)]
pub struct ProjectGraph {
    pub root: PathBuf,
    pub source_base: PathBuf,
    pub graph_hash: String,
    pub units: Vec<ProjectUnit>,
    pub definitions: BTreeMap<String, ProjectDefinition>,
}

impl ProjectGraph {
    pub fn definition_env(&self) -> DefinitionEnv {
        self.definitions
            .iter()
            .map(|(hash, definition)| {
                (
                    hash.clone(),
                    ProvidedDefinition::new(definition.def.clone(), definition.visibility, true),
                )
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct ProjectUnit {
    pub hash: String,
    pub node: Node,
    pub sidecar: Option<SidecarNode>,
    pub source_paths: Vec<PathBuf>,
    pub definition_hashes: Vec<String>,
    pub public_exports: BTreeSet<String>,
    pub package_exports: BTreeSet<String>,
}

#[derive(Debug, Clone)]
pub struct ProjectDefinition {
    pub hash: String,
    pub def: Node,
    pub visibility: DefinitionVisibility,
    pub unit_hashes: BTreeSet<String>,
}

#[derive(Debug)]
pub enum ProjectLoadError {
    Io { path: PathBuf, source: io::Error },
    Parse { path: PathBuf, source: ParseError },
    Sidecar { path: PathBuf, message: String },
    NotDirectory { path: PathBuf },
    EmptyProject { source_base: PathBuf },
    NonUnitArtifact { path: PathBuf },
}

impl fmt::Display for ProjectLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProjectLoadError::Io { path, source } => write!(f, "{}: {}", path.display(), source),
            ProjectLoadError::Parse { path, source } => {
                write!(f, "{}: {}", path.display(), source)
            }
            ProjectLoadError::Sidecar { path, message } => {
                write!(f, "{}: {}", path.display(), message)
            }
            ProjectLoadError::NotDirectory { path } => {
                write!(f, "{}: expected project root directory", path.display())
            }
            ProjectLoadError::EmptyProject { source_base } => {
                write!(f, "{}: no .tac unit files found", source_base.display())
            }
            ProjectLoadError::NonUnitArtifact { path } => {
                write!(
                    f,
                    "{}: project .tac files must be unit artifacts",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for ProjectLoadError {}

#[derive(Debug)]
pub enum ProjectDerivedError {
    Io { path: PathBuf, source: io::Error },
    Json { source: serde_json::Error },
}

impl fmt::Display for ProjectDerivedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProjectDerivedError::Io { path, source } => {
                write!(f, "{}: {}", path.display(), source)
            }
            ProjectDerivedError::Json { source } => {
                write!(f, "project graph index serialization failed: {}", source)
            }
        }
    }
}

impl std::error::Error for ProjectDerivedError {}

#[derive(Debug, Clone)]
pub struct ProjectEntry {
    pub hash: String,
    pub expression: Node,
}

#[derive(Debug, Clone)]
pub enum ProjectEntryError {
    NoPublicExports,
    AmbiguousPublicExports(Vec<String>),
    EntryNotFound(String),
    AmbiguousEntryAlias { alias: String, hashes: Vec<String> },
    MissingDefinition(String),
    CyclicDependency(Vec<String>),
    InvalidSignature { hash: String, message: String },
    NonExecutableEntry { hash: String, ty: String },
}

impl fmt::Display for ProjectEntryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProjectEntryError::NoPublicExports => {
                write!(f, "project has no public exports to compile")
            }
            ProjectEntryError::AmbiguousPublicExports(hashes) => write!(
                f,
                "project has multiple public exports; pass --entry with one of: {}",
                hashes
                    .iter()
                    .map(|hash| format!("blake3:{}", hash))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            ProjectEntryError::EntryNotFound(selector) => {
                write!(f, "no public export matches entry {:?}", selector)
            }
            ProjectEntryError::AmbiguousEntryAlias { alias, hashes } => write!(
                f,
                "entry alias {:?} is ambiguous across public exports: {}",
                alias,
                hashes
                    .iter()
                    .map(|hash| format!("blake3:{}", hash))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            ProjectEntryError::MissingDefinition(hash) => {
                write!(f, "public export blake3:{} has no indexed definition", hash)
            }
            ProjectEntryError::CyclicDependency(cycle) => write!(
                f,
                "cannot lower cyclic project dependency: {}",
                cycle
                    .iter()
                    .map(|hash| format!("blake3:{}", hash))
                    .collect::<Vec<_>>()
                    .join(" -> ")
            ),
            ProjectEntryError::InvalidSignature { hash, message } => write!(
                f,
                "definition blake3:{} has an invalid signature: {}",
                hash, message
            ),
            ProjectEntryError::NonExecutableEntry { hash, ty } => write!(
                f,
                "public export blake3:{} has type {}; standalone executables require Int or Bool",
                hash, ty
            ),
        }
    }
}

impl std::error::Error for ProjectEntryError {}

pub fn discover_project_root(input: impl AsRef<Path>) -> Result<PathBuf, ProjectLoadError> {
    let input = input.as_ref();
    let metadata = std::fs::metadata(input).map_err(|source| ProjectLoadError::Io {
        path: input.to_path_buf(),
        source,
    })?;
    if !metadata.is_dir() {
        return Err(ProjectLoadError::NotDirectory {
            path: input.to_path_buf(),
        });
    }
    std::fs::canonicalize(input).map_err(|source| ProjectLoadError::Io {
        path: input.to_path_buf(),
        source,
    })
}

pub fn load_project(root: impl AsRef<Path>) -> Result<ProjectGraph, ProjectLoadError> {
    let root = discover_project_root(root)?;
    let source_base = if root.join("src").is_dir() {
        root.join("src")
    } else {
        root.clone()
    };

    let mut tac_files = Vec::new();
    collect_tac_files(&source_base, &mut tac_files)?;
    tac_files.sort();
    if tac_files.is_empty() {
        return Err(ProjectLoadError::EmptyProject { source_base });
    }

    let mut units_by_hash = BTreeMap::new();
    for path in tac_files {
        let bytes = std::fs::read(&path).map_err(|source| ProjectLoadError::Io {
            path: path.clone(),
            source,
        })?;
        let node = parse(&bytes).map_err(|source| ProjectLoadError::Parse {
            path: path.clone(),
            source,
        })?;
        if !matches!(node, Node::Unit { .. }) {
            return Err(ProjectLoadError::NonUnitArtifact { path });
        }

        let unit_hash = hex_hash_bytes(&emit(&node));
        let rel_path = path
            .strip_prefix(&root)
            .map(Path::to_path_buf)
            .unwrap_or_else(|_| path.clone());
        let sidecar = read_fresh_sidecar(&path, &bytes)?;
        let (definition_hashes, public_exports, package_exports) = unit_boundary_hashes(&node);

        units_by_hash
            .entry(unit_hash.clone())
            .and_modify(|unit: &mut ProjectUnit| unit.source_paths.push(rel_path.clone()))
            .or_insert(ProjectUnit {
                hash: unit_hash,
                node,
                sidecar,
                source_paths: vec![rel_path],
                definition_hashes,
                public_exports,
                package_exports,
            });
    }

    let units: Vec<ProjectUnit> = units_by_hash.into_values().collect();
    let graph_hash = project_graph_hash(units.iter().map(|unit| unit.hash.as_str()));
    let definitions = build_definition_index(&units);

    Ok(ProjectGraph {
        root,
        source_base,
        graph_hash,
        units,
        definitions,
    })
}

pub fn check_project(graph: &ProjectGraph) -> Result<Vec<CheckedUnit>, Vec<Diagnostic>> {
    let env = graph.definition_env();
    let mut checked = Vec::new();
    let mut diags = Vec::new();

    for (unit_index, unit) in graph.units.iter().enumerate() {
        match check_unit_with_sidecar(&unit.node, &env, unit.sidecar.as_ref()) {
            Ok(unit) => checked.push(unit),
            Err(mut errors) => {
                for error in &mut errors {
                    error
                        .location
                        .ast_path
                        .insert(0, crate::error::PathStep { child: unit_index });
                }
                diags.append(&mut errors);
            }
        }
    }

    if diags.is_empty() {
        Ok(checked)
    } else {
        Err(diags)
    }
}

pub fn materialize_project_derived(graph: &ProjectGraph) -> Result<PathBuf, ProjectDerivedError> {
    let derived_root = graph
        .root
        .join(".tacit")
        .join("derived")
        .join(format!("project-{}", graph.graph_hash));
    let units_dir = derived_root.join("units");
    let defs_dir = derived_root.join("defs");
    let index_dir = derived_root.join("index");
    let build_dir = derived_root.join("build");
    let bin_dir = derived_root.join("bin");
    let views_dir = derived_root.join("views");

    for dir in [
        &units_dir, &defs_dir, &index_dir, &build_dir, &bin_dir, &views_dir,
    ] {
        std::fs::create_dir_all(dir).map_err(|source| ProjectDerivedError::Io {
            path: dir.clone(),
            source,
        })?;
    }

    for unit in &graph.units {
        let path = units_dir.join(format!("{}.tac", unit.hash));
        std::fs::write(&path, emit(&unit.node))
            .map_err(|source| ProjectDerivedError::Io { path, source })?;
    }

    for definition in graph.definitions.values() {
        let path = defs_dir.join(format!("{}.tac", definition.hash));
        std::fs::write(&path, emit(&definition.def))
            .map_err(|source| ProjectDerivedError::Io { path, source })?;
    }

    let index = DerivedProjectIndex::from_graph(graph);
    let json =
        serde_json::to_vec_pretty(&index).map_err(|source| ProjectDerivedError::Json { source })?;
    let index_path = index_dir.join("project-graph.json");
    std::fs::write(&index_path, json).map_err(|source| ProjectDerivedError::Io {
        path: index_path,
        source,
    })?;

    Ok(derived_root)
}

pub fn project_entry_expression(
    graph: &ProjectGraph,
    selector: Option<&str>,
) -> Result<ProjectEntry, ProjectEntryError> {
    let hash = resolve_entry_hash(graph, selector)?;
    project_definition_expression(graph, &hash)
}

pub fn project_definition_expression(
    graph: &ProjectGraph,
    hash: &str,
) -> Result<ProjectEntry, ProjectEntryError> {
    let definition = graph
        .definitions
        .get(hash)
        .ok_or_else(|| ProjectEntryError::MissingDefinition(hash.to_string()))?;

    let ty = definition_value_type(hash, &definition.def)?;
    if !matches!(ty, Ty::Int | Ty::Bool | Ty::FixedInt(_)) {
        return Err(ProjectEntryError::NonExecutableEntry {
            hash: hash.to_string(),
            ty: ty.to_string(),
        });
    }

    let mut stack = Vec::new();
    let expression = expanded_definition_body(graph, hash, &mut stack, &BTreeSet::new())?;
    Ok(ProjectEntry {
        hash: definition.hash.clone(),
        expression,
    })
}

/// Expand a definition body but leave `Ref` nodes whose hash is in `leaves`
/// in place. Used by Stage 11 host-library codegen so host-import refs can be
/// dispatched through the callback table rather than being expanded inline.
pub fn project_definition_expression_with_leaves(
    graph: &ProjectGraph,
    hash: &str,
    leaves: &BTreeSet<String>,
) -> Result<ProjectEntry, ProjectEntryError> {
    let definition = graph
        .definitions
        .get(hash)
        .ok_or_else(|| ProjectEntryError::MissingDefinition(hash.to_string()))?;
    let mut stack = Vec::new();
    let expression = expanded_definition_body(graph, hash, &mut stack, leaves)?;
    Ok(ProjectEntry {
        hash: definition.hash.clone(),
        expression,
    })
}

pub fn emit_project_inspection(graph: &ProjectGraph, flags: &InspectFlags) -> String {
    let mut out = String::new();
    out.push_str(&format!("project blake3:{}\n", graph.graph_hash));
    out.push_str(&format!(
        "source {}\n",
        relative_display(&graph.root, &graph.source_base)
    ));
    out.push_str("units\n");
    for unit in &graph.units {
        out.push_str(&format!("  blake3:{}\n", unit.hash));
        if !unit.source_paths.is_empty() {
            out.push_str("    sources");
            for path in &unit.source_paths {
                out.push(' ');
                out.push_str(&path.to_string_lossy());
            }
            out.push('\n');
        }
        out.push_str("    public");
        if unit.public_exports.is_empty() {
            out.push_str(" <none>\n");
        } else {
            for hash in &unit.public_exports {
                out.push_str(&format!(" blake3:{}", hash));
            }
            out.push('\n');
        }
        out.push_str("    package");
        if unit.package_exports.is_empty() {
            out.push_str(" <none>\n");
        } else {
            for hash in &unit.package_exports {
                out.push_str(&format!(" blake3:{}", hash));
            }
            out.push('\n');
        }
    }

    out.push_str("definitions\n");
    for definition in graph.definitions.values() {
        out.push_str(&format!(
            "  {} blake3:{}\n",
            visibility_str(definition.visibility),
            definition.hash
        ));
    }

    out.push_str("unit views\n");
    for unit in &graph.units {
        out.push_str(&format!("  blake3:{}\n", unit.hash));
        let rendered = emit_inspection(&unit.node, unit.sidecar.as_ref(), flags);
        for line in rendered.lines() {
            out.push_str("    ");
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn collect_tac_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), ProjectLoadError> {
    if should_skip_dir(dir) {
        return Ok(());
    }

    let mut entries = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|source| ProjectLoadError::Io {
        path: dir.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| ProjectLoadError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        entries.push(entry.path());
    }
    entries.sort();

    for path in entries {
        let metadata = std::fs::metadata(&path).map_err(|source| ProjectLoadError::Io {
            path: path.clone(),
            source,
        })?;
        if metadata.is_dir() {
            collect_tac_files(&path, out)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("tac") {
            out.push(path);
        }
    }

    Ok(())
}

#[derive(Serialize)]
struct DerivedProjectIndex {
    schema_version: &'static str,
    graph_hash: String,
    source_base: String,
    units: Vec<DerivedUnitIndex>,
    definitions: Vec<DerivedDefinitionIndex>,
}

impl DerivedProjectIndex {
    fn from_graph(graph: &ProjectGraph) -> Self {
        Self {
            schema_version: "phase6-project-v1",
            graph_hash: graph.graph_hash.clone(),
            source_base: relative_display(&graph.root, &graph.source_base),
            units: graph
                .units
                .iter()
                .map(DerivedUnitIndex::from_unit)
                .collect(),
            definitions: graph
                .definitions
                .values()
                .map(DerivedDefinitionIndex::from_definition)
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct DerivedUnitIndex {
    hash: String,
    source_paths: Vec<String>,
    definition_hashes: Vec<String>,
    public_exports: Vec<String>,
    package_exports: Vec<String>,
}

impl DerivedUnitIndex {
    fn from_unit(unit: &ProjectUnit) -> Self {
        Self {
            hash: unit.hash.clone(),
            source_paths: unit
                .source_paths
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
            definition_hashes: unit.definition_hashes.clone(),
            public_exports: unit.public_exports.iter().cloned().collect(),
            package_exports: unit.package_exports.iter().cloned().collect(),
        }
    }
}

#[derive(Serialize)]
struct DerivedDefinitionIndex {
    hash: String,
    visibility: &'static str,
    unit_hashes: Vec<String>,
}

impl DerivedDefinitionIndex {
    fn from_definition(definition: &ProjectDefinition) -> Self {
        Self {
            hash: definition.hash.clone(),
            visibility: visibility_str(definition.visibility),
            unit_hashes: definition.unit_hashes.iter().cloned().collect(),
        }
    }
}

fn should_skip_dir(dir: &Path) -> bool {
    matches!(
        dir.file_name().and_then(|name| name.to_str()),
        Some(".git") | Some(".tacit") | Some("target")
    ) || is_sealed_corpus_dir(dir)
}

fn is_sealed_corpus_dir(dir: &Path) -> bool {
    let mut prev_was_corpus = false;
    for component in dir.components() {
        let name = component.as_os_str().to_string_lossy();
        if prev_was_corpus && name == "sealed" {
            return true;
        }
        prev_was_corpus = name == "corpus";
    }
    false
}

fn read_fresh_sidecar(
    tac_path: &Path,
    canonical_bytes: &[u8],
) -> Result<Option<SidecarNode>, ProjectLoadError> {
    let tacd_path = tac_path.with_extension("tacd");
    if !tacd_path.exists() {
        return Ok(None);
    }
    let sidecar = Sidecar::read(&tacd_path).map_err(|error| ProjectLoadError::Sidecar {
        path: tacd_path,
        message: error.to_string(),
    })?;
    if sidecar.is_fresh(canonical_bytes) {
        Ok(Some(sidecar.display))
    } else {
        Ok(None)
    }
}

pub(crate) fn unit_boundary_hashes(
    unit: &Node,
) -> (Vec<String>, BTreeSet<String>, BTreeSet<String>) {
    let mut definition_hashes = Vec::new();
    let mut public_exports = BTreeSet::new();
    let mut package_exports = BTreeSet::new();

    if let Node::Unit { exports, defs, .. } = unit {
        definition_hashes = defs
            .iter()
            .filter(|entry| matches!(entry, Node::Def { .. }))
            .map(hex_hash_node)
            .collect();
        definition_hashes.sort();
        for export in exports {
            if let Node::Export { visibility, hash } = export {
                match visibility.as_str() {
                    "public" => {
                        public_exports.insert(hash.clone());
                    }
                    "package" => {
                        package_exports.insert(hash.clone());
                    }
                    _ => {}
                }
            }
        }
    }

    (definition_hashes, public_exports, package_exports)
}

pub(crate) fn build_definition_index(units: &[ProjectUnit]) -> BTreeMap<String, ProjectDefinition> {
    let mut definitions = BTreeMap::new();

    for unit in units {
        let Node::Unit { defs, .. } = &unit.node else {
            continue;
        };
        for def in defs {
            if !matches!(def, Node::Def { .. }) {
                continue;
            }
            let hash = hex_hash_node(def);
            let visibility = unit_visibility_for_hash(unit, &hash);
            definitions
                .entry(hash.clone())
                .and_modify(|entry: &mut ProjectDefinition| {
                    entry.visibility = max_visibility(entry.visibility, visibility);
                    entry.unit_hashes.insert(unit.hash.clone());
                })
                .or_insert_with(|| {
                    let mut unit_hashes = BTreeSet::new();
                    unit_hashes.insert(unit.hash.clone());
                    ProjectDefinition {
                        hash,
                        def: def.clone(),
                        visibility,
                        unit_hashes,
                    }
                });
        }
    }

    definitions
}

fn resolve_entry_hash(
    graph: &ProjectGraph,
    selector: Option<&str>,
) -> Result<String, ProjectEntryError> {
    let public_hashes: BTreeSet<String> = graph
        .units
        .iter()
        .flat_map(|unit| unit.public_exports.iter().cloned())
        .collect();

    let Some(selector) = selector else {
        return match public_hashes.len() {
            0 => Err(ProjectEntryError::NoPublicExports),
            1 => Ok(public_hashes.into_iter().next().expect("len checked")),
            _ => Err(ProjectEntryError::AmbiguousPublicExports(
                public_hashes.into_iter().collect(),
            )),
        };
    };

    if let Some(hash) = selector_hash(selector) {
        return if public_hashes.contains(&hash) {
            Ok(hash)
        } else {
            Err(ProjectEntryError::EntryNotFound(selector.to_string()))
        };
    }

    let mut matches = BTreeSet::new();
    for unit in &graph.units {
        let Some(sidecar) = unit.sidecar.as_ref() else {
            continue;
        };
        for hash in &unit.public_exports {
            if sidecar_alias_matches(sidecar, hash, selector) {
                matches.insert(hash.clone());
            }
        }
    }

    match matches.len() {
        0 => Err(ProjectEntryError::EntryNotFound(selector.to_string())),
        1 => Ok(matches.into_iter().next().expect("len checked")),
        _ => Err(ProjectEntryError::AmbiguousEntryAlias {
            alias: selector.to_string(),
            hashes: matches.into_iter().collect(),
        }),
    }
}

pub(crate) fn selector_hash(selector: &str) -> Option<String> {
    let raw = selector.strip_prefix("blake3:").unwrap_or(selector);
    is_hash_str(raw).then(|| raw.to_string())
}

pub(crate) fn is_hash_str(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn sidecar_alias_matches(sidecar: &SidecarNode, hash: &str, alias: &str) -> bool {
    sidecar
        .export_aliases
        .as_ref()
        .and_then(|aliases| aliases.get(hash))
        .is_some_and(|candidate| candidate == alias)
        || sidecar
            .definition_aliases
            .as_ref()
            .and_then(|aliases| aliases.get(hash))
            .is_some_and(|candidate| candidate == alias)
}

fn definition_value_type(hash: &str, def: &Node) -> Result<Ty, ProjectEntryError> {
    let Node::Def { sig, .. } = def else {
        return Err(ProjectEntryError::MissingDefinition(hash.to_string()));
    };
    let Node::Sig { type_, .. } = sig.as_ref() else {
        return Err(ProjectEntryError::InvalidSignature {
            hash: hash.to_string(),
            message: "definition child 0 is not a sig node".to_string(),
        });
    };

    let mut subst = Subst::default();
    let mut diags = Vec::new();
    let ty = type_from_node(type_, &[], &[], &mut subst, &[], &mut diags);
    if let Some(diag) = diags.into_iter().find(|diag| diag.severity == "error") {
        return Err(ProjectEntryError::InvalidSignature {
            hash: hash.to_string(),
            message: diag.message,
        });
    }
    Ok(subst.apply(&ty))
}

fn expanded_definition_body(
    graph: &ProjectGraph,
    hash: &str,
    stack: &mut Vec<String>,
    leaves: &BTreeSet<String>,
) -> Result<Node, ProjectEntryError> {
    if let Some(start) = stack.iter().position(|entry| entry == hash) {
        let mut cycle = stack[start..].to_vec();
        cycle.push(hash.to_string());
        return Err(ProjectEntryError::CyclicDependency(cycle));
    }

    let definition = graph
        .definitions
        .get(hash)
        .ok_or_else(|| ProjectEntryError::MissingDefinition(hash.to_string()))?;
    let Node::Def { body, .. } = &definition.def else {
        return Err(ProjectEntryError::MissingDefinition(hash.to_string()));
    };

    stack.push(hash.to_string());
    let expanded = expand_refs(graph, body, stack, 0, leaves)?;
    stack.pop();
    Ok(expanded)
}

fn expand_refs(
    graph: &ProjectGraph,
    node: &Node,
    stack: &mut Vec<String>,
    depth: u64,
    leaves: &BTreeSet<String>,
) -> Result<Node, ProjectEntryError> {
    match node {
        Node::Ref { hash } if leaves.contains(hash) => Ok(Node::Ref { hash: hash.clone() }),
        Node::Ref { hash } => {
            let expanded = expanded_definition_body(graph, hash, stack, leaves)?;
            Ok(shift_free_vars(&expanded, 0, depth))
        }
        Node::Lam { body } => Ok(Node::Lam {
            body: Box::new(expand_refs(graph, body, stack, depth + 1, leaves)?),
        }),
        Node::App { fn_, arg } => Ok(Node::App {
            fn_: Box::new(expand_refs(graph, fn_, stack, depth, leaves)?),
            arg: Box::new(expand_refs(graph, arg, stack, depth, leaves)?),
        }),
        Node::Let { rhs, body } => Ok(Node::Let {
            rhs: Box::new(expand_refs(graph, rhs, stack, depth, leaves)?),
            body: Box::new(expand_refs(graph, body, stack, depth + 1, leaves)?),
        }),
        Node::Rec { bindings, body } => {
            let inner = depth + bindings.len() as u64;
            Ok(Node::Rec {
                bindings: bindings
                    .iter()
                    .map(|binding| expand_refs(graph, binding, stack, inner, leaves))
                    .collect::<Result<Vec<_>, _>>()?,
                body: Box::new(expand_refs(graph, body, stack, inner, leaves)?),
            })
        }
        Node::Module { bindings } => {
            let inner = depth + bindings.len() as u64;
            Ok(Node::Module {
                bindings: bindings
                    .iter()
                    .map(|binding| expand_refs(graph, binding, stack, inner, leaves))
                    .collect::<Result<Vec<_>, _>>()?,
            })
        }
        Node::If { cond, then, else_ } => Ok(Node::If {
            cond: Box::new(expand_refs(graph, cond, stack, depth, leaves)?),
            then: Box::new(expand_refs(graph, then, stack, depth, leaves)?),
            else_: Box::new(expand_refs(graph, else_, stack, depth, leaves)?),
        }),
        Node::Match { scrutinee, arms } => Ok(Node::Match {
            scrutinee: Box::new(expand_refs(graph, scrutinee, stack, depth, leaves)?),
            arms: arms
                .iter()
                .map(|arm| expand_refs(graph, arm, stack, depth, leaves))
                .collect::<Result<Vec<_>, _>>()?,
        }),
        Node::Arm { pattern, body } => Ok(Node::Arm {
            pattern: pattern.clone(),
            body: Box::new(expand_refs(
                graph,
                body,
                stack,
                depth + count_pat_vars(pattern),
                leaves,
            )?),
        }),
        Node::Record { fields } => Ok(Node::Record {
            fields: fields
                .iter()
                .map(|(name, value)| {
                    Ok((
                        name.clone(),
                        expand_refs(graph, value, stack, depth, leaves)?,
                    ))
                })
                .collect::<Result<Vec<_>, ProjectEntryError>>()?,
        }),
        Node::Proj { record, field } => Ok(Node::Proj {
            record: Box::new(expand_refs(graph, record, stack, depth, leaves)?),
            field: field.clone(),
        }),
        Node::Ctor { name, args } => Ok(Node::Ctor {
            name: name.clone(),
            args: args
                .iter()
                .map(|arg| expand_refs(graph, arg, stack, depth, leaves))
                .collect::<Result<Vec<_>, _>>()?,
        }),
        Node::Ann { expr, type_ } => Ok(Node::Ann {
            expr: Box::new(expand_refs(graph, expr, stack, depth, leaves)?),
            type_: type_.clone(),
        }),
        Node::Def { sig, body } => Ok(Node::Def {
            sig: sig.clone(),
            body: Box::new(expand_refs(graph, body, stack, depth, leaves)?),
        }),
        _ => Ok(node.clone()),
    }
}

fn shift_free_vars(node: &Node, cutoff: u64, amount: u64) -> Node {
    match node {
        Node::Var { index } if *index >= cutoff => Node::Var {
            index: index + amount,
        },
        Node::Var { .. } => node.clone(),
        Node::Lam { body } => Node::Lam {
            body: Box::new(shift_free_vars(body, cutoff + 1, amount)),
        },
        Node::App { fn_, arg } => Node::App {
            fn_: Box::new(shift_free_vars(fn_, cutoff, amount)),
            arg: Box::new(shift_free_vars(arg, cutoff, amount)),
        },
        Node::Let { rhs, body } => Node::Let {
            rhs: Box::new(shift_free_vars(rhs, cutoff, amount)),
            body: Box::new(shift_free_vars(body, cutoff + 1, amount)),
        },
        Node::Rec { bindings, body } => {
            let inner = cutoff + bindings.len() as u64;
            Node::Rec {
                bindings: bindings
                    .iter()
                    .map(|binding| shift_free_vars(binding, inner, amount))
                    .collect(),
                body: Box::new(shift_free_vars(body, inner, amount)),
            }
        }
        Node::Module { bindings } => {
            let inner = cutoff + bindings.len() as u64;
            Node::Module {
                bindings: bindings
                    .iter()
                    .map(|binding| shift_free_vars(binding, inner, amount))
                    .collect(),
            }
        }
        Node::If { cond, then, else_ } => Node::If {
            cond: Box::new(shift_free_vars(cond, cutoff, amount)),
            then: Box::new(shift_free_vars(then, cutoff, amount)),
            else_: Box::new(shift_free_vars(else_, cutoff, amount)),
        },
        Node::Match { scrutinee, arms } => Node::Match {
            scrutinee: Box::new(shift_free_vars(scrutinee, cutoff, amount)),
            arms: arms
                .iter()
                .map(|arm| shift_free_vars(arm, cutoff, amount))
                .collect(),
        },
        Node::Arm { pattern, body } => Node::Arm {
            pattern: pattern.clone(),
            body: Box::new(shift_free_vars(
                body,
                cutoff + count_pat_vars(pattern),
                amount,
            )),
        },
        Node::Record { fields } => Node::Record {
            fields: fields
                .iter()
                .map(|(name, value)| (name.clone(), shift_free_vars(value, cutoff, amount)))
                .collect(),
        },
        Node::Proj { record, field } => Node::Proj {
            record: Box::new(shift_free_vars(record, cutoff, amount)),
            field: field.clone(),
        },
        Node::Ctor { name, args } => Node::Ctor {
            name: name.clone(),
            args: args
                .iter()
                .map(|arg| shift_free_vars(arg, cutoff, amount))
                .collect(),
        },
        Node::Ann { expr, type_ } => Node::Ann {
            expr: Box::new(shift_free_vars(expr, cutoff, amount)),
            type_: type_.clone(),
        },
        Node::Def { sig, body } => Node::Def {
            sig: sig.clone(),
            body: Box::new(shift_free_vars(body, cutoff, amount)),
        },
        _ => node.clone(),
    }
}

fn count_pat_vars(node: &Node) -> u64 {
    match node {
        Node::PatVar => 1,
        Node::PatCtor { sub_patterns, .. } => sub_patterns.iter().map(count_pat_vars).sum(),
        _ => 0,
    }
}

fn unit_visibility_for_hash(unit: &ProjectUnit, hash: &str) -> DefinitionVisibility {
    if unit.public_exports.contains(hash) {
        DefinitionVisibility::Public
    } else if unit.package_exports.contains(hash) {
        DefinitionVisibility::Package
    } else {
        DefinitionVisibility::Private
    }
}

fn visibility_str(visibility: DefinitionVisibility) -> &'static str {
    match visibility {
        DefinitionVisibility::Public => "public",
        DefinitionVisibility::Package => "package",
        DefinitionVisibility::Private => "private",
    }
}

fn relative_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

pub(crate) fn max_visibility(
    left: DefinitionVisibility,
    right: DefinitionVisibility,
) -> DefinitionVisibility {
    if visibility_rank(right) > visibility_rank(left) {
        right
    } else {
        left
    }
}

fn visibility_rank(visibility: DefinitionVisibility) -> u8 {
    match visibility {
        DefinitionVisibility::Private => 0,
        DefinitionVisibility::Package => 1,
        DefinitionVisibility::Public => 2,
    }
}

pub(crate) fn project_graph_hash<'a>(unit_hashes: impl Iterator<Item = &'a str>) -> String {
    graph_hash_with_tag("tacit-project-v1", unit_hashes)
}

pub(crate) fn graph_hash_with_tag<'a>(
    tag: &str,
    unit_hashes: impl Iterator<Item = &'a str>,
) -> String {
    let mut bytes = tag.as_bytes().to_vec();
    bytes.push(b'\n');
    for hash in unit_hashes {
        bytes.extend_from_slice(hash.as_bytes());
        bytes.push(b'\n');
    }
    hex_hash_bytes(&bytes)
}

fn hex_hash_node(node: &Node) -> String {
    hash_node(node)
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect()
}

pub(crate) fn hex_hash_bytes(bytes: &[u8]) -> String {
    hash_bytes(bytes)
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect()
}
