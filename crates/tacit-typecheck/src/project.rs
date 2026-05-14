//! Whole-project graph loading and checking.
//!
//! Stage 2 keeps project composition manifestless: a project root contributes
//! canonical `unit` artifacts from `.tac` files, then the graph is indexed and
//! checked by content hash rather than file path.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use tacit_canonical::ast::Node;
use tacit_canonical::{emit, hash_bytes, hash_node, parse, ParseError};
use tacit_views::sidecar::{Sidecar, SidecarNode};

use crate::error::Diagnostic;
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

fn unit_boundary_hashes(unit: &Node) -> (Vec<String>, BTreeSet<String>, BTreeSet<String>) {
    let mut definition_hashes = Vec::new();
    let mut public_exports = BTreeSet::new();
    let mut package_exports = BTreeSet::new();

    if let Node::Unit { exports, defs, .. } = unit {
        definition_hashes = defs.iter().map(hex_hash_node).collect();
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

fn build_definition_index(units: &[ProjectUnit]) -> BTreeMap<String, ProjectDefinition> {
    let mut definitions = BTreeMap::new();

    for unit in units {
        let Node::Unit { defs, .. } = &unit.node else {
            continue;
        };
        for def in defs {
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

fn unit_visibility_for_hash(unit: &ProjectUnit, hash: &str) -> DefinitionVisibility {
    if unit.public_exports.contains(hash) {
        DefinitionVisibility::Public
    } else if unit.package_exports.contains(hash) {
        DefinitionVisibility::Package
    } else {
        DefinitionVisibility::Private
    }
}

fn max_visibility(left: DefinitionVisibility, right: DefinitionVisibility) -> DefinitionVisibility {
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

fn project_graph_hash<'a>(unit_hashes: impl Iterator<Item = &'a str>) -> String {
    let mut bytes = b"tacit-project-v1\n".to_vec();
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

fn hex_hash_bytes(bytes: &[u8]) -> String {
    hash_bytes(bytes)
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect()
}
