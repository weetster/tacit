use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
#[cfg(feature = "llvm")]
use std::process::Command;

use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use tacit_canonical::ast::Node;

use tacit_typecheck::ty::EffAtom;
use tacit_typecheck::{
    check_package, check_project, check_unit_with_sidecar, clear_package_cache,
    emit_project_inspection, evict_package_cache, infer_module, load_package, load_project,
    lock_package, materialize_package_derived, package_entry_expression,
    package_test_entry_expression, write_host_interface, CheckedUnit, DefinitionEnv, DiagOutput,
    Diagnostic, EffSet, HostTarget, PackageTest, Ty,
};
use tacit_views::authoring::{emit_authoring, parse_authoring};
use tacit_views::sidecar::{Sidecar, SidecarNode};
use tacit_views::{emit_inspection, InspectFlags};

mod pin;
mod release;

const DEFAULT_TEST_STEP_BUDGET: u64 = 100_000;
#[cfg(feature = "llvm")]
const TEST_STEP_BUDGET_EXIT_CODE: i32 = 124;

#[derive(Parser)]
#[command(
    name = "tacit",
    about = "Tacit-Lite compiler and viewer",
    version = release::TOOLCHAIN_VERSION
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Print installed Tacit toolchain version metadata.
    Version {
        /// Output format: human-readable text (default) or stable JSON.
        #[arg(long, value_enum, value_name = "FORMAT", default_value = "text")]
        format: VersionFormat,
    },

    /// Print or verify the matching Tacit-Lite primer for this toolchain.
    Primer {
        /// Output format: primer text (default) or stable JSON metadata.
        #[arg(long, value_enum, value_name = "FORMAT", default_value = "text")]
        format: PrimerFormat,

        /// Verify a file's BLAKE3 hash against the installed primer metadata.
        #[arg(
            long,
            value_name = "FILE",
            conflicts_with_all = ["list_sections", "section", "search"]
        )]
        check: Option<PathBuf>,

        /// List selectable primer section ids and headings.
        #[arg(
            long,
            conflicts_with_all = ["check", "section", "search"]
        )]
        list_sections: bool,

        /// Print one primer section by id or heading.
        #[arg(
            long,
            value_name = "ID_OR_HEADING",
            conflicts_with_all = ["check", "list_sections", "search"]
        )]
        section: Option<String>,

        /// Search primer lines and report matching sections.
        #[arg(
            long,
            value_name = "TEXT",
            conflicts_with_all = ["check", "list_sections", "section"]
        )]
        search: Option<String>,
    },

    /// Inspect or seed bundled source-level stdlib packages.
    Stdlib {
        #[command(subcommand)]
        command: StdlibCmd,
    },

    /// Create a new pinned Tacit project.
    Init {
        /// Project directory to create.
        path: PathBuf,

        /// Add bundled stdlib hash dependencies and seed them into the cache.
        #[arg(long)]
        with_stdlib: bool,

        /// Starter project template.
        #[arg(long, value_enum, value_name = "KIND", default_value = "executable")]
        template: InitTemplate,
    },

    /// Compile a .tac source file to a native executable.
    Compile {
        /// Input .tac/.taca file or project root directory.
        input: PathBuf,

        /// Write executable to FILE.
        #[arg(short, long = "output", value_name = "FILE")]
        output: Option<PathBuf>,

        /// Project entry public export, as blake3:<hash>, raw hash, or sidecar alias.
        #[arg(long, value_name = "HASH_OR_ALIAS")]
        entry: Option<String>,

        /// Dump the constructed LLVM IR to stdout (the executable is still
        /// produced if -o is also given).
        #[arg(long)]
        emit_llvm_ir: bool,
    },

    /// Check types and effects without compiling.
    Check {
        /// Input .tac/.taca file or project root directory.
        input: PathBuf,

        /// Output format: human-readable text (default) or JSON.
        #[arg(long, value_enum, value_name = "FORMAT", default_value = "text")]
        format: CheckFormat,
    },

    /// Run package-level tests declared in tacit.toml.
    Test {
        /// Package root directory.
        #[arg(default_value = ".")]
        input: PathBuf,

        /// Output format: human-readable text (default) or stable JSON.
        #[arg(long, value_enum, value_name = "FORMAT", default_value = "text")]
        format: TestFormat,
    },

    /// Generate Phase 6 constrained host-interface metadata and bindings.
    Interface {
        /// Package root directory.
        #[arg(default_value = ".")]
        input: PathBuf,

        /// Host target. Phase 6 supports native; wasm is rejected.
        #[arg(long, value_enum, value_name = "TARGET", default_value = "native")]
        target: InterfaceTarget,

        /// Emit a linkable static library under .tacit/derived/.../host/.
        /// Supports scalar boundary types, ABI records, and borrowed typed
        /// vector parameters. Owned vector returns remain outside the ABI.
        #[arg(long)]
        emit_library: bool,
    },

    /// Regenerate tacit.lock for a package root.
    Lock {
        /// Package root directory.
        #[arg(default_value = ".")]
        input: PathBuf,
    },

    /// Manage the workspace-local package cache.
    Cache {
        #[command(subcommand)]
        command: CacheCmd,
    },

    /// Parse authoring view (.taca) and emit canonical .tac + .tacd sidecar.
    Canonicalize {
        /// Input file (authoring view, typically .taca).
        input: PathBuf,

        /// Write canonical bytes to FILE.tac; .tacd is placed alongside. Defaults to <input stem>.tac.
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,

        /// Overwrite an existing .tac file.
        #[arg(long)]
        force: bool,

        /// Reject ASTs containing Hole nodes.
        #[arg(long)]
        strict: bool,
    },

    /// Render a canonical .tac file as authoring or inspection view.
    Render {
        /// Input .tac file (canonical).
        input: PathBuf,

        /// Which view to render (default: authoring).
        #[arg(
            long = "as",
            value_enum,
            value_name = "FORMAT",
            default_value = "authoring"
        )]
        view_format: ViewFormat,

        /// Write output to FILE (must end in .taca for authoring view).
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,

        /// (inspection) Annotate variable occurrences with DeBruijn indices.
        #[arg(long)]
        debruijn: bool,

        /// (inspection) Prefix each node with its 4-byte BLAKE3 hash badge.
        #[arg(long)]
        hashes: bool,

        /// (inspection) Render type annotations in human-readable form.
        #[arg(long)]
        types: bool,

        /// (inspection) Render effect annotations verbosely.
        #[arg(long)]
        effects: bool,
    },

    /// Render a .tac or .taca source file in the authoring or inspection view.
    View {
        /// Input .tac (canonical) or .taca (authoring) file.
        input: PathBuf,

        /// Which view to render: authoring or inspection.
        #[arg(long = "as", value_enum, value_name = "FORMAT")]
        view_format: ViewFormat,

        /// (inspection) Annotate variable occurrences with DeBruijn indices.
        #[arg(long)]
        debruijn: bool,

        /// (inspection) Prefix each node with its 4-byte BLAKE3 hash badge.
        #[arg(long)]
        hashes: bool,

        /// (inspection) Render type annotations in human-readable form.
        #[arg(long)]
        types: bool,

        /// (inspection) Render effect annotations verbosely.
        #[arg(long)]
        effects: bool,
    },
}

#[derive(ValueEnum, Clone)]
enum CheckFormat {
    /// Human-readable diagnostics on stderr (default).
    Text,
    /// JSON diagnostic envelope on stdout.
    Json,
}

#[derive(ValueEnum, Clone)]
enum VersionFormat {
    /// Human-readable toolchain version summary.
    Text,
    /// Stable JSON version envelope.
    Json,
}

#[derive(ValueEnum, Clone)]
enum PrimerFormat {
    /// Primer markdown bytes, or a compact check result when --check is used.
    Text,
    /// Stable JSON primer metadata, or a JSON check result when --check is used.
    Json,
}

#[derive(ValueEnum, Clone)]
enum StdlibFormat {
    /// Human-readable bundled stdlib package list.
    Text,
    /// Stable JSON bundled stdlib package index.
    Json,
}

#[derive(ValueEnum, Clone, Copy)]
enum InitTemplate {
    /// Public Int-returning executable entry.
    Executable,
    /// Public scalar function export suitable for host-library output.
    Library,
}

#[derive(ValueEnum, Clone)]
enum TestFormat {
    /// Human-readable summary on stdout (default).
    Text,
    /// Stable tacit-test-v1 result envelope on stdout.
    Json,
}

#[derive(ValueEnum, Clone)]
enum InterfaceTarget {
    Native,
    Wasm,
}

#[derive(Subcommand)]
enum CacheCmd {
    /// Remove the package cache under ROOT/.tacit/cache.
    Clear {
        /// Package root directory.
        #[arg(default_value = ".")]
        root: PathBuf,
    },

    /// Evict a package or object hash from ROOT/.tacit/cache.
    Evict {
        /// blake3:<hash> or bare 64-hex hash.
        hash: String,

        /// Package root directory.
        #[arg(default_value = ".")]
        root: PathBuf,
    },
}

#[derive(Subcommand)]
enum StdlibCmd {
    /// Print bundled stdlib package hashes, exports, and source metadata.
    List {
        /// Output format: human-readable text (default) or stable JSON.
        #[arg(long, value_enum, value_name = "FORMAT", default_value = "text")]
        format: StdlibFormat,
    },

    /// Materialize bundled stdlib package objects into ROOT/.tacit/cache.
    Seed {
        /// Project root directory.
        #[arg(long, value_name = "PROJECT", default_value = ".")]
        root: PathBuf,
    },
}

#[derive(ValueEnum, Clone)]
enum ViewFormat {
    Authoring,
    Inspection,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Cmd::Version { format } => cmd_version(format),
        Cmd::Primer {
            format,
            check,
            list_sections,
            section,
            search,
        } => cmd_primer(format, check, list_sections, section, search),
        Cmd::Stdlib { command } => cmd_stdlib(command),
        Cmd::Init {
            path,
            with_stdlib,
            template,
        } => cmd_init(path, with_stdlib, template),
        Cmd::Compile {
            input,
            output,
            entry,
            emit_llvm_ir,
        } => cmd_compile(input, output, entry, emit_llvm_ir),
        Cmd::Check { input, format } => cmd_check(input, format),
        Cmd::Test { input, format } => cmd_test(input, format),
        Cmd::Interface {
            input,
            target,
            emit_library,
        } => cmd_interface(input, target, emit_library),
        Cmd::Lock { input } => cmd_lock(input),
        Cmd::Cache { command } => cmd_cache(command),
        Cmd::Canonicalize {
            input,
            output,
            force,
            strict,
        } => cmd_canonicalize(input, output, force, strict),
        Cmd::Render {
            input,
            view_format,
            output,
            debruijn,
            hashes,
            types,
            effects,
        } => cmd_render(input, view_format, output, debruijn, hashes, types, effects),
        Cmd::View {
            input,
            view_format,
            debruijn,
            hashes,
            types,
            effects,
        } => cmd_view(input, view_format, debruijn, hashes, types, effects),
    }
}

fn cmd_version(format: VersionFormat) -> Result<(), Box<dyn std::error::Error>> {
    let envelope = release::version_envelope()?;
    match format {
        VersionFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&envelope)?);
        }
        VersionFormat::Text => {
            println!("tacit {}", envelope.toolchain_version);
            println!("release {}", envelope.release_hash);
            println!("installed manifest: {}", envelope.installed_manifest.status);
        }
    }
    Ok(())
}

const PRIMER_SEARCH_LIMIT: usize = 40;

#[derive(Clone, Serialize)]
struct PrimerSectionInfo {
    id: String,
    level: u8,
    title: String,
    line: usize,
}

#[derive(Clone)]
struct PrimerSection {
    info: PrimerSectionInfo,
    start: usize,
    end: usize,
}

#[derive(Serialize)]
struct PrimerSectionsEnvelope {
    format: &'static str,
    toolchain_version: &'static str,
    primer_hash: &'static str,
    sections: Vec<PrimerSectionInfo>,
}

#[derive(Serialize)]
struct PrimerSectionEnvelope {
    format: &'static str,
    toolchain_version: &'static str,
    primer_hash: &'static str,
    query: String,
    section: PrimerSectionInfo,
    text: String,
}

#[derive(Serialize)]
struct PrimerSearchEnvelope {
    format: &'static str,
    toolchain_version: &'static str,
    primer_hash: &'static str,
    query: String,
    total_matches: usize,
    returned_matches: usize,
    truncated: bool,
    matches: Vec<PrimerSearchMatch>,
}

#[derive(Serialize)]
struct PrimerSearchMatch {
    section: PrimerSectionInfo,
    line: usize,
    text: String,
}

fn cmd_primer(
    format: PrimerFormat,
    check: Option<PathBuf>,
    list_sections: bool,
    section: Option<String>,
    search: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(path) = check {
        let result = release::primer_check(path)?;
        match format {
            PrimerFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            PrimerFormat::Text => {
                if result.ok {
                    println!("ok {}", result.path);
                }
            }
        }
        if !result.ok {
            return Err(format!(
                "primer hash mismatch for {}: expected {}, got {}",
                result.path, result.expected_hash, result.actual_hash
            )
            .into());
        }
        return Ok(());
    }

    if list_sections {
        let text = primer_text()?;
        let sections = parse_primer_sections(text);
        match format {
            PrimerFormat::Json => {
                let envelope = PrimerSectionsEnvelope {
                    format: "tacit-primer-sections-v1",
                    toolchain_version: release::PRIMER_TOOLCHAIN_VERSION,
                    primer_hash: release::PRIMER_HASH,
                    sections: sections.into_iter().map(|section| section.info).collect(),
                };
                println!("{}", serde_json::to_string_pretty(&envelope)?);
            }
            PrimerFormat::Text => {
                for section in sections {
                    println!(
                        "{}\tline {}\t{} {}",
                        section.info.id,
                        section.info.line,
                        "#".repeat(section.info.level as usize),
                        section.info.title
                    );
                }
            }
        }
        return Ok(());
    }

    if let Some(query) = section {
        let text = primer_text()?;
        let sections = parse_primer_sections(text);
        let section = resolve_primer_section(&sections, &query)
            .map_err(|err| format!("{err}; run `tacit primer --list-sections`"))?;
        let section_text = text[section.start..section.end].to_string();
        match format {
            PrimerFormat::Json => {
                let envelope = PrimerSectionEnvelope {
                    format: "tacit-primer-section-v1",
                    toolchain_version: release::PRIMER_TOOLCHAIN_VERSION,
                    primer_hash: release::PRIMER_HASH,
                    query,
                    section: section.info.clone(),
                    text: section_text,
                };
                println!("{}", serde_json::to_string_pretty(&envelope)?);
            }
            PrimerFormat::Text => {
                use std::io::Write;
                std::io::stdout().write_all(section_text.as_bytes())?;
            }
        }
        return Ok(());
    }

    if let Some(query) = search {
        let text = primer_text()?;
        let sections = parse_primer_sections(text);
        let (matches, total_matches) = search_primer_lines(text, &sections, &query)
            .map_err(|err| format!("{err}; provide non-empty text to `--search`"))?;
        let returned_matches = matches.len();
        let truncated = total_matches > returned_matches;
        match format {
            PrimerFormat::Json => {
                let envelope = PrimerSearchEnvelope {
                    format: "tacit-primer-search-v1",
                    toolchain_version: release::PRIMER_TOOLCHAIN_VERSION,
                    primer_hash: release::PRIMER_HASH,
                    query,
                    total_matches,
                    returned_matches,
                    truncated,
                    matches,
                };
                println!("{}", serde_json::to_string_pretty(&envelope)?);
            }
            PrimerFormat::Text => {
                println!(
                    "primer search for {:?}: {} match(es), showing {}",
                    query, total_matches, returned_matches
                );
                if returned_matches > 0 {
                    println!("use `tacit primer --section <id>` to read a matching section");
                }
                for m in matches {
                    println!(
                        "{}:{} [{}] {}",
                        m.section.id, m.line, m.section.title, m.text
                    );
                }
                if truncated {
                    println!(
                        "truncated after {} matches; refine the search text",
                        PRIMER_SEARCH_LIMIT
                    );
                }
            }
        }
        return Ok(());
    }

    match format {
        PrimerFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&release::primer_metadata())?
            );
        }
        PrimerFormat::Text => {
            use std::io::Write;
            std::io::stdout().write_all(release::PRIMER_BYTES)?;
        }
    }
    Ok(())
}

fn primer_text() -> Result<&'static str, Box<dyn std::error::Error>> {
    Ok(std::str::from_utf8(release::PRIMER_BYTES)?)
}

fn parse_primer_sections(text: &str) -> Vec<PrimerSection> {
    #[derive(Clone)]
    struct Heading {
        info: PrimerSectionInfo,
        start: usize,
    }

    let mut headings = Vec::new();
    let mut used_ids: BTreeMap<String, usize> = BTreeMap::new();
    let mut offset = 0;
    for (line_idx, raw_line) in text.split_inclusive('\n').enumerate() {
        let line = raw_line.trim_end_matches('\n').trim_end_matches('\r');
        if let Some((level, title)) = primer_heading(line) {
            let base_id = slugify_primer_heading(title);
            let next = used_ids.entry(base_id.clone()).or_insert(0);
            *next += 1;
            let id = if *next == 1 {
                base_id
            } else {
                format!("{base_id}-{}", *next)
            };
            headings.push(Heading {
                info: PrimerSectionInfo {
                    id,
                    level,
                    title: title.to_string(),
                    line: line_idx + 1,
                },
                start: offset,
            });
        }
        offset += raw_line.len();
    }

    let mut sections = Vec::new();
    for (idx, heading) in headings.iter().enumerate() {
        let end = headings
            .iter()
            .skip(idx + 1)
            .find(|next| next.info.level <= heading.info.level)
            .map(|next| next.start)
            .unwrap_or(text.len());
        sections.push(PrimerSection {
            info: heading.info.clone(),
            start: heading.start,
            end,
        });
    }
    sections
}

fn primer_heading(line: &str) -> Option<(u8, &str)> {
    if let Some(title) = line.strip_prefix("### ") {
        Some((3, title.trim()))
    } else {
        line.strip_prefix("## ").map(|title| (2, title.trim()))
    }
}

fn slugify_primer_heading(title: &str) -> String {
    let mut out = String::new();
    let mut pending_dash = false;
    for ch in title.chars() {
        if ch.is_ascii_alphanumeric() {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            out.push(ch.to_ascii_lowercase());
            pending_dash = false;
        } else if !out.is_empty() {
            pending_dash = true;
        }
    }
    if out.is_empty() {
        "section".to_string()
    } else {
        out
    }
}

fn resolve_primer_section<'a>(
    sections: &'a [PrimerSection],
    query: &str,
) -> Result<&'a PrimerSection, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err("empty primer section query".to_string());
    }
    let query_lower = query.to_ascii_lowercase();
    let query_slug = slugify_primer_heading(query);

    if let Some(section) = sections.iter().find(|section| {
        section.info.id == query_lower
            || section.info.id == query_slug
            || section.info.title.eq_ignore_ascii_case(query)
    }) {
        return Ok(section);
    }

    let matches: Vec<&PrimerSection> = sections
        .iter()
        .filter(|section| {
            section.info.id.contains(&query_lower)
                || section
                    .info
                    .title
                    .to_ascii_lowercase()
                    .contains(&query_lower)
        })
        .collect();

    match matches.as_slice() {
        [section] => Ok(section),
        [] => Err(format!("no primer section matched `{query}`")),
        many => {
            let ids = many
                .iter()
                .take(8)
                .map(|section| section.info.id.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            Err(format!(
                "ambiguous primer section `{query}`; matches: {ids}"
            ))
        }
    }
}

fn search_primer_lines(
    text: &str,
    sections: &[PrimerSection],
    query: &str,
) -> Result<(Vec<PrimerSearchMatch>, usize), String> {
    let needle = query.trim();
    if needle.is_empty() {
        return Err("empty primer search query".to_string());
    }
    let needle_lower = needle.to_ascii_lowercase();
    let mut matches = Vec::new();
    let mut total = 0;
    let mut section_idx = 0;
    let mut offset = 0;

    for (line_idx, raw_line) in text.split_inclusive('\n').enumerate() {
        while section_idx + 1 < sections.len() && sections[section_idx + 1].start <= offset {
            section_idx += 1;
        }
        let line = raw_line.trim_end_matches('\n').trim_end_matches('\r');
        if line.to_ascii_lowercase().contains(&needle_lower) {
            total += 1;
            if matches.len() < PRIMER_SEARCH_LIMIT {
                let section = sections
                    .get(section_idx)
                    .filter(|section| section.start <= offset && offset < section.end)
                    .or_else(|| sections.first());
                if let Some(section) = section {
                    matches.push(PrimerSearchMatch {
                        section: section.info.clone(),
                        line: line_idx + 1,
                        text: line.to_string(),
                    });
                }
            }
        }
        offset += raw_line.len();
    }

    Ok((matches, total))
}

fn cmd_stdlib(command: StdlibCmd) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        StdlibCmd::List { format } => {
            let list = match release::stdlib_list() {
                Ok(list) => list,
                Err(diags) => {
                    eprintln!("{}", DiagOutput::new(diags).to_json_string());
                    std::process::exit(1);
                }
            };
            match format {
                StdlibFormat::Json => {
                    println!("{}", serde_json::to_string_pretty(&list)?);
                }
                StdlibFormat::Text => {
                    for package in &list.packages {
                        println!(
                            "{} {} ({})",
                            package.name, package.hash, package.source_path
                        );
                        for export in &package.public_exports {
                            println!("  {} {}", export.alias, export.hash);
                        }
                    }
                }
            }
        }
        StdlibCmd::Seed { root } => {
            let seeded = match release::seed_stdlib(root) {
                Ok(seeded) => seeded,
                Err(diags) => {
                    eprintln!("{}", DiagOutput::new(diags).to_json_string());
                    std::process::exit(1);
                }
            };
            for package in &seeded.packages {
                println!("seeded {} {}", package.name, package.hash);
            }
            println!("cache {}", seeded.cache_root);
        }
    }
    Ok(())
}

fn cmd_init(
    path: PathBuf,
    with_stdlib: bool,
    template: InitTemplate,
) -> Result<(), Box<dyn std::error::Error>> {
    if path.is_file() {
        return Err(format!("{}: file already exists", path.display()).into());
    }
    if path.is_dir() && path.read_dir()?.next().is_some() {
        return Err(format!("{}: directory is not empty", path.display()).into());
    }

    let stdlib = match release::stdlib_list() {
        Ok(list) => list,
        Err(diags) => {
            eprintln!("{}", DiagOutput::new(diags).to_json_string());
            std::process::exit(1);
        }
    };
    let version = release::version_envelope()?;
    let package_name = init_package_name(&path);
    let project = init_project_template(template);
    let root = path;
    let src_dir = root.join("src");
    std::fs::create_dir_all(&src_dir).map_err(|e| format!("{}: {}", src_dir.display(), e))?;

    let unit_bytes = tacit_canonical::emit(&project.unit);
    let unit_path = src_dir.join("main.tac");
    std::fs::write(&unit_path, &unit_bytes)
        .map_err(|e| format!("{}: {}", unit_path.display(), e))?;
    let sidecar_path = src_dir.join("main.tacd");
    project
        .sidecar(&unit_bytes)
        .write(&sidecar_path)
        .map_err(|e| format!("{}: {}", sidecar_path.display(), e))?;

    let toolchain_pin = pin::render_toolchain_pin(&version.release_hash, &stdlib);
    write_text_file(&root.join("tacit-toolchain.toml"), &toolchain_pin)?;
    write_text_file(
        &root.join("tacit.toml"),
        &render_init_manifest(&package_name, &project, with_stdlib, &stdlib),
    )?;
    let agent_doc = render_agent_doc(&package_name);
    write_text_file(&root.join("AGENTS.md"), &agent_doc)?;
    write_text_file(&root.join("CLAUDE.md"), &agent_doc)?;
    write_text_file(&root.join(".gitignore"), ".scratch/\n")?;
    let scratch_dir = root.join(".scratch");
    std::fs::create_dir_all(&scratch_dir)
        .map_err(|e| format!("{}: {}", scratch_dir.display(), e))?;

    if with_stdlib {
        if let Err(diags) = release::seed_stdlib(root.clone()) {
            eprintln!("{}", DiagOutput::new(diags).to_json_string());
            std::process::exit(1);
        }
    }

    match lock_package(&root) {
        Ok(_) => {
            println!("created {}", root.display());
            Ok(())
        }
        Err(diags) => {
            eprintln!("{}", DiagOutput::new(diags).to_json_string());
            std::process::exit(1);
        }
    }
}

struct InitProject {
    unit: Node,
    export_hash: String,
    test_hash: String,
    export_alias: &'static str,
    bin_alias: Option<&'static str>,
    unit_alias: &'static str,
}

const INIT_TEMPLATE_TEST_ALIAS: &str = "template_smoke_test";

impl InitProject {
    fn sidecar(&self, canonical_bytes: &[u8]) -> Sidecar {
        let mut definition_aliases = BTreeMap::new();
        definition_aliases.insert(self.export_hash.clone(), self.export_alias.to_string());
        definition_aliases.insert(self.test_hash.clone(), INIT_TEMPLATE_TEST_ALIAS.to_string());

        let mut export_aliases = BTreeMap::new();
        export_aliases.insert(self.export_hash.clone(), self.export_alias.to_string());

        Sidecar::new(
            canonical_bytes,
            SidecarNode {
                unit_alias: Some(self.unit_alias.to_string()),
                definition_aliases: Some(definition_aliases),
                export_aliases: Some(export_aliases),
                ..Default::default()
            },
        )
    }
}

fn init_project_template(template: InitTemplate) -> InitProject {
    match template {
        InitTemplate::Executable => executable_template(),
        InitTemplate::Library => library_template(),
    }
}

fn executable_template() -> InitProject {
    let main = Node::Def {
        sig: Box::new(int_sig()),
        body: Box::new(Node::Int { value: "0".into() }),
    };
    let main_hash = hex_hash_node(&main);
    let test = Node::Def {
        sig: Box::new(bool_sig()),
        body: Box::new(eq_ints(
            Node::Ref {
                hash: main_hash.clone(),
            },
            Node::Int { value: "0".into() },
        )),
    };
    let test_hash = hex_hash_node(&test);
    InitProject {
        unit: Node::Unit {
            imports: vec![],
            exports: vec![Node::Export {
                visibility: "public".into(),
                hash: main_hash.clone(),
            }],
            defs: vec![main, test],
        },
        export_hash: main_hash,
        test_hash,
        export_alias: "main",
        bin_alias: Some("main"),
        unit_alias: "Main",
    }
}

fn library_template() -> InitProject {
    let identity = Node::Def {
        sig: Box::new(int_to_int_sig()),
        body: Box::new(Node::Lam {
            body: Box::new(Node::Var { index: 0 }),
        }),
    };
    let identity_hash = hex_hash_node(&identity);
    let test = Node::Def {
        sig: Box::new(bool_sig()),
        body: Box::new(eq_ints(
            Node::App {
                fn_: Box::new(Node::Ref {
                    hash: identity_hash.clone(),
                }),
                arg: Box::new(Node::Int { value: "7".into() }),
            },
            Node::Int { value: "7".into() },
        )),
    };
    let test_hash = hex_hash_node(&test);
    InitProject {
        unit: Node::Unit {
            imports: vec![],
            exports: vec![Node::Export {
                visibility: "public".into(),
                hash: identity_hash.clone(),
            }],
            defs: vec![identity, test],
        },
        export_hash: identity_hash,
        test_hash,
        export_alias: "identity",
        bin_alias: None,
        unit_alias: "Library",
    }
}

fn render_init_manifest(
    package_name: &str,
    project: &InitProject,
    with_stdlib: bool,
    stdlib: &release::StdlibListEnvelope,
) -> String {
    let mut out = String::new();
    out.push_str("[package]\n");
    out.push_str(&format!("name = \"{}\"\n", toml_escape(package_name)));
    out.push_str("version = \"0.1.0\"\n\n");

    if with_stdlib {
        out.push_str("[dependencies]\n");
        for package in &stdlib.packages {
            out.push_str(&format!(
                "{} = {{ hash = \"{}\", source = {{ registry = \"builtin\", name = \"{}\" }} }}\n",
                package.short_name,
                package.hash,
                toml_escape(&package.name)
            ));
        }
        out.push('\n');
    }

    out.push_str("[exports]\n");
    out.push_str(&format!(
        "{} = \"blake3:{}\"\n\n",
        project.export_alias, project.export_hash
    ));

    if let Some(bin_alias) = project.bin_alias {
        out.push_str("[bin]\n");
        out.push_str(&format!("{bin_alias} = \"{}\"\n\n", project.export_alias));
    }

    out.push_str("[[tests]]\n");
    out.push_str(&format!("name = \"{INIT_TEMPLATE_TEST_ALIAS}\"\n"));
    out.push_str(&format!("target = \"blake3:{}\"\n", project.test_hash));
    out
}

fn render_agent_doc(package_name: &str) -> String {
    format!(
        "# {package_name} Agent Instructions\n\nThis is a Tacit project pinned by `tacit-toolchain.toml`.\n\n## Reading the language and workflow contracts\n\n- For targeted syntax refreshes, prefer selective primer commands before reading the full primer: `tacit primer --search <term>`, `tacit primer --list-sections`, then `tacit primer --section <id>`. `tacit primer --search` uses case-insensitive plain substring matching, not regex.\n- Run `tacit primer` when you need the full Tacit-Lite language primer that matches this toolchain. Do not copy primer prose from another repository or another toolchain version.\n- The agent workflow companion is installed at `share/tacit/workflow/agent-workflow.md` in the toolchain prefix. Read it before running tools.\n\n## Editing source\n\nDo not hand-edit `.tac` files. The `.tac` format is canonical S-expression bytes with BLAKE3 definition-hash references; the primer teaches the authoring view (`.taca`), which is a different surface syntax. The two do not line up token for token, and `.tac` hashes change with every edit. Edit via the round-trip loop instead:\n\n1. Render existing source as authoring view into the project's `.scratch/` directory, for example `tacit render src/main.tac --as authoring -o .scratch/{package_name}.taca`. Create `.scratch/` if it does not exist; it is excluded by `.gitignore`.\n2. Edit the scratch `.taca` using authoring-view syntax from the primer.\n3. Canonicalize back into the project: `tacit canonicalize .scratch/{package_name}.taca -o src/main.tac --force`. That rewrites both `src/main.tac` and `src/main.tacd`.\n4. Delete the scratch `.taca`. Do not check `.taca` files into this project.\n\nAfter any source edit, definition hashes change. Run `tacit lock` to refresh `tacit.lock`, then update any `[exports]`, `[bin]`, or `[[tests]]` entries in `tacit.toml` that reference the old hashes. Use `tacit view src --as inspection --hashes` (or read `tacit.lock`) to look up the new ones.\n\n## Hand-off\n\nBefore handing off changes, run `tacit lock`, `tacit check .`, and `tacit test . --format json` when LLVM support is available.\n"
    )
}

fn init_package_name(path: &Path) -> String {
    let raw = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("tacit-project");
    let mut out = String::new();
    let mut previous_dash = false;
    for ch in raw.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            ch.to_ascii_lowercase()
        } else {
            '-'
        };
        if mapped == '-' {
            if !previous_dash && !out.is_empty() {
                out.push(mapped);
            }
            previous_dash = true;
        } else {
            out.push(mapped);
            previous_dash = false;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "tacit-project".to_string()
    } else {
        out
    }
}

fn int_sig() -> Node {
    Node::Sig {
        type_: Box::new(sym("Int")),
        eval_eff: Box::new(eff(&[])),
    }
}

fn bool_sig() -> Node {
    Node::Sig {
        type_: Box::new(sym("Bool")),
        eval_eff: Box::new(eff(&[])),
    }
}

fn int_to_int_sig() -> Node {
    Node::Sig {
        type_: Box::new(Node::FnTy {
            arg: Box::new(sym("Int")),
            ret: Box::new(sym("Int")),
            eff: Box::new(eff(&[])),
        }),
        eval_eff: Box::new(eff(&[])),
    }
}

fn eq_ints(left: Node, right: Node) -> Node {
    Node::App {
        fn_: Box::new(Node::App {
            fn_: Box::new(sym("eq")),
            arg: Box::new(left),
        }),
        arg: Box::new(right),
    }
}

fn sym(name: &str) -> Node {
    Node::Sym { name: name.into() }
}

fn eff(atoms: &[&str]) -> Node {
    Node::EffSet {
        atoms: atoms.iter().map(|atom| (*atom).to_string()).collect(),
    }
}

fn hex_hash_node(node: &Node) -> String {
    tacit_canonical::hash_node(node)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn toml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn write_text_file(path: &Path, text: &str) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::write(path, text.as_bytes()).map_err(|e| format!("{}: {}", path.display(), e).into())
}

// ---------------------------------------------------------------------------
// load_canonical: shared input helper for compile / check / view
// ---------------------------------------------------------------------------

/// Load a `.tac` (canonical) or `.taca` (authoring) file into an AST.
/// For `.tac`, the paired `.tacd` sidecar is loaded if it exists alongside.
/// For `.taca`, the sidecar produced by `parse_authoring` is returned directly.
fn load_canonical(
    input: &Path,
) -> Result<(tacit_canonical::ast::Node, Option<SidecarNode>), Box<dyn std::error::Error>> {
    let src = std::fs::read(input).map_err(|e| format!("{}: {}", input.display(), e))?;
    let ext = input.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext {
        "tac" => {
            let node =
                tacit_canonical::parse(&src).map_err(|e| format!("{}: {}", input.display(), e))?;
            let tacd_path = input.with_extension("tacd");
            let sidecar = if tacd_path.exists() {
                let s = Sidecar::read(&tacd_path)
                    .map_err(|e| format!("{}: {}", tacd_path.display(), e))?;
                Some(s.display)
            } else {
                None
            };
            Ok((node, sidecar))
        }
        "taca" => {
            let (node, display) =
                parse_authoring(&src).map_err(|e| format!("{}: {}", input.display(), e))?;
            Ok((node, Some(display)))
        }
        _ => Err(format!(
            "{}: expected .tac (canonical) or .taca (authoring) input",
            input.display()
        )
        .into()),
    }
}

// ---------------------------------------------------------------------------
// canonicalize subcommand
// ---------------------------------------------------------------------------

fn cmd_canonicalize(
    input: PathBuf,
    output: Option<PathBuf>,
    force: bool,
    strict: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let src = std::fs::read(&input).map_err(|e| format!("{}: {}", input.display(), e))?;
    let (node, display_sidecar) =
        parse_authoring(&src).map_err(|e| format!("{}: {}", input.display(), e))?;

    if strict && contains_hole(&node) {
        return Err(format!(
            "{}: AST contains Hole node(s); refusing with --strict",
            input.display()
        )
        .into());
    }

    let canonical_bytes = tacit_canonical::emit(&node);

    let canonical_path = match output {
        Some(p) => p,
        None => input.with_extension("tac"),
    };
    let tacd_path = canonical_path.with_extension("tacd");

    if canonical_path.exists() && !force {
        return Err(format!(
            "{}: file exists; use --force to overwrite",
            canonical_path.display()
        )
        .into());
    }

    let tacd = Sidecar::new(&canonical_bytes, display_sidecar);
    std::fs::write(&canonical_path, &canonical_bytes)
        .map_err(|e| format!("{}: {}", canonical_path.display(), e))?;
    tacd.write(&tacd_path)
        .map_err(|e| format!("{}: {}", tacd_path.display(), e))?;

    println!(
        "wrote {} ({} bytes) and {}",
        canonical_path.display(),
        canonical_bytes.len(),
        tacd_path.display(),
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// render subcommand
// ---------------------------------------------------------------------------

fn cmd_render(
    input: PathBuf,
    view_format: ViewFormat,
    output: Option<PathBuf>,
    debruijn: bool,
    hashes: bool,
    types: bool,
    effects: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let src = std::fs::read(&input).map_err(|e| format!("{}: {}", input.display(), e))?;
    let node = tacit_canonical::parse(&src).map_err(|e| format!("{}: {}", input.display(), e))?;

    let tacd_path = input.with_extension("tacd");
    let sidecar: Option<SidecarNode> = if tacd_path.exists() {
        let s = Sidecar::read(&tacd_path).map_err(|e| format!("{}: {}", tacd_path.display(), e))?;
        Some(s.display)
    } else {
        None
    };

    let rendered = match view_format {
        ViewFormat::Authoring => emit_authoring(&node, sidecar.as_ref()),
        ViewFormat::Inspection => {
            let flags = InspectFlags {
                debruijn,
                hashes,
                types,
                effects,
            };
            emit_inspection(&node, sidecar.as_ref(), &flags)
        }
    };

    match output {
        None => print!("{}", rendered),
        Some(out) => {
            if matches!(view_format, ViewFormat::Authoring)
                && out.extension().and_then(|e| e.to_str()) != Some("taca")
            {
                return Err(format!(
                    "{}: output for authoring view must end in .taca",
                    out.display()
                )
                .into());
            }
            std::fs::write(&out, rendered.as_bytes())
                .map_err(|e| format!("{}: {}", out.display(), e))?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// check subcommand
// ---------------------------------------------------------------------------

fn enforce_project_pin(root: &Path) {
    if let Err(diags) = pin::enforce_pin(root) {
        eprintln!("{}", DiagOutput::new(diags).to_json_string());
        std::process::exit(1);
    }
}

fn cmd_check(input: PathBuf, format: CheckFormat) -> Result<(), Box<dyn std::error::Error>> {
    let result = if input.is_dir() {
        enforce_project_pin(&input);
        match load_package(&input) {
            Ok(package) => check_package(&package).map(|_| ()),
            Err(mut diags) => {
                if should_run_body_check_after_package_errors(&diags) {
                    if let Ok(graph) = load_project(&input) {
                        if let Err(mut body_diags) = check_project(&graph) {
                            diags.append(&mut body_diags);
                        }
                    }
                }
                Err(diags)
            }
        }
    } else {
        let (node, sidecar) = load_canonical(&input)?;

        if matches!(node, tacit_canonical::ast::Node::Unit { .. }) {
            check_unit_with_sidecar(&node, &DefinitionEnv::new(), sidecar.as_ref()).map(|_| ())
        } else {
            infer_module(&node).map(|_| ())
        }
    };

    match result {
        Ok(_) => {
            if let CheckFormat::Json = format {
                println!("{}", DiagOutput::new(vec![]).to_json_string());
            }
        }
        Err(diags) => {
            match format {
                CheckFormat::Json => {
                    println!("{}", DiagOutput::new(diags).to_json_string());
                }
                CheckFormat::Text => {
                    for d in &diags {
                        eprintln!("error[{}]: {}", d.kind, d.message);
                    }
                }
            }
            std::process::exit(1);
        }
    }
    Ok(())
}

fn should_run_body_check_after_package_errors(diags: &[Diagnostic]) -> bool {
    diags
        .iter()
        .any(|diag| matches!(diag.kind.as_str(), "lockfile-drift" | "unresolved-entry"))
}

// ---------------------------------------------------------------------------
// test subcommand
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct TestEnvelope {
    schema_version: &'static str,
    package: TestPackageInfo,
    outcome: String,
    summary: TestSummary,
    diagnostics: DiagOutput,
    results: Vec<TestResult>,
}

#[derive(Serialize)]
struct TestPackageInfo {
    hash: Option<String>,
    name: Option<String>,
}

#[derive(Default, Serialize)]
struct TestSummary {
    total: usize,
    pass: usize,
    fail: usize,
    compile_fail: usize,
    effect_fail: usize,
    error: usize,
}

#[derive(Serialize)]
struct TestResult {
    name: String,
    definition_hash: String,
    unit_hash: Option<String>,
    status: String,
    declared_effects: Vec<String>,
    allowed_effects: Vec<String>,
    step_budget: Option<u64>,
    observed: TestObserved,
    diagnostics: DiagOutput,
}

#[derive(Serialize)]
struct TestObserved {
    bool: Option<bool>,
}

fn cmd_test(input: PathBuf, format: TestFormat) -> Result<(), Box<dyn std::error::Error>> {
    let envelope = run_package_tests(&input);
    let exit_code = test_exit_code(&envelope);

    match format {
        TestFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&envelope)?);
        }
        TestFormat::Text => {
            print_test_text(&envelope);
            for diagnostic in &envelope.diagnostics.errors {
                eprintln!("error[{}]: {}", diagnostic.kind, diagnostic.message);
            }
        }
    }

    if exit_code != 0 {
        std::process::exit(exit_code);
    }
    Ok(())
}

fn run_package_tests(input: &Path) -> TestEnvelope {
    enforce_project_pin(input);
    let package = match load_package(input) {
        Ok(package) => package,
        Err(diags) => return test_envelope(None, None, diags, Vec::new()),
    };

    let package_hash = package.package_hash.clone();
    let package_name = package
        .manifest
        .package
        .as_ref()
        .and_then(|metadata| metadata.name.clone());
    let mut package_diags = Vec::new();

    let tests_dir = match materialize_package_derived(&package) {
        Ok(derived) => {
            let tests = derived.join("tests");
            let build = tests.join("build");
            if let Err(source) = std::fs::create_dir_all(&build) {
                package_diags.push(test_diag(
                    "test-runtime-error",
                    format!(
                        "{}: failed to create test build directory: {}",
                        build.display(),
                        source
                    ),
                    Some(&package_hash),
                    None,
                    None,
                ));
                None
            } else {
                Some(tests)
            }
        }
        Err(error) => {
            package_diags.push(test_diag(
                "test-runtime-error",
                format!("failed to materialize package test output: {}", error),
                Some(&package_hash),
                None,
                None,
            ));
            None
        }
    };

    let checked = match check_package(&package) {
        Ok(checked) => Some(checked),
        Err(diags) => {
            package_diags.extend(diags.clone());
            let results = sorted_tests(&package.manifest.tests)
                .into_iter()
                .map(|test| {
                    compile_fail_result(
                        test,
                        unit_hash_for_test(&package, test),
                        Vec::new(),
                        diags.clone(),
                    )
                })
                .collect();
            let envelope = test_envelope(Some(package_hash), package_name, package_diags, results);
            write_test_results(tests_dir.as_deref(), &envelope);
            return envelope;
        }
    };

    if tests_dir.is_none() {
        let results = sorted_tests(&package.manifest.tests)
            .into_iter()
            .map(|test| {
                error_result(
                    test,
                    unit_hash_for_test(&package, test),
                    Vec::new(),
                    vec![test_diag(
                        "test-runtime-error",
                        format!("test {} could not be launched because the derived test directory is unavailable", test.name),
                        Some(&package_hash),
                        Some(test),
                        unit_hash_for_test(&package, test).as_deref(),
                    )],
                )
            })
            .collect();
        return test_envelope(Some(package_hash), package_name, package_diags, results);
    }

    let definition_types = checked_definition_types(checked.as_ref().expect("checked is Some"));
    let definition_effects = checked_definition_effects(checked.as_ref().expect("checked is Some"));
    let build_dir = tests_dir.as_ref().expect("checked above").join("build");
    let results = sorted_tests(&package.manifest.tests)
        .into_iter()
        .map(|test| {
            run_one_package_test(
                &package,
                test,
                &definition_types,
                &definition_effects,
                &build_dir,
            )
        })
        .collect();
    let envelope = test_envelope(Some(package_hash), package_name, package_diags, results);
    write_test_results(tests_dir.as_deref(), &envelope);
    envelope
}

fn run_one_package_test(
    package: &tacit_typecheck::PackageGraph,
    test: &PackageTest,
    definition_types: &BTreeMap<String, Ty>,
    definition_effects: &BTreeMap<String, EffSet>,
    build_dir: &Path,
) -> TestResult {
    let unit_hash = unit_hash_for_test(package, test);
    let package_hash = package.package_hash.as_str();
    let declared = definition_effects
        .get(&test.target)
        .cloned()
        .unwrap_or_default();

    if !package.root.definitions.contains_key(&test.target) {
        return compile_fail_result(
            test,
            unit_hash,
            effect_strings(&declared),
            vec![test_diag(
                "test-target-unresolved",
                format!(
                    "test {} target blake3:{} is not a local package definition",
                    test.name, test.target
                ),
                Some(package_hash),
                Some(test),
                None,
            )],
        );
    }

    match definition_types.get(&test.target) {
        Some(Ty::Bool) => {}
        Some(ty) => {
            return compile_fail_result(
                test,
                unit_hash.clone(),
                effect_strings(&declared),
                vec![test_diag(
                    "test-signature-mismatch",
                    format!(
                        "test {} target blake3:{} has type {}; expected Bool",
                        test.name, test.target, ty
                    ),
                    Some(package_hash),
                    Some(test),
                    unit_hash.as_deref(),
                )],
            );
        }
        None => {
            return compile_fail_result(
                test,
                unit_hash.clone(),
                effect_strings(&declared),
                vec![test_diag(
                    "test-signature-mismatch",
                    format!(
                        "test {} target blake3:{} has no checked signature",
                        test.name, test.target
                    ),
                    Some(package_hash),
                    Some(test),
                    unit_hash.as_deref(),
                )],
            );
        }
    }

    if !declared.is_subset_of(&test.effects) {
        return effect_fail_result(
            test,
            unit_hash.clone(),
            effect_strings(&declared),
            vec![test_diag(
                "test-effect-violation",
                format!(
                    "test {} declares effects {} but manifest allows {}",
                    test.name, declared, test.effects
                ),
                Some(package_hash),
                Some(test),
                unit_hash.as_deref(),
            )],
        );
    }

    let entry = match package_test_entry_expression(package, &test.target) {
        Ok(entry) => entry,
        Err(error) => {
            return compile_fail_result(
                test,
                unit_hash.clone(),
                effect_strings(&declared),
                vec![test_diag(
                    "test-compile-failure",
                    format!("test {} failed entry lowering: {}", test.name, error),
                    Some(package_hash),
                    Some(test),
                    unit_hash.as_deref(),
                )],
            );
        }
    };

    match compile_and_run_test(
        &entry.expression,
        package_hash,
        &test.target,
        build_dir,
        resolved_step_budget(test, &declared),
    ) {
        Ok(value) => TestResult {
            name: test.name.clone(),
            definition_hash: prefixed(&test.target),
            unit_hash: unit_hash.map(|hash| prefixed(&hash)),
            status: if value { "pass" } else { "fail" }.to_string(),
            declared_effects: effect_strings(&declared),
            allowed_effects: effect_strings(&test.effects),
            step_budget: resolved_step_budget(test, &declared),
            observed: TestObserved { bool: Some(value) },
            diagnostics: DiagOutput::new(Vec::new()),
        },
        Err(diag) if diag.kind == "test-compile-failure" => {
            compile_fail_result(test, unit_hash, effect_strings(&declared), vec![*diag])
        }
        Err(diag) => error_result(test, unit_hash, effect_strings(&declared), vec![*diag]),
    }
}

#[cfg(feature = "llvm")]
fn compile_and_run_test(
    node: &tacit_canonical::ast::Node,
    package_hash: &str,
    target_hash: &str,
    build_dir: &Path,
    step_budget: Option<u64>,
) -> Result<bool, Box<Diagnostic>> {
    let module_name = format!("test_{}_{}", &package_hash[..12], &target_hash[..12]);
    let output = build_dir.join(format!("test-{}", &target_hash[..12]));
    compile_with_llvm_node_in_dir(
        node,
        &module_name,
        Some(output.clone()),
        false,
        Some(build_dir),
        step_budget,
    )
    .map_err(|error| {
        Box::new(test_diag(
            "test-compile-failure",
            format!("test blake3:{} failed to compile: {}", target_hash, error),
            Some(package_hash),
            None,
            None,
        ))
    })?;
    let status = Command::new(&output).status().map_err(|error| {
        Box::new(test_diag(
            "test-runtime-error",
            format!(
                "test blake3:{} could not be launched: {}",
                target_hash, error
            ),
            Some(package_hash),
            None,
            None,
        ))
    })?;
    match status.code() {
        Some(1) => Ok(true),
        Some(0) => Ok(false),
        Some(code) if code == TEST_STEP_BUDGET_EXIT_CODE => Err(Box::new(test_diag(
            "test-step-budget-exceeded",
            match step_budget {
                Some(step_budget) => format!(
                    "test blake3:{} exceeded step budget {}",
                    target_hash, step_budget
                ),
                None => format!("test blake3:{} exceeded its step budget", target_hash),
            },
            Some(package_hash),
            None,
            None,
        ))),
        Some(code) => Err(Box::new(test_diag(
            "test-runtime-error",
            format!(
                "test blake3:{} exited with code {}; Bool tests must return 0 or 1",
                target_hash, code
            ),
            Some(package_hash),
            None,
            None,
        ))),
        None => Err(Box::new(test_diag(
            "test-runtime-error",
            format!(
                "test blake3:{} terminated without an exit code",
                target_hash
            ),
            Some(package_hash),
            None,
            None,
        ))),
    }
}

#[cfg(not(feature = "llvm"))]
fn compile_and_run_test(
    _node: &tacit_canonical::ast::Node,
    package_hash: &str,
    target_hash: &str,
    _build_dir: &Path,
    _step_budget: Option<u64>,
) -> Result<bool, Box<Diagnostic>> {
    Err(Box::new(test_diag(
        "test-compile-failure",
        format!(
            "test blake3:{} cannot compile because tacit was not built with LLVM support",
            target_hash
        ),
        Some(package_hash),
        None,
        None,
    )))
}

fn sorted_tests(tests: &[PackageTest]) -> Vec<&PackageTest> {
    let mut tests: Vec<_> = tests.iter().collect();
    tests.sort_by(|left, right| {
        left.target
            .cmp(&right.target)
            .then_with(|| left.name.cmp(&right.name))
    });
    tests
}

fn checked_definition_types(checked: &[CheckedUnit]) -> BTreeMap<String, Ty> {
    let mut out = BTreeMap::new();
    for unit in checked {
        for (hash, ty) in &unit.definition_types {
            out.insert(hash.clone(), ty.clone());
        }
    }
    out
}

fn checked_definition_effects(checked: &[CheckedUnit]) -> BTreeMap<String, EffSet> {
    let mut out = BTreeMap::new();
    for unit in checked {
        for (hash, effects) in &unit.definition_effects {
            out.insert(hash.clone(), effects.clone());
        }
    }
    out
}

fn unit_hash_for_test(
    package: &tacit_typecheck::PackageGraph,
    test: &PackageTest,
) -> Option<String> {
    package
        .root
        .definitions
        .get(&test.target)
        .and_then(|definition| definition.unit_hashes.iter().next().cloned())
}

fn compile_fail_result(
    test: &PackageTest,
    unit_hash: Option<String>,
    declared_effects: Vec<String>,
    diags: Vec<Diagnostic>,
) -> TestResult {
    static_result(test, unit_hash, "compile-fail", declared_effects, diags)
}

fn effect_fail_result(
    test: &PackageTest,
    unit_hash: Option<String>,
    declared_effects: Vec<String>,
    diags: Vec<Diagnostic>,
) -> TestResult {
    static_result(test, unit_hash, "effect-fail", declared_effects, diags)
}

fn error_result(
    test: &PackageTest,
    unit_hash: Option<String>,
    declared_effects: Vec<String>,
    diags: Vec<Diagnostic>,
) -> TestResult {
    static_result(test, unit_hash, "error", declared_effects, diags)
}

fn static_result(
    test: &PackageTest,
    unit_hash: Option<String>,
    status: &str,
    declared_effects: Vec<String>,
    diags: Vec<Diagnostic>,
) -> TestResult {
    TestResult {
        name: test.name.clone(),
        definition_hash: prefixed(&test.target),
        unit_hash: unit_hash.map(|hash| prefixed(&hash)),
        status: status.to_string(),
        declared_effects,
        allowed_effects: effect_strings(&test.effects),
        step_budget: test.step_budget,
        observed: TestObserved { bool: None },
        diagnostics: DiagOutput::new(diags),
    }
}

fn test_envelope(
    package_hash: Option<String>,
    package_name: Option<String>,
    diagnostics: Vec<Diagnostic>,
    results: Vec<TestResult>,
) -> TestEnvelope {
    let summary = summarize_tests(&results);
    let outcome = if summary.compile_fail > 0
        || summary.effect_fail > 0
        || summary.error > 0
        || diagnostics.iter().any(|diag| diag.severity == "error")
    {
        "error"
    } else if summary.fail > 0 {
        "fail"
    } else {
        "pass"
    };
    TestEnvelope {
        schema_version: "tacit-test-v1",
        package: TestPackageInfo {
            hash: package_hash.map(|hash| prefixed(&hash)),
            name: package_name,
        },
        outcome: outcome.to_string(),
        summary,
        diagnostics: DiagOutput::new(diagnostics),
        results,
    }
}

fn summarize_tests(results: &[TestResult]) -> TestSummary {
    let mut summary = TestSummary {
        total: results.len(),
        ..Default::default()
    };
    for result in results {
        match result.status.as_str() {
            "pass" => summary.pass += 1,
            "fail" => summary.fail += 1,
            "compile-fail" => summary.compile_fail += 1,
            "effect-fail" => summary.effect_fail += 1,
            "error" => summary.error += 1,
            _ => {}
        }
    }
    summary
}

fn test_exit_code(envelope: &TestEnvelope) -> i32 {
    if envelope.summary.compile_fail > 0
        || envelope.summary.effect_fail > 0
        || envelope.summary.error > 0
        || envelope.diagnostics.has_errors()
    {
        2
    } else if envelope.summary.fail > 0 {
        1
    } else {
        0
    }
}

fn print_test_text(envelope: &TestEnvelope) {
    for result in &envelope.results {
        println!("{} {}", result.status, result.name);
    }
    println!(
        "{}: {} passed, {} failed, {} compile-fail, {} effect-fail, {} error",
        envelope.outcome,
        envelope.summary.pass,
        envelope.summary.fail,
        envelope.summary.compile_fail,
        envelope.summary.effect_fail,
        envelope.summary.error
    );
}

fn write_test_results(tests_dir: Option<&Path>, envelope: &TestEnvelope) {
    let Some(tests_dir) = tests_dir else {
        return;
    };
    if let Ok(json) = serde_json::to_vec_pretty(envelope) {
        let _ = std::fs::write(tests_dir.join("results.json"), json);
    }
}

fn cmd_interface(
    input: PathBuf,
    target: InterfaceTarget,
    emit_library: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let target = match target {
        InterfaceTarget::Native => HostTarget::Native,
        InterfaceTarget::Wasm => HostTarget::Wasm,
    };
    enforce_project_pin(&input);
    let package = match load_package(&input) {
        Ok(package) => package,
        Err(diags) => {
            eprintln!("{}", DiagOutput::new(diags).to_json_string());
            std::process::exit(1);
        }
    };
    let outputs = match write_host_interface(&package, target) {
        Ok((_interface, outputs)) => outputs,
        Err(diags) => {
            eprintln!("{}", DiagOutput::new(diags).to_json_string());
            std::process::exit(1);
        }
    };
    println!("{}", outputs.metadata_path.display());
    println!("{}", outputs.c_header_path.display());
    println!("{}", outputs.rust_bindings_path.display());

    if emit_library {
        let library = match tacit_typecheck::package_library(&package, target) {
            Ok((_, library)) => library,
            Err(diags) => {
                eprintln!("{}", DiagOutput::new(diags).to_json_string());
                std::process::exit(1);
            }
        };
        let lib_path = emit_static_library(&library, &outputs.c_header_path)?;
        println!("{}", lib_path.display());
    }
    Ok(())
}

#[cfg(feature = "llvm")]
fn emit_static_library(
    library: &tacit_typecheck::PackageLibrary,
    header_path: &Path,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    use tacit_codegen::compile_library_to_object;

    let derived_dir = header_path
        .parent()
        .ok_or("interface header has no parent directory")?;
    let build_dir = derived_dir.join("build");
    std::fs::create_dir_all(&build_dir).map_err(|e| format!("{}: {}", build_dir.display(), e))?;
    let module_name = format!("tacit_lib_{}", &library.package_hash[..12]);
    let obj_path = build_dir.join(format!("{module_name}.o"));
    compile_library_to_object(library, &module_name, &obj_path)?;

    let archive_name = format!("lib{}.a", library.package_prefix);
    let archive_path = derived_dir.join(&archive_name);
    let ar_tool = pick_archiver()
        .ok_or("no archiver found (ar/llvm-ar); install binutils to assemble static libraries")?;
    let _ = std::fs::remove_file(&archive_path);
    let status = Command::new(&ar_tool)
        .arg("rcs")
        .arg(&archive_path)
        .arg(&obj_path)
        .status()
        .map_err(|e| format!("failed to invoke archiver {}: {}", ar_tool, e))?;
    if !status.success() {
        return Err(format!("archiver {} exited with {}", ar_tool, status).into());
    }
    Ok(archive_path)
}

#[cfg(not(feature = "llvm"))]
fn emit_static_library(
    _library: &tacit_typecheck::PackageLibrary,
    _header_path: &Path,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Err("tacit was not built with LLVM support (rebuild with --features llvm19-1)".into())
}

#[cfg(feature = "llvm")]
fn pick_archiver() -> Option<String> {
    for cand in ["ar", "llvm-ar"] {
        if cmd_on_path(cand) {
            return Some(cand.to_string());
        }
    }
    None
}

fn effect_strings(effects: &EffSet) -> Vec<String> {
    effects.atoms.iter().map(ToString::to_string).collect()
}

fn test_diag(
    kind: &str,
    message: String,
    package_hash: Option<&str>,
    test: Option<&PackageTest>,
    unit_hash: Option<&str>,
) -> Diagnostic {
    let mut details = serde_json::Map::new();
    if let Some(package_hash) = package_hash {
        details.insert(
            "package_hash".to_string(),
            serde_json::json!(prefixed(package_hash)),
        );
    }
    if let Some(test) = test {
        details.insert("test".to_string(), serde_json::json!(test.name));
        details.insert(
            "definition_hash".to_string(),
            serde_json::json!(prefixed(&test.target)),
        );
    }
    if let Some(unit_hash) = unit_hash {
        details.insert(
            "unit_hash".to_string(),
            serde_json::json!(prefixed(unit_hash)),
        );
    }
    Diagnostic::package_error(kind, message, serde_json::Value::Object(details))
}

fn prefixed(hash: &str) -> String {
    if hash.starts_with("blake3:") {
        hash.to_string()
    } else {
        format!("blake3:{hash}")
    }
}

fn cmd_lock(input: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    enforce_project_pin(&input);
    match lock_package(&input) {
        Ok(package) => {
            println!("wrote {}", package.root.root.join("tacit.lock").display());
            Ok(())
        }
        Err(diags) => {
            eprintln!("{}", DiagOutput::new(diags).to_json_string());
            std::process::exit(1);
        }
    }
}

fn cmd_cache(command: CacheCmd) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        CacheCmd::Clear { root } => clear_package_cache(root)?,
        CacheCmd::Evict { hash, root } => evict_package_cache(root, &hash)?,
    }
    Ok(())
}

fn contains_hole(node: &tacit_canonical::ast::Node) -> bool {
    use tacit_canonical::ast::Node;
    match node {
        Node::Hole { .. } => true,
        Node::Int { .. }
        | Node::Str { .. }
        | Node::Sym { .. }
        | Node::Var { .. }
        | Node::PatWild
        | Node::PatVar
        | Node::PatInt { .. }
        | Node::TyVar { .. }
        | Node::EffSet { .. }
        | Node::EffVar { .. } => false,
        Node::Lam { body } => contains_hole(body),
        Node::App { fn_, arg } => contains_hole(fn_) || contains_hole(arg),
        Node::Let { rhs, body } => contains_hole(rhs) || contains_hole(body),
        Node::If { cond, then, else_ } => {
            contains_hole(cond) || contains_hole(then) || contains_hole(else_)
        }
        Node::Rec { bindings, body } => bindings.iter().any(contains_hole) || contains_hole(body),
        Node::Module { bindings } => bindings.iter().any(contains_hole),
        Node::Unit {
            imports,
            exports,
            defs,
        } => {
            imports.iter().any(contains_hole)
                || exports.iter().any(contains_hole)
                || defs.iter().any(contains_hole)
        }
        Node::Imports { entries } => entries.iter().any(contains_hole),
        Node::Import { sig, .. } => contains_hole(sig),
        Node::HostImport { sig, .. } => contains_hole(sig),
        Node::Exports { entries } => entries.iter().any(contains_hole),
        Node::Export { .. } => false,
        Node::Defs { defs } => defs.iter().any(contains_hole),
        Node::Def { sig, body } => contains_hole(sig) || contains_hole(body),
        Node::Sig { type_, eval_eff } => contains_hole(type_) || contains_hole(eval_eff),
        Node::Ref { .. } => false,
        Node::Record { fields } => fields.iter().any(|(_, v)| contains_hole(v)),
        Node::Proj { record, .. } => contains_hole(record),
        Node::Match { scrutinee, arms } => {
            contains_hole(scrutinee) || arms.iter().any(contains_hole)
        }
        Node::Arm { pattern, body } => contains_hole(pattern) || contains_hole(body),
        Node::Ann { expr, type_ } => contains_hole(expr) || contains_hole(type_),
        Node::Ctor { args, .. } => args.iter().any(contains_hole),
        Node::PatCtor { sub_patterns, .. } => sub_patterns.iter().any(contains_hole),
        Node::FnTy { arg, ret, eff } => {
            contains_hole(arg) || contains_hole(ret) || contains_hole(eff)
        }
        Node::Forall { body, .. } => contains_hole(body),
        Node::State { type_, .. } => contains_hole(type_),
    }
}

// ---------------------------------------------------------------------------
// view subcommand
// ---------------------------------------------------------------------------

fn cmd_view(
    input: PathBuf,
    view_format: ViewFormat,
    debruijn: bool,
    hashes: bool,
    types: bool,
    effects: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if input.is_dir() {
        let graph = load_project(&input)?;
        return match view_format {
            ViewFormat::Inspection => {
                let flags = InspectFlags {
                    debruijn,
                    hashes,
                    types,
                    effects,
                };
                print!("{}", emit_project_inspection(&graph, &flags));
                Ok(())
            }
            ViewFormat::Authoring => {
                Err("project authoring view is not supported; use --as inspection".into())
            }
        };
    }

    let (node, sidecar) = load_canonical(&input)?;

    match view_format {
        ViewFormat::Authoring => {
            print!("{}", emit_authoring(&node, sidecar.as_ref()));
        }
        ViewFormat::Inspection => {
            let flags = InspectFlags {
                debruijn,
                hashes,
                types,
                effects,
            };
            print!("{}", emit_inspection(&node, sidecar.as_ref(), &flags));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// compile subcommand
// ---------------------------------------------------------------------------

fn cmd_compile(
    input: PathBuf,
    output: Option<PathBuf>,
    entry: Option<String>,
    emit_llvm_ir: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if input.is_dir() {
        return cmd_compile_project(input, output, entry, emit_llvm_ir);
    }

    if entry.is_some() {
        return Err("--entry is only valid when compiling a project directory".into());
    }

    if output.is_none() && !emit_llvm_ir {
        return Err("must specify -o <output> or --emit-llvm-ir (or both)".into());
    }

    let (node, _sidecar) = load_canonical(&input)?;

    let module_name = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("module")
        .to_string();

    // Typecheck ahead of codegen — no LLVM dep; type/effect errors exit 1.
    if let Err(diags) = infer_module(&node) {
        let out = DiagOutput::new(diags);
        eprintln!("{}", out.to_json_string());
        std::process::exit(1);
    }

    #[cfg(feature = "llvm")]
    {
        compile_with_llvm_node(&node, &module_name, output, emit_llvm_ir)
    }
    #[cfg(not(feature = "llvm"))]
    {
        let _ = (node, module_name, output, emit_llvm_ir);
        Err("tacit was not built with LLVM support (rebuild with --features llvm19-1)".into())
    }
}

fn cmd_compile_project(
    input: PathBuf,
    output: Option<PathBuf>,
    entry: Option<String>,
    emit_llvm_ir: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    enforce_project_pin(&input);
    let package = match load_package(&input) {
        Ok(package) => package,
        Err(diags) => {
            eprintln!("{}", DiagOutput::new(diags).to_json_string());
            std::process::exit(1);
        }
    };
    if let Err(diags) = check_package(&package) {
        let out = DiagOutput::new(diags);
        eprintln!("{}", out.to_json_string());
        std::process::exit(1);
    }

    let derived_root = materialize_package_derived(&package)?;
    let entry = package_entry_expression(&package, entry.as_deref())?;
    let module_name = format!("project_{}", &package.package_hash[..12]);
    let output = output.or_else(|| {
        (!emit_llvm_ir).then(|| {
            derived_root
                .join("bin")
                .join(format!("entry-{}", &entry.hash[..12]))
        })
    });

    #[cfg(feature = "llvm")]
    {
        compile_with_llvm_node_in_dir(
            &entry.expression,
            &module_name,
            output,
            emit_llvm_ir,
            Some(&derived_root.join("build")),
            None,
        )
    }
    #[cfg(not(feature = "llvm"))]
    {
        let _ = (entry, module_name, output, emit_llvm_ir, derived_root);
        Err("tacit was not built with LLVM support (rebuild with --features llvm19-1)".into())
    }
}

#[cfg(feature = "llvm")]
fn compile_with_llvm_node(
    node: &tacit_canonical::ast::Node,
    module_name: &str,
    output: Option<PathBuf>,
    emit_llvm_ir: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    compile_with_llvm_node_in_dir(node, module_name, output, emit_llvm_ir, None, None)
}

#[cfg(feature = "llvm")]
fn compile_with_llvm_node_in_dir(
    node: &tacit_canonical::ast::Node,
    module_name: &str,
    output: Option<PathBuf>,
    emit_llvm_ir: bool,
    object_dir: Option<&Path>,
    step_budget: Option<u64>,
) -> Result<(), Box<dyn std::error::Error>> {
    use tacit_codegen::{
        compile_to_ir_string, compile_to_object, compile_to_object_with_options, CompileOptions,
    };

    if emit_llvm_ir {
        let ir = compile_to_ir_string(node, module_name)?;
        print!("{}", ir);
    }

    if let Some(out) = output {
        let _tmp;
        let obj_path = if let Some(object_dir) = object_dir {
            std::fs::create_dir_all(object_dir)
                .map_err(|e| format!("{}: {}", object_dir.display(), e))?;
            object_dir.join(format!("{}.o", module_name))
        } else {
            _tmp = tempfile::tempdir()?;
            _tmp.path().join(format!("{}.o", module_name))
        };
        if step_budget.is_some() {
            compile_to_object_with_options(
                node,
                module_name,
                &obj_path,
                CompileOptions {
                    loop_step_budget: step_budget,
                },
            )?;
        } else {
            compile_to_object(node, module_name, &obj_path)?;
        }

        let linker = pick_linker()
            .ok_or("no C linker found (cc/clang/gcc); install build-essential or Xcode CLT")?;
        let status = Command::new(&linker)
            .arg(&obj_path)
            .arg("-o")
            .arg(&out)
            .status()
            .map_err(|e| format!("failed to invoke linker {}: {}", linker, e))?;
        if !status.success() {
            return Err(format!("linker {} exited with {}", linker, status).into());
        }
    }
    Ok(())
}

fn resolved_step_budget(test: &PackageTest, declared: &EffSet) -> Option<u64> {
    if declared.atoms.contains(&EffAtom::Div) {
        Some(test.step_budget.unwrap_or(DEFAULT_TEST_STEP_BUDGET))
    } else {
        test.step_budget
    }
}

#[cfg(feature = "llvm")]
fn pick_linker() -> Option<String> {
    for cand in ["cc", "clang", "gcc"] {
        if cmd_on_path(cand) {
            return Some(cand.to_string());
        }
    }
    None
}

#[cfg(feature = "llvm")]
fn cmd_on_path(cmd: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {} >/dev/null 2>&1", cmd))
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
