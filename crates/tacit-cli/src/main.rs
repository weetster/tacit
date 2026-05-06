use std::path::PathBuf;
#[cfg(feature = "llvm")]
use std::process::Command;

use clap::{Parser, Subcommand, ValueEnum};

use tacit_typecheck::{infer_module, DiagOutput};
use tacit_views::authoring::{emit_authoring, parse_authoring};
use tacit_views::sidecar::Sidecar;
use tacit_views::{emit_inspection, InspectFlags};

#[derive(Parser)]
#[command(name = "tacit", about = "Tacit-Lite compiler and viewer")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Compile a .tac source file to a native executable.
    Compile {
        /// Input .tac file (authoring view).
        input: PathBuf,

        /// Write executable to FILE.
        #[arg(short, long = "output", value_name = "FILE")]
        output: Option<PathBuf>,

        /// Dump the constructed LLVM IR to stdout (the executable is still
        /// produced if -o is also given).
        #[arg(long)]
        emit_llvm_ir: bool,
    },

    /// Check types and effects without compiling.
    Check {
        /// Input .tac file (authoring view).
        input: PathBuf,

        /// Output format: human-readable text (default) or JSON.
        #[arg(long, value_enum, value_name = "FORMAT", default_value = "text")]
        format: CheckFormat,
    },

    /// Migrate a .tac (authoring view) + .tac.sidecar.toml to canonical .tac + .tacd (one-shot).
    MigrateSidecar {
        /// Input .tac file (authoring view).
        input: PathBuf,

        /// Path to the .tac.sidecar.toml to fold in (defaults to <input>.sidecar.toml).
        #[arg(long, value_name = "TOML")]
        toml: Option<PathBuf>,

        /// Parse and canonicalize but write nothing; prints canonical hash.
        #[arg(long)]
        dry_run: bool,

        /// Overwrite <input> with canonical bytes and write <input stem>.tacd alongside.
        #[arg(long)]
        in_place: bool,

        /// Reject ASTs containing Hole nodes.
        #[arg(long)]
        strict: bool,
    },

    /// Render a .tac source file in the authoring or inspection view.
    View {
        /// Input .tac file (authoring view).
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
        Cmd::Compile {
            input,
            output,
            emit_llvm_ir,
        } => cmd_compile(input, output, emit_llvm_ir),
        Cmd::Check { input, format } => cmd_check(input, format),
        Cmd::MigrateSidecar {
            input,
            toml,
            dry_run,
            in_place,
            strict,
        } => cmd_migrate_sidecar(input, toml, dry_run, in_place, strict),
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

// ---------------------------------------------------------------------------
// check subcommand
// ---------------------------------------------------------------------------

fn cmd_check(input: PathBuf, format: CheckFormat) -> Result<(), Box<dyn std::error::Error>> {
    let src = std::fs::read(&input).map_err(|e| format!("{}: {}", input.display(), e))?;
    let (node, _sidecar) =
        parse_authoring(&src).map_err(|e| format!("{}: {}", input.display(), e))?;

    match infer_module(&node) {
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

// ---------------------------------------------------------------------------
// migrate-sidecar subcommand (one-shot; deleted after repository conversion)
// ---------------------------------------------------------------------------

fn cmd_migrate_sidecar(
    input: PathBuf,
    toml_override: Option<PathBuf>,
    dry_run: bool,
    in_place: bool,
    strict: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if !dry_run && !in_place {
        return Err("specify --dry-run or --in-place".into());
    }

    // --- Parse authoring view ---
    let src = std::fs::read(&input).map_err(|e| format!("{}: {}", input.display(), e))?;
    let (node, display_sidecar) =
        parse_authoring(&src).map_err(|e| format!("{}: {}", input.display(), e))?;

    // --- Reject holes if --strict ---
    if strict && contains_hole(&node) {
        return Err(format!(
            "{}: AST contains Hole node(s); refusing with --strict",
            input.display()
        )
        .into());
    }

    // --- Emit canonical bytes ---
    let canonical_bytes = tacit_canonical::emit(&node);

    // --- Load TOML sidecar (optional) ---
    let toml_path = toml_override
        .unwrap_or_else(|| input.with_extension("tac.sidecar.toml"));
    let toml_sidecar = tacit_typecheck::TypeSidecar::load(&toml_path)
        .map_err(|e| format!("{}: {}", toml_path.display(), e))?;

    // --- Build .tacd: start from display sidecar, fold type/effect onto root node ---
    let mut root_display = display_sidecar;
    if let Some(entry) = toml_sidecar.get("main") {
        root_display.type_hint = Some(entry.type_str.clone());
        if !entry.effects.is_empty() {
            root_display.effect_hint = Some(entry.effects.clone());
        }
    }
    let tacd = Sidecar::new(&canonical_bytes, root_display);

    // --- Compute output paths ---
    let canonical_path = input.with_extension("tac");
    let tacd_path = input.with_extension("tacd");

    if dry_run {
        let hash = &tacd.targets_hash_blake3;
        println!(
            "canonical path: {}\ntacd path:      {}\nblake3 hash:    {}",
            canonical_path.display(),
            tacd_path.display(),
            hash,
        );
        return Ok(());
    }

    // in_place
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
        Node::Rec { bindings, body } => {
            bindings.iter().any(contains_hole) || contains_hole(body)
        }
        Node::Module { bindings } => bindings.iter().any(contains_hole),
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
    let src = std::fs::read(&input).map_err(|e| format!("{}: {}", input.display(), e))?;
    let (node, sidecar) =
        parse_authoring(&src).map_err(|e| format!("{}: {}", input.display(), e))?;

    match view_format {
        ViewFormat::Authoring => {
            print!("{}", emit_authoring(&node, Some(&sidecar)));
        }
        ViewFormat::Inspection => {
            let flags = InspectFlags {
                debruijn,
                hashes,
                types,
                effects,
            };
            print!("{}", emit_inspection(&node, Some(&sidecar), &flags));
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
    emit_llvm_ir: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if output.is_none() && !emit_llvm_ir {
        return Err("must specify -o <output> or --emit-llvm-ir (or both)".into());
    }

    let src = std::fs::read(&input).map_err(|e| format!("{}: {}", input.display(), e))?;
    let (node, _sidecar) =
        parse_authoring(&src).map_err(|e| format!("{}: {}", input.display(), e))?;

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

#[cfg(feature = "llvm")]
fn compile_with_llvm_node(
    node: &tacit_canonical::ast::Node,
    module_name: &str,
    output: Option<PathBuf>,
    emit_llvm_ir: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use tacit_codegen::{compile_to_ir_string, compile_to_object};

    if emit_llvm_ir {
        let ir = compile_to_ir_string(node, module_name)?;
        print!("{}", ir);
    }

    if let Some(out) = output {
        let tmp = tempfile::tempdir()?;
        let obj_path = tmp.path().join(format!("{}.o", module_name));
        compile_to_object(node, module_name, &obj_path)?;

        let linker = pick_linker()
            .ok_or("no C linker found (cc/clang/gcc); install build-essential or Xcode CLT")?;
        let status = Command::new(&linker)
            .arg(&obj_path)
            .arg("-o")
            .arg(&out)
            .status()
            .map_err(|e| format!("failed to invoke linker {}: {}", linker, e))?;
        if !status.success() {
            eprintln!("error: linker {} exited with {}", linker, status);
            std::process::exit(2);
        }
    }
    Ok(())
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
