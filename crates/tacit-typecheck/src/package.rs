//! Package manifest, lockfile, dependency cache, and package graph support.
#![allow(clippy::result_large_err)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tacit_canonical::ast::Node;
use tacit_canonical::{emit, parse};
use tacit_views::sidecar::{Sidecar, SidecarNode};

use crate::error::{Diagnostic, PathStep};
use crate::project::{
    build_definition_index, graph_hash_with_tag, hex_hash_bytes, is_hash_str, load_project,
    materialize_project_derived, max_visibility, project_definition_expression,
    project_entry_expression, selector_hash, unit_boundary_hashes, ProjectDefinition,
    ProjectDerivedError, ProjectEntry, ProjectEntryError, ProjectGraph, ProjectUnit,
};
use crate::ty::{EffAtom, EffSet};
use crate::units::{check_unit_with_sidecar, CheckedUnit, DefinitionEnv, ProvidedDefinition};

const MANIFEST_FILE: &str = "tacit.toml";
const LOCK_FILE: &str = "tacit.lock";
const LOCK_FORMAT: &str = "tacit-lock-v1";
const PACKAGE_FORMAT: &str = "tacit-package-v1";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackageManifest {
    pub package: Option<PackageMetadata>,
    pub dependencies: BTreeMap<String, DependencySpec>,
    pub exports: BTreeMap<String, String>,
    pub bin: BTreeMap<String, String>,
    pub tests: Vec<PackageTest>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageMetadata {
    pub name: Option<String>,
    pub description: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencySpec {
    Hash {
        hash: String,
        registry: Option<String>,
        name: Option<String>,
    },
    Path {
        path: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageTest {
    pub name: String,
    pub target: String,
    pub effects: EffSet,
    pub step_budget: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Lockfile {
    pub format: String,
    pub package: LockPackage,
    pub dependencies: Vec<LockDependency>,
    pub transitive: Vec<LockTransitive>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LockPackage {
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LockDependency {
    pub alias: String,
    pub hash: String,
    pub source: LockSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LockTransitive {
    pub hash: String,
    pub source: LockSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LockSource {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PackageGraph {
    pub root: ProjectGraph,
    pub package_hash: String,
    pub manifest: PackageManifest,
    pub lockfile: Option<Lockfile>,
    pub dependencies: Vec<CachedPackage>,
    pub cache_root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct CachedPackage {
    pub hash: String,
    pub units: Vec<ProjectUnit>,
    pub definitions: BTreeMap<String, ProjectDefinition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LockMode {
    Verify,
    Write,
}

#[derive(Debug)]
pub enum PackageCacheError {
    Io { path: PathBuf, source: io::Error },
    InvalidHash(String),
}

impl fmt::Display for PackageCacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackageCacheError::Io { path, source } => write!(f, "{}: {}", path.display(), source),
            PackageCacheError::InvalidHash(hash) => write!(f, "invalid blake3 hash {:?}", hash),
        }
    }
}

impl std::error::Error for PackageCacheError {}

#[derive(Debug, Clone)]
struct ManifestSource {
    manifest: PackageManifest,
    text: Option<String>,
}

#[derive(Debug, Clone)]
struct PackageCache {
    root: PathBuf,
}

struct ResolvedDependencies {
    lock_dependencies: Vec<LockDependency>,
    lock_transitive: Vec<LockTransitive>,
    cached_packages: Vec<CachedPackage>,
}

pub fn load_package(root: impl AsRef<Path>) -> Result<PackageGraph, Vec<Diagnostic>> {
    load_package_with_mode(root.as_ref(), LockMode::Verify, &mut Vec::new())
}

pub fn lock_package(root: impl AsRef<Path>) -> Result<PackageGraph, Vec<Diagnostic>> {
    load_package_with_mode(root.as_ref(), LockMode::Write, &mut Vec::new())
}

pub fn check_package(package: &PackageGraph) -> Result<Vec<CheckedUnit>, Vec<Diagnostic>> {
    let env = package.definition_env();
    let mut checked = Vec::new();
    let mut diags = Vec::new();

    for (unit_index, unit) in package.root.units.iter().enumerate() {
        match check_unit_with_sidecar(&unit.node, &env, unit.sidecar.as_ref()) {
            Ok(unit) => checked.push(unit),
            Err(mut errors) => {
                for error in &mut errors {
                    error
                        .location
                        .ast_path
                        .insert(0, PathStep { child: unit_index });
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

pub fn materialize_package_derived(package: &PackageGraph) -> Result<PathBuf, ProjectDerivedError> {
    let mut graph = package.root.clone();
    graph.graph_hash = package.package_hash.clone();
    materialize_project_derived(&graph)
}

pub fn package_entry_expression(
    package: &PackageGraph,
    selector: Option<&str>,
) -> Result<ProjectEntry, ProjectEntryError> {
    let selector = package.resolve_entry_selector(selector)?;
    let graph = package.linked_project_graph();
    project_entry_expression(&graph, selector.as_deref())
}

pub fn package_test_entry_expression(
    package: &PackageGraph,
    target_hash: &str,
) -> Result<ProjectEntry, ProjectEntryError> {
    if !package.root.definitions.contains_key(target_hash) {
        return Err(ProjectEntryError::MissingDefinition(
            target_hash.to_string(),
        ));
    }
    let graph = package.linked_project_graph();
    project_definition_expression(&graph, target_hash)
}

pub fn clear_package_cache(root: impl AsRef<Path>) -> Result<(), PackageCacheError> {
    let cache = PackageCache::for_project_root(root.as_ref());
    if cache.root.exists() {
        std::fs::remove_dir_all(&cache.root).map_err(|source| PackageCacheError::Io {
            path: cache.root.clone(),
            source,
        })?;
    }
    Ok(())
}

pub fn evict_package_cache(root: impl AsRef<Path>, hash: &str) -> Result<(), PackageCacheError> {
    let hash = normalize_hash_selector(hash)
        .ok_or_else(|| PackageCacheError::InvalidHash(hash.to_string()))?;
    let cache = PackageCache::for_project_root(root.as_ref());
    for path in [
        cache
            .root
            .join("objects")
            .join("units")
            .join(format!("{hash}.tac")),
        cache
            .root
            .join("objects")
            .join("defs")
            .join(format!("{hash}.tac")),
        cache
            .root
            .join("objects")
            .join("sidecars")
            .join(format!("{hash}.tacd")),
        cache.root.join("packages").join(&hash),
    ] {
        if path.is_dir() {
            std::fs::remove_dir_all(&path).map_err(|source| PackageCacheError::Io {
                path: path.clone(),
                source,
            })?;
        } else if path.exists() {
            std::fs::remove_file(&path).map_err(|source| PackageCacheError::Io {
                path: path.clone(),
                source,
            })?;
        }
    }
    Ok(())
}

pub fn seed_package_cache(
    root: impl AsRef<Path>,
    package: &CachedPackage,
    manifest_text: Option<&str>,
) -> Result<(), Diagnostic> {
    let cache = PackageCache::for_project_root(root.as_ref());
    cache.materialize_cached_package(package)?;
    if let Some(text) = manifest_text {
        let path = cache
            .root
            .join("packages")
            .join(&package.hash)
            .join("manifest.toml");
        write_atomic(&path, text.as_bytes()).map_err(|source| {
            io_diag(
                "cache-corruption",
                &path,
                source,
                "failed to write manifest snapshot",
            )
        })?;
    }
    Ok(())
}

pub fn package_hash_for_project(graph: &ProjectGraph) -> String {
    package_hash_from_hashes(graph.units.iter().map(|unit| unit.hash.as_str()))
}

impl PackageGraph {
    fn definition_env(&self) -> DefinitionEnv {
        let mut env = self.root.definition_env();
        for package in &self.dependencies {
            for (hash, definition) in &package.definitions {
                env.entry(hash.clone())
                    .and_modify(|existing| {
                        existing.visibility =
                            max_visibility(existing.visibility, definition.visibility);
                    })
                    .or_insert_with(|| {
                        ProvidedDefinition::new(
                            definition.def.clone(),
                            definition.visibility,
                            false,
                        )
                    });
            }
        }
        env
    }

    fn linked_project_graph(&self) -> ProjectGraph {
        let mut graph = self.root.clone();
        for package in &self.dependencies {
            for (hash, definition) in &package.definitions {
                graph
                    .definitions
                    .entry(hash.clone())
                    .or_insert_with(|| definition.clone());
            }
        }
        graph
    }

    fn resolve_entry_selector(
        &self,
        selector: Option<&str>,
    ) -> Result<Option<String>, ProjectEntryError> {
        let selector = match selector {
            Some(selector) => selector,
            None if self.manifest.bin.len() == 1 => {
                let value = self.manifest.bin.values().next().expect("len checked");
                return Ok(Some(prefix_hash(
                    &resolve_bin_value(&self.manifest, value)
                        .ok_or_else(|| ProjectEntryError::EntryNotFound(value.clone()))?,
                )));
            }
            None => return Ok(None),
        };

        if let Some(hash) = self.manifest.bin.get(selector) {
            return Ok(Some(prefix_hash(
                &resolve_bin_value(&self.manifest, hash)
                    .ok_or_else(|| ProjectEntryError::EntryNotFound(selector.to_string()))?,
            )));
        }

        if let Some(hash) = self.manifest.exports.get(selector) {
            return Ok(Some(prefix_hash(hash)));
        }

        Ok(Some(selector.to_string()))
    }
}

impl CachedPackage {
    fn from_project_graph(graph: &ProjectGraph, package_hash: String) -> Self {
        Self {
            hash: package_hash,
            units: graph.units.clone(),
            definitions: graph.definitions.clone(),
        }
    }
}

impl PackageCache {
    fn for_project_root(root: &Path) -> Self {
        Self {
            root: root.join(".tacit").join("cache"),
        }
    }

    fn materialize_project_package(
        &self,
        graph: &ProjectGraph,
        package_hash: &str,
        manifest_text: Option<&str>,
    ) -> Result<(), Diagnostic> {
        let package = CachedPackage::from_project_graph(graph, package_hash.to_string());
        self.materialize_cached_package(&package)?;
        if let Some(text) = manifest_text {
            let path = self
                .root
                .join("packages")
                .join(package_hash)
                .join("manifest.toml");
            write_atomic(&path, text.as_bytes()).map_err(|source| {
                io_diag(
                    "cache-corruption",
                    &path,
                    source,
                    "failed to write manifest snapshot",
                )
            })?;
        }
        Ok(())
    }

    fn materialize_cached_package(&self, package: &CachedPackage) -> Result<(), Diagnostic> {
        for unit in &package.units {
            let bytes = emit(&unit.node);
            let unit_path = self
                .root
                .join("objects")
                .join("units")
                .join(format!("{}.tac", unit.hash));
            write_atomic(&unit_path, &bytes).map_err(|source| {
                io_diag(
                    "cache-corruption",
                    &unit_path,
                    source,
                    "failed to write unit object",
                )
            })?;

            if let Some(sidecar) = &unit.sidecar {
                let sidecar_path = self
                    .root
                    .join("objects")
                    .join("sidecars")
                    .join(format!("{}.tacd", unit.hash));
                let sidecar = Sidecar::new(&bytes, sidecar.clone());
                let json = serde_json::to_vec_pretty(&sidecar).map_err(|source| {
                    package_diag(
                        "cache-corruption",
                        format!(
                            "failed to serialize sidecar {}: {}",
                            sidecar_path.display(),
                            source
                        ),
                        None,
                        None,
                        Some(&sidecar_path),
                    )
                })?;
                write_atomic(&sidecar_path, &with_trailing_newline(json)).map_err(|source| {
                    io_diag(
                        "cache-corruption",
                        &sidecar_path,
                        source,
                        "failed to write sidecar object",
                    )
                })?;
            }
        }

        for definition in package.definitions.values() {
            let path = self
                .root
                .join("objects")
                .join("defs")
                .join(format!("{}.tac", definition.hash));
            write_atomic(&path, &emit(&definition.def)).map_err(|source| {
                io_diag(
                    "cache-corruption",
                    &path,
                    source,
                    "failed to write definition object",
                )
            })?;
        }

        let index = PackageIndex::from_cached(package);
        let index_json = serde_json::to_vec_pretty(&index).map_err(|source| {
            package_diag(
                "cache-corruption",
                format!(
                    "failed to serialize package index blake3:{}: {}",
                    package.hash, source
                ),
                None,
                Some(&package.hash),
                None,
            )
        })?;
        let path = self
            .root
            .join("packages")
            .join(&package.hash)
            .join("package.json");
        write_atomic(&path, &with_trailing_newline(index_json)).map_err(|source| {
            io_diag(
                "cache-corruption",
                &path,
                source,
                "failed to write package index",
            )
        })?;

        Ok(())
    }

    fn load_cached_package(
        &self,
        hash: &str,
        missing_kind: &str,
    ) -> Result<CachedPackage, Diagnostic> {
        let index_path = self.root.join("packages").join(hash).join("package.json");
        if !index_path.exists() {
            return Err(package_diag(
                missing_kind,
                format!("cached package blake3:{} is not present", hash),
                None,
                Some(hash),
                Some(&index_path),
            ));
        }

        let bytes = std::fs::read(&index_path).map_err(|source| {
            io_diag(
                "cache-corruption",
                &index_path,
                source,
                "failed to read package index",
            )
        })?;
        let index: PackageIndex = serde_json::from_slice(&bytes).map_err(|source| {
            package_diag(
                "cache-corruption",
                format!(
                    "cached package index {} is invalid JSON: {}",
                    index_path.display(),
                    source
                ),
                None,
                Some(hash),
                Some(&index_path),
            )
        })?;
        index.validate(hash, &index_path)?;

        let unit_hashes = index
            .units
            .iter()
            .map(|hash| {
                parse_prefixed_hash(hash).ok_or_else(|| {
                    package_diag(
                        "cache-corruption",
                        format!(
                            "cached package index {} contains invalid unit hash {}",
                            index_path.display(),
                            hash
                        ),
                        None,
                        None,
                        Some(&index_path),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let computed = package_hash_from_hashes(unit_hashes.iter().map(String::as_str));
        if computed != hash {
            return Err(package_diag(
                "cache-corruption",
                format!(
                    "cached package index blake3:{} recomputes to blake3:{}",
                    hash, computed
                ),
                None,
                Some(hash),
                Some(&index_path),
            ));
        }

        let mut units = Vec::new();
        for unit_hash in unit_hashes {
            units.push(self.load_cached_unit(&unit_hash)?);
        }
        let definitions = build_definition_index(&units);
        for definition in definitions.values() {
            self.read_verified_object("defs", &definition.hash, "tac", "cache-missing-object")?;
        }

        Ok(CachedPackage {
            hash: hash.to_string(),
            units,
            definitions,
        })
    }

    fn load_cached_unit(&self, hash: &str) -> Result<ProjectUnit, Diagnostic> {
        let bytes = self.read_verified_object("units", hash, "tac", "cache-missing-object")?;
        let node = parse(&bytes).map_err(|source| {
            package_diag(
                "cache-corruption",
                format!("cached unit blake3:{} does not parse: {}", hash, source),
                None,
                Some(hash),
                None,
            )
        })?;
        if !matches!(node, Node::Unit { .. }) {
            return Err(package_diag(
                "cache-corruption",
                format!("cached unit blake3:{} is not a unit artifact", hash),
                None,
                Some(hash),
                None,
            ));
        }
        let (definition_hashes, public_exports, package_exports) = unit_boundary_hashes(&node);
        let sidecar = self.read_cached_sidecar(hash, &bytes);
        Ok(ProjectUnit {
            hash: hash.to_string(),
            node,
            sidecar,
            source_paths: vec![PathBuf::from(format!(
                ".tacit/cache/objects/units/{hash}.tac"
            ))],
            definition_hashes,
            public_exports,
            package_exports,
        })
    }

    fn read_cached_sidecar(&self, hash: &str, canonical_bytes: &[u8]) -> Option<SidecarNode> {
        let path = self
            .root
            .join("objects")
            .join("sidecars")
            .join(format!("{hash}.tacd"));
        let sidecar = Sidecar::read(&path).ok()?;
        sidecar.is_fresh(canonical_bytes).then_some(sidecar.display)
    }

    fn read_verified_object(
        &self,
        object_kind: &str,
        hash: &str,
        extension: &str,
        missing_kind: &str,
    ) -> Result<Vec<u8>, Diagnostic> {
        let path = self
            .root
            .join("objects")
            .join(object_kind)
            .join(format!("{hash}.{extension}"));
        if !path.exists() {
            return Err(package_diag(
                missing_kind,
                format!("cached object blake3:{} is missing", hash),
                None,
                Some(hash),
                Some(&path),
            ));
        }

        let bytes = std::fs::read(&path).map_err(|source| {
            io_diag(
                "cache-corruption",
                &path,
                source,
                "failed to read cached object",
            )
        })?;
        let actual = hex_hash_bytes(&bytes);
        if actual != hash {
            let _ = self.quarantine(&path);
            return Err(package_diag(
                "cache-corruption",
                format!(
                    "cached object {} expected blake3:{} but read blake3:{}",
                    path.display(),
                    hash,
                    actual
                ),
                None,
                Some(hash),
                Some(&path),
            ));
        }
        Ok(bytes)
    }

    fn quarantine(&self, path: &Path) -> io::Result<()> {
        let trash = self.root.join("trash");
        std::fs::create_dir_all(&trash)?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("object");
        std::fs::rename(path, trash.join(format!("{stamp}-{name}")))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PackageIndex {
    format: String,
    hash: String,
    units: Vec<String>,
    public_exports: Vec<String>,
    package_exports: Vec<String>,
}

impl PackageIndex {
    fn from_cached(package: &CachedPackage) -> Self {
        let mut units: Vec<_> = package
            .units
            .iter()
            .map(|unit| prefix_hash(&unit.hash))
            .collect();
        units.sort();
        let mut public_exports = BTreeSet::new();
        let mut package_exports = BTreeSet::new();
        for unit in &package.units {
            public_exports.extend(unit.public_exports.iter().map(|hash| prefix_hash(hash)));
            package_exports.extend(unit.package_exports.iter().map(|hash| prefix_hash(hash)));
        }
        Self {
            format: PACKAGE_FORMAT.to_string(),
            hash: prefix_hash(&package.hash),
            units,
            public_exports: public_exports.into_iter().collect(),
            package_exports: package_exports.into_iter().collect(),
        }
    }

    fn validate(&self, expected_hash: &str, path: &Path) -> Result<(), Diagnostic> {
        if self.format != PACKAGE_FORMAT {
            return Err(package_diag(
                "cache-corruption",
                format!(
                    "cached package index {} has unsupported format {}",
                    path.display(),
                    self.format
                ),
                None,
                Some(expected_hash),
                Some(path),
            ));
        }
        let Some(hash) = parse_prefixed_hash(&self.hash) else {
            return Err(package_diag(
                "cache-corruption",
                format!(
                    "cached package index {} has invalid package hash {}",
                    path.display(),
                    self.hash
                ),
                None,
                Some(expected_hash),
                Some(path),
            ));
        };
        if hash != expected_hash {
            return Err(package_diag(
                "cache-corruption",
                format!(
                    "cached package index {} lives under blake3:{} but names blake3:{}",
                    path.display(),
                    expected_hash,
                    hash
                ),
                None,
                Some(expected_hash),
                Some(path),
            ));
        }
        Ok(())
    }
}

fn load_package_with_mode(
    root: &Path,
    mode: LockMode,
    stack: &mut Vec<PathBuf>,
) -> Result<PackageGraph, Vec<Diagnostic>> {
    let graph = load_project(root).map_err(|error| {
        vec![package_diag(
            "dependency-unresolved",
            error.to_string(),
            None,
            None,
            Some(root),
        )]
    })?;

    if stack.iter().any(|entry| entry == &graph.root) {
        return Err(vec![package_diag(
            "circular-package-dependency",
            format!("package dependency cycle reaches {}", graph.root.display()),
            None,
            None,
            Some(&graph.root),
        )]);
    }
    stack.push(graph.root.clone());

    let manifest_source = match read_manifest(&graph.root) {
        Ok(manifest) => manifest,
        Err(diag) => {
            stack.pop();
            return Err(vec![diag]);
        }
    };
    let mut diags = validate_manifest_entries(&manifest_source.manifest, &graph);

    let package_hash = package_hash_for_project(&graph);
    let cache = PackageCache::for_project_root(&graph.root);
    let mut resolved =
        match resolve_dependencies(&manifest_source.manifest, &graph.root, &cache, mode, stack) {
            Ok(resolved) => resolved,
            Err(mut errors) => {
                stack.pop();
                diags.append(&mut errors);
                return Err(diags);
            }
        };

    let expected_lock = Lockfile::new(
        package_hash.clone(),
        resolved.lock_dependencies,
        resolved.lock_transitive,
    );
    let lock_path = graph.root.join(LOCK_FILE);
    let lockfile = match mode {
        LockMode::Write => {
            if let Err(diag) = write_lockfile(&lock_path, &expected_lock) {
                stack.pop();
                diags.push(diag);
                return Err(diags);
            }
            Some(expected_lock)
        }
        LockMode::Verify => match verify_lockfile(
            &lock_path,
            &expected_lock,
            !manifest_source.manifest.dependencies.is_empty(),
        ) {
            Ok(lockfile) => {
                if let Some(lockfile) = &lockfile {
                    match load_locked_transitives(&cache, lockfile, &mut resolved.cached_packages) {
                        Ok(()) => {}
                        Err(diag) => {
                            stack.pop();
                            diags.push(diag);
                            return Err(diags);
                        }
                    }
                }
                lockfile
            }
            Err(diag) => {
                stack.pop();
                diags.push(diag);
                return Err(diags);
            }
        },
    };

    if let Err(diag) =
        cache.materialize_project_package(&graph, &package_hash, manifest_source.text.as_deref())
    {
        stack.pop();
        diags.push(diag);
        return Err(diags);
    }

    stack.pop();

    if diags.is_empty() {
        Ok(PackageGraph {
            root: graph,
            package_hash,
            manifest: manifest_source.manifest,
            lockfile,
            dependencies: resolved.cached_packages,
            cache_root: cache.root,
        })
    } else {
        Err(diags)
    }
}

fn resolve_dependencies(
    manifest: &PackageManifest,
    root: &Path,
    cache: &PackageCache,
    mode: LockMode,
    stack: &mut Vec<PathBuf>,
) -> Result<ResolvedDependencies, Vec<Diagnostic>> {
    let mut lock_dependencies = Vec::new();
    let mut transitive = BTreeMap::<String, LockSource>::new();
    let mut cached_packages = BTreeMap::<String, CachedPackage>::new();
    let mut diags = Vec::new();

    for (alias, dep) in &manifest.dependencies {
        match dep {
            DependencySpec::Hash {
                hash,
                registry,
                name,
            } => {
                let source = LockSource::cache(registry.clone(), name.clone());
                lock_dependencies.push(LockDependency {
                    alias: alias.clone(),
                    hash: prefix_hash(hash),
                    source,
                });
                match cache.load_cached_package(hash, "dependency-unresolved") {
                    Ok(package) => {
                        cached_packages.insert(package.hash.clone(), package);
                    }
                    Err(diag) => diags.push(with_alias(diag, alias)),
                }
            }
            DependencySpec::Path { path } => {
                let dep_path = root.join(path);
                let dep_root = match std::fs::canonicalize(&dep_path) {
                    Ok(path) => path,
                    Err(source) => {
                        diags.push(io_diag(
                            "dependency-unresolved",
                            &dep_path,
                            source,
                            "failed to resolve path dependency",
                        ));
                        continue;
                    }
                };
                match load_package_with_mode(&dep_root, mode, stack) {
                    Ok(package) => {
                        let hash = package.package_hash.clone();
                        lock_dependencies.push(LockDependency {
                            alias: alias.clone(),
                            hash: prefix_hash(&hash),
                            source: LockSource::path(path.clone()),
                        });
                        for dep in &package.dependencies {
                            transitive
                                .entry(dep.hash.clone())
                                .or_insert_with(|| LockSource::cache(None, None));
                            if let Err(diag) = cache.materialize_cached_package(dep) {
                                diags.push(diag);
                            }
                            cached_packages.insert(dep.hash.clone(), dep.clone());
                        }
                        let cached = CachedPackage::from_project_graph(&package.root, hash);
                        if let Err(diag) = cache.materialize_cached_package(&cached) {
                            diags.push(diag);
                        }
                        cached_packages.insert(cached.hash.clone(), cached);
                    }
                    Err(mut errors) => diags.append(&mut errors),
                }
            }
        }
    }

    lock_dependencies.sort_by(|left, right| left.alias.cmp(&right.alias));
    let direct_hashes: BTreeSet<String> = lock_dependencies
        .iter()
        .filter_map(|entry| parse_prefixed_hash(&entry.hash))
        .collect();
    let lock_transitive = transitive
        .into_iter()
        .filter(|(hash, _)| !direct_hashes.contains(hash))
        .map(|(hash, source)| LockTransitive {
            hash: prefix_hash(&hash),
            source,
        })
        .collect();

    if diags.is_empty() {
        Ok(ResolvedDependencies {
            lock_dependencies,
            lock_transitive,
            cached_packages: cached_packages.into_values().collect(),
        })
    } else {
        Err(diags)
    }
}

fn load_locked_transitives(
    cache: &PackageCache,
    lockfile: &Lockfile,
    packages: &mut Vec<CachedPackage>,
) -> Result<(), Diagnostic> {
    let mut known: BTreeSet<String> = packages
        .iter()
        .map(|package| package.hash.clone())
        .collect();
    for entry in &lockfile.transitive {
        let Some(hash) = parse_prefixed_hash(&entry.hash) else {
            return Err(package_diag(
                "lockfile-parse",
                format!("lockfile contains invalid transitive hash {}", entry.hash),
                None,
                None,
                None,
            ));
        };
        if known.insert(hash.clone()) {
            packages.push(cache.load_cached_package(&hash, "cache-missing-object")?);
        }
    }
    Ok(())
}

impl Lockfile {
    fn new(
        package_hash: String,
        dependencies: Vec<LockDependency>,
        mut transitive: Vec<LockTransitive>,
    ) -> Self {
        transitive.sort_by(|left, right| left.hash.cmp(&right.hash));
        transitive.dedup_by(|left, right| left.hash == right.hash);
        Self {
            format: LOCK_FORMAT.to_string(),
            package: LockPackage {
                hash: prefix_hash(&package_hash),
            },
            dependencies,
            transitive,
        }
    }

    fn validate(&self, path: &Path) -> Result<(), Diagnostic> {
        if self.format != LOCK_FORMAT {
            return Err(package_diag(
                "lockfile-parse",
                format!(
                    "{} uses unsupported lockfile format {}",
                    path.display(),
                    self.format
                ),
                None,
                None,
                Some(path),
            ));
        }
        if parse_prefixed_hash(&self.package.hash).is_none() {
            return Err(package_diag(
                "lockfile-parse",
                format!(
                    "{} has invalid package hash {}",
                    path.display(),
                    self.package.hash
                ),
                None,
                None,
                Some(path),
            ));
        }
        let mut aliases = BTreeSet::new();
        for dep in &self.dependencies {
            if !aliases.insert(dep.alias.clone()) {
                return Err(package_diag(
                    "lockfile-parse",
                    format!("{} repeats dependency alias {}", path.display(), dep.alias),
                    Some(&dep.alias),
                    None,
                    Some(path),
                ));
            }
            if parse_prefixed_hash(&dep.hash).is_none() {
                return Err(package_diag(
                    "lockfile-parse",
                    format!(
                        "{} has invalid dependency hash {}",
                        path.display(),
                        dep.hash
                    ),
                    Some(&dep.alias),
                    None,
                    Some(path),
                ));
            }
            dep.source.validate(path)?;
        }
        for dep in &self.transitive {
            if parse_prefixed_hash(&dep.hash).is_none() {
                return Err(package_diag(
                    "lockfile-parse",
                    format!(
                        "{} has invalid transitive hash {}",
                        path.display(),
                        dep.hash
                    ),
                    None,
                    None,
                    Some(path),
                ));
            }
            dep.source.validate(path)?;
        }
        Ok(())
    }
}

impl LockSource {
    fn cache(registry: Option<String>, name: Option<String>) -> Self {
        Self {
            kind: "cache".to_string(),
            name,
            path: None,
            registry,
        }
    }

    fn path(path: String) -> Self {
        Self {
            kind: "path".to_string(),
            name: None,
            path: Some(path),
            registry: None,
        }
    }

    fn validate(&self, path: &Path) -> Result<(), Diagnostic> {
        match self.kind.as_str() {
            "cache" if self.path.is_none() => Ok(()),
            "path" if self.path.is_some() && self.registry.is_none() && self.name.is_none() => {
                Ok(())
            }
            _ => Err(package_diag(
                "lockfile-parse",
                format!(
                    "{} has invalid lock source kind {}",
                    path.display(),
                    self.kind
                ),
                None,
                None,
                Some(path),
            )),
        }
    }
}

fn read_manifest(root: &Path) -> Result<ManifestSource, Diagnostic> {
    let path = root.join(MANIFEST_FILE);
    if !path.exists() {
        return Ok(ManifestSource {
            manifest: PackageManifest::default(),
            text: None,
        });
    }
    let text = std::fs::read_to_string(&path).map_err(|source| {
        io_diag(
            "manifest-parse",
            &path,
            source,
            "failed to read package manifest",
        )
    })?;
    let manifest = parse_manifest(&text, &path)?;
    Ok(ManifestSource {
        manifest,
        text: Some(text),
    })
}

fn parse_manifest(text: &str, path: &Path) -> Result<PackageManifest, Diagnostic> {
    let value: toml::Value = toml::from_str(text).map_err(|source| {
        let kind = if source.to_string().contains("duplicate key") {
            "duplicate-dependency-alias"
        } else {
            "manifest-parse"
        };
        package_diag(
            kind,
            format!("{} is not valid tacit.toml: {}", path.display(), source),
            None,
            None,
            Some(path),
        )
    })?;
    let Some(table) = value.as_table() else {
        return Err(package_diag(
            "manifest-parse",
            format!("{} must contain TOML tables", path.display()),
            None,
            None,
            Some(path),
        ));
    };

    for key in table.keys() {
        if !matches!(
            key.as_str(),
            "package" | "dependencies" | "exports" | "bin" | "tests"
        ) {
            return Err(package_diag(
                "manifest-unknown-field",
                format!(
                    "{} contains unknown top-level field {}",
                    path.display(),
                    key
                ),
                None,
                None,
                Some(path),
            ));
        }
    }

    Ok(PackageManifest {
        package: table
            .get("package")
            .map(|value| parse_package_metadata(value, path))
            .transpose()?,
        dependencies: table
            .get("dependencies")
            .map(|value| parse_dependencies(value, path))
            .transpose()?
            .unwrap_or_default(),
        exports: table
            .get("exports")
            .map(|value| parse_string_table(value, path, "exports"))
            .transpose()?
            .unwrap_or_default()
            .into_iter()
            .map(|(alias, hash)| {
                if let Some(hash) = parse_prefixed_hash(&hash) {
                    Ok((alias, hash))
                } else {
                    Err(package_diag(
                        "manifest-parse",
                        format!("[exports].{} must be a blake3:<hash>", alias),
                        Some(&alias),
                        None,
                        Some(path),
                    ))
                }
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?,
        bin: table
            .get("bin")
            .map(|value| parse_string_table(value, path, "bin"))
            .transpose()?
            .unwrap_or_default(),
        tests: table
            .get("tests")
            .map(|value| parse_tests(value, path))
            .transpose()?
            .unwrap_or_default(),
    })
}

fn parse_package_metadata(value: &toml::Value, path: &Path) -> Result<PackageMetadata, Diagnostic> {
    let table = expect_table(value, path, "[package]")?;
    for key in table.keys() {
        if !matches!(key.as_str(), "name" | "description" | "version") {
            return Err(package_diag(
                "manifest-unknown-field",
                format!("[package] contains unknown field {}", key),
                None,
                None,
                Some(path),
            ));
        }
    }
    Ok(PackageMetadata {
        name: optional_string(table, "name", path)?,
        description: optional_string(table, "description", path)?,
        version: optional_string(table, "version", path)?,
    })
}

fn parse_dependencies(
    value: &toml::Value,
    path: &Path,
) -> Result<BTreeMap<String, DependencySpec>, Diagnostic> {
    let table = expect_table(value, path, "[dependencies]")?;
    let mut out = BTreeMap::new();
    for (alias, value) in table {
        let dep = expect_table(value, path, &format!("[dependencies].{alias}"))?;
        for key in dep.keys() {
            if !matches!(key.as_str(), "hash" | "path" | "source") {
                return Err(package_diag(
                    "manifest-unknown-field",
                    format!("[dependencies].{} contains unknown field {}", alias, key),
                    Some(alias),
                    None,
                    Some(path),
                ));
            }
        }
        let hash = optional_string(dep, "hash", path)?;
        let local_path = optional_string(dep, "path", path)?;
        match (hash, local_path) {
            (Some(_), Some(_)) => {
                return Err(package_diag(
                    "manifest-ambiguous-source",
                    format!("[dependencies].{} declares both hash and path", alias),
                    Some(alias),
                    None,
                    Some(path),
                ));
            }
            (None, None) => {
                return Err(package_diag(
                    "manifest-missing-source",
                    format!("[dependencies].{} declares no hash or path", alias),
                    Some(alias),
                    None,
                    Some(path),
                ));
            }
            (Some(hash), None) => {
                let Some(hash) = parse_prefixed_hash(&hash) else {
                    return Err(package_diag(
                        "manifest-parse",
                        format!("[dependencies].{} hash must be blake3:<hash>", alias),
                        Some(alias),
                        None,
                        Some(path),
                    ));
                };
                let (registry, name) = parse_dependency_source(dep.get("source"), alias, path)?;
                out.insert(
                    alias.clone(),
                    DependencySpec::Hash {
                        hash,
                        registry,
                        name,
                    },
                );
            }
            (None, Some(path_value)) => {
                if dep.contains_key("source") {
                    return Err(package_diag(
                        "manifest-unknown-field",
                        format!(
                            "[dependencies].{} path dependency cannot carry source metadata",
                            alias
                        ),
                        Some(alias),
                        None,
                        Some(path),
                    ));
                }
                out.insert(alias.clone(), DependencySpec::Path { path: path_value });
            }
        }
    }
    Ok(out)
}

fn parse_dependency_source(
    value: Option<&toml::Value>,
    alias: &str,
    path: &Path,
) -> Result<(Option<String>, Option<String>), Diagnostic> {
    let Some(value) = value else {
        return Ok((None, None));
    };
    let table = expect_table(value, path, &format!("[dependencies].{alias}.source"))?;
    for key in table.keys() {
        if !matches!(key.as_str(), "registry" | "name") {
            return Err(package_diag(
                "manifest-unknown-field",
                format!(
                    "[dependencies].{}.source contains unknown field {}",
                    alias, key
                ),
                Some(alias),
                None,
                Some(path),
            ));
        }
    }
    Ok((
        optional_string(table, "registry", path)?,
        optional_string(table, "name", path)?,
    ))
}

fn parse_string_table(
    value: &toml::Value,
    path: &Path,
    table_name: &str,
) -> Result<BTreeMap<String, String>, Diagnostic> {
    let table = expect_table(value, path, &format!("[{table_name}]"))?;
    let mut out = BTreeMap::new();
    for (key, value) in table {
        let Some(value) = value.as_str() else {
            return Err(package_diag(
                "manifest-parse",
                format!("[{}].{} must be a string", table_name, key),
                Some(key),
                None,
                Some(path),
            ));
        };
        out.insert(key.clone(), value.to_string());
    }
    Ok(out)
}

fn parse_tests(value: &toml::Value, path: &Path) -> Result<Vec<PackageTest>, Diagnostic> {
    let Some(entries) = value.as_array() else {
        return Err(package_diag(
            "manifest-parse",
            format!("[[tests]] must be an array of tables in {}", path.display()),
            None,
            None,
            Some(path),
        ));
    };

    let mut out = Vec::new();
    for (index, value) in entries.iter().enumerate() {
        let table = expect_table(value, path, &format!("[[tests]] entry {index}"))?;
        for key in table.keys() {
            if !matches!(key.as_str(), "name" | "target" | "effects" | "step_budget") {
                return Err(package_diag(
                    "manifest-unknown-field",
                    format!("[[tests]] entry {} contains unknown field {}", index, key),
                    None,
                    None,
                    Some(path),
                ));
            }
        }
        let name = optional_string(table, "name", path)?.ok_or_else(|| {
            package_diag(
                "manifest-parse",
                format!("[[tests]] entry {} is missing required name", index),
                None,
                None,
                Some(path),
            )
        })?;
        let target = optional_string(table, "target", path)?.ok_or_else(|| {
            package_diag(
                "manifest-parse",
                format!("[[tests]] entry {} is missing required target", index),
                Some(&name),
                None,
                Some(path),
            )
        })?;
        let Some(target) = parse_prefixed_hash(&target) else {
            return Err(package_diag(
                "manifest-parse",
                format!("[[tests]].{} target must be a blake3:<hash>", name),
                Some(&name),
                None,
                Some(path),
            ));
        };
        let effects = table
            .get("effects")
            .map(|value| parse_test_effects(value, path, &name))
            .transpose()?
            .unwrap_or_default();
        let step_budget = table
            .get("step_budget")
            .map(|value| parse_test_step_budget(value, path, &name))
            .transpose()?;
        out.push(PackageTest {
            name,
            target,
            effects,
            step_budget,
        });
    }
    Ok(out)
}

fn parse_test_effects(value: &toml::Value, path: &Path, name: &str) -> Result<EffSet, Diagnostic> {
    let Some(entries) = value.as_array() else {
        return Err(package_diag(
            "manifest-parse",
            format!("[[tests]].{} effects must be an array of strings", name),
            Some(name),
            None,
            Some(path),
        ));
    };
    let mut effects = EffSet::empty();
    let mut previous_rank = None;
    for entry in entries {
        let Some(atom) = entry.as_str() else {
            return Err(package_diag(
                "manifest-parse",
                format!("[[tests]].{} effects entries must be strings", name),
                Some(name),
                None,
                Some(path),
            ));
        };
        let (atom, rank) = match atom {
            "Alloc" => (EffAtom::Alloc, 0),
            "Div" => (EffAtom::Div, 1),
            "IO" => (EffAtom::IO, 2),
            "Mut" => (EffAtom::Mut, 3),
            other => {
                return Err(package_diag(
                    "manifest-parse",
                    format!(
                        "[[tests]].{} effects contains {}; valid atoms are Alloc, Div, IO, Mut",
                        name, other
                    ),
                    Some(name),
                    None,
                    Some(path),
                ));
            }
        };
        if previous_rank.is_some_and(|previous| previous >= rank) {
            return Err(package_diag(
                "manifest-parse",
                format!(
                    "[[tests]].{} effects must be sorted without duplicates as Alloc, Div, IO, Mut",
                    name
                ),
                Some(name),
                None,
                Some(path),
            ));
        }
        previous_rank = Some(rank);
        effects.atoms.insert(atom);
    }
    Ok(effects)
}

fn parse_test_step_budget(value: &toml::Value, path: &Path, name: &str) -> Result<u64, Diagnostic> {
    let Some(step_budget) = value.as_integer() else {
        return Err(package_diag(
            "manifest-parse",
            format!("[[tests]].{} step_budget must be a positive integer", name),
            Some(name),
            None,
            Some(path),
        ));
    };
    let Ok(step_budget) = u64::try_from(step_budget) else {
        return Err(package_diag(
            "manifest-parse",
            format!("[[tests]].{} step_budget must be a positive integer", name),
            Some(name),
            None,
            Some(path),
        ));
    };
    if step_budget == 0 {
        return Err(package_diag(
            "manifest-parse",
            format!("[[tests]].{} step_budget must be greater than zero", name),
            Some(name),
            None,
            Some(path),
        ));
    }
    Ok(step_budget)
}

fn validate_manifest_entries(manifest: &PackageManifest, graph: &ProjectGraph) -> Vec<Diagnostic> {
    let public_exports = project_public_exports(graph);
    let mut diags = Vec::new();

    for (alias, hash) in &manifest.exports {
        if !public_exports.contains(hash) {
            diags.push(package_diag(
                "unresolved-entry",
                format!(
                    "[exports].{} does not resolve to a public export blake3:{}",
                    alias, hash
                ),
                Some(alias),
                Some(hash),
                Some(&graph.root.join(MANIFEST_FILE)),
            ));
        }
    }

    for (alias, value) in &manifest.bin {
        match resolve_bin_value(manifest, value) {
            Some(hash) if public_exports.contains(&hash) => {}
            Some(hash) => diags.push(package_diag(
                "unresolved-entry",
                format!(
                    "[bin].{} does not resolve to a public export blake3:{}",
                    alias, hash
                ),
                Some(alias),
                Some(&hash),
                Some(&graph.root.join(MANIFEST_FILE)),
            )),
            None => diags.push(package_diag(
                "unresolved-entry",
                format!(
                    "[bin].{} does not resolve through [exports] or a blake3 hash",
                    alias
                ),
                Some(alias),
                None,
                Some(&graph.root.join(MANIFEST_FILE)),
            )),
        }
    }

    let manifest_path = graph.root.join(MANIFEST_FILE);
    let mut test_names = BTreeSet::new();
    let mut test_targets = BTreeSet::new();
    for test in &manifest.tests {
        if !test_names.insert(test.name.clone()) {
            diags.push(package_diag(
                "duplicate-test-alias",
                format!("[[tests]] repeats test name {}", test.name),
                Some(&test.name),
                Some(&test.target),
                Some(&manifest_path),
            ));
        }
        if !test_targets.insert(test.target.clone()) {
            diags.push(package_diag(
                "duplicate-test-target",
                format!(
                    "[[tests]] repeats target blake3:{} for test {}",
                    test.target, test.name
                ),
                Some(&test.name),
                Some(&test.target),
                Some(&manifest_path),
            ));
        }
    }

    diags
}

fn verify_lockfile(
    path: &Path,
    expected: &Lockfile,
    required: bool,
) -> Result<Option<Lockfile>, Diagnostic> {
    if !path.exists() {
        return if required {
            Err(package_diag(
                "lockfile-drift",
                format!("{} is missing; run `tacit lock`", path.display()),
                None,
                None,
                Some(path),
            ))
        } else {
            Ok(None)
        };
    }
    let text = std::fs::read_to_string(path)
        .map_err(|source| io_diag("lockfile-parse", path, source, "failed to read lockfile"))?;
    let actual: Lockfile = serde_json::from_str(&text).map_err(|source| {
        package_diag(
            "lockfile-parse",
            format!(
                "{} is not valid tacit.lock JSON: {}",
                path.display(),
                source
            ),
            None,
            None,
            Some(path),
        )
    })?;
    actual.validate(path)?;
    let canonical = String::from_utf8(serialize_lockfile(&actual)).expect("JSON is UTF-8");
    if canonical != text {
        return Err(package_diag(
            "lockfile-parse",
            format!("{} is not in deterministic tacit.lock form", path.display()),
            None,
            None,
            Some(path),
        ));
    }
    if &actual != expected {
        return Err(package_diag(
            "lockfile-drift",
            format!(
                "{} does not match the current manifest resolution",
                path.display()
            ),
            None,
            None,
            Some(path),
        ));
    }
    Ok(Some(actual))
}

fn write_lockfile(path: &Path, lockfile: &Lockfile) -> Result<(), Diagnostic> {
    write_atomic(path, &serialize_lockfile(lockfile))
        .map_err(|source| io_diag("lockfile-parse", path, source, "failed to write lockfile"))
}

fn serialize_lockfile(lockfile: &Lockfile) -> Vec<u8> {
    with_trailing_newline(
        serde_json::to_vec_pretty(lockfile).expect("lockfile serialization is infallible"),
    )
}

fn resolve_bin_value(manifest: &PackageManifest, value: &str) -> Option<String> {
    if let Some(hash) = parse_prefixed_hash(value) {
        return Some(hash);
    }
    manifest.exports.get(value).cloned()
}

fn project_public_exports(graph: &ProjectGraph) -> BTreeSet<String> {
    graph
        .units
        .iter()
        .flat_map(|unit| unit.public_exports.iter().cloned())
        .collect()
}

fn package_hash_from_hashes<'a>(unit_hashes: impl Iterator<Item = &'a str>) -> String {
    let mut hashes: Vec<_> = unit_hashes.collect();
    hashes.sort();
    graph_hash_with_tag(PACKAGE_FORMAT, hashes.into_iter())
}

fn expect_table<'a>(
    value: &'a toml::Value,
    path: &Path,
    subject: &str,
) -> Result<&'a toml::map::Map<String, toml::Value>, Diagnostic> {
    value.as_table().ok_or_else(|| {
        package_diag(
            "manifest-parse",
            format!("{} must be a table in {}", subject, path.display()),
            None,
            None,
            Some(path),
        )
    })
}

fn optional_string(
    table: &toml::map::Map<String, toml::Value>,
    key: &str,
    path: &Path,
) -> Result<Option<String>, Diagnostic> {
    let Some(value) = table.get(key) else {
        return Ok(None);
    };
    value.as_str().map(|s| Some(s.to_string())).ok_or_else(|| {
        package_diag(
            "manifest-parse",
            format!("{} must be a string in {}", key, path.display()),
            Some(key),
            None,
            Some(path),
        )
    })
}

fn normalize_hash_selector(hash: &str) -> Option<String> {
    selector_hash(hash)
}

fn parse_prefixed_hash(hash: &str) -> Option<String> {
    let raw = hash.strip_prefix("blake3:")?;
    is_hash_str(raw).then(|| raw.to_string())
}

fn prefix_hash(hash: &str) -> String {
    format!("blake3:{hash}")
}

fn with_trailing_newline(mut bytes: Vec<u8>) -> Vec<u8> {
    if !bytes.ends_with(b"\n") {
        bytes.push(b'\n');
    }
    bytes
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("object");
    let tmp_path =
        path.with_file_name(format!(".{file_name}.{}.{}.tmp", std::process::id(), stamp));
    {
        let mut file = File::create(&tmp_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    std::fs::rename(&tmp_path, path)?;
    if let Some(parent) = path.parent() {
        if let Ok(dir) = File::open(parent) {
            let _ = dir.sync_all();
        }
    }
    Ok(())
}

fn with_alias(mut diag: Diagnostic, alias: &str) -> Diagnostic {
    let details = diag.actual.take().unwrap_or_else(|| serde_json::json!({}));
    diag.actual = Some(match details {
        serde_json::Value::Object(mut map) => {
            map.insert("alias".to_string(), serde_json::json!(alias));
            serde_json::Value::Object(map)
        }
        other => serde_json::json!({"alias": alias, "details": other}),
    });
    diag
}

fn io_diag(kind: &str, path: &Path, source: io::Error, action: &str) -> Diagnostic {
    package_diag(
        kind,
        format!("{}: {}: {}", path.display(), action, source),
        None,
        None,
        Some(path),
    )
}

fn package_diag(
    kind: &str,
    message: String,
    alias: Option<&str>,
    hash: Option<&str>,
    path: Option<&Path>,
) -> Diagnostic {
    let mut details = serde_json::Map::new();
    if let Some(alias) = alias {
        details.insert("alias".to_string(), serde_json::json!(alias));
    }
    if let Some(hash) = hash {
        details.insert("hash".to_string(), serde_json::json!(prefix_hash(hash)));
    }
    if let Some(path) = path {
        details.insert(
            "path".to_string(),
            serde_json::json!(path.to_string_lossy().into_owned()),
        );
    }
    Diagnostic::package_error(kind, message, serde_json::Value::Object(details))
}
