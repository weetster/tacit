use std::path::{Path, PathBuf};
#[cfg(feature = "llvm")]
use std::process::Command;

use clap::{Parser, Subcommand, ValueEnum};

use tacit_typecheck::{infer_module, DiagOutput};
use tacit_views::authoring::{emit_authoring, parse_authoring};
use tacit_views::sidecar::{Sidecar, SidecarNode};
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
        #[arg(long = "as", value_enum, value_name = "FORMAT", default_value = "authoring")]
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
            let node = tacit_canonical::parse(&src)
                .map_err(|e| format!("{}: {}", input.display(), e))?;
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
            let (node, display) = parse_authoring(&src)
                .map_err(|e| format!("{}: {}", input.display(), e))?;
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
    let node = tacit_canonical::parse(&src)
        .map_err(|e| format!("{}: {}", input.display(), e))?;

    let tacd_path = input.with_extension("tacd");
    let sidecar: Option<SidecarNode> = if tacd_path.exists() {
        let s = Sidecar::read(&tacd_path)
            .map_err(|e| format!("{}: {}", tacd_path.display(), e))?;
        Some(s.display)
    } else {
        None
    };

    let rendered = match view_format {
        ViewFormat::Authoring => emit_authoring(&node, sidecar.as_ref()),
        ViewFormat::Inspection => {
            let flags = InspectFlags { debruijn, hashes, types, effects };
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

fn cmd_check(input: PathBuf, format: CheckFormat) -> Result<(), Box<dyn std::error::Error>> {
    let (node, _sidecar) = load_canonical(&input)?;

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
    let (node, sidecar) = load_canonical(&input)?;

    match view_format {
        ViewFormat::Authoring => {
            print!("{}", emit_authoring(&node, sidecar.as_ref()));
        }
        ViewFormat::Inspection => {
            let flags = InspectFlags { debruijn, hashes, types, effects };
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
    emit_llvm_ir: bool,
) -> Result<(), Box<dyn std::error::Error>> {
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
