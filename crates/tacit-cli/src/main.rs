use std::path::PathBuf;
#[cfg(feature = "llvm")]
use std::process::Command;

use clap::{Parser, Subcommand, ValueEnum};

use tacit_views::authoring::{emit_authoring, parse_authoring};
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
        Cmd::Compile {
            input,
            output,
            emit_llvm_ir,
        } => cmd_compile(input, output, emit_llvm_ir),
        Cmd::View {
            input,
            view_format,
            debruijn,
            hashes,
        } => cmd_view(input, view_format, debruijn, hashes),
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
) -> Result<(), Box<dyn std::error::Error>> {
    let src = std::fs::read(&input).map_err(|e| format!("{}: {}", input.display(), e))?;
    let (node, sidecar) =
        parse_authoring(&src).map_err(|e| format!("{}: {}", input.display(), e))?;

    match view_format {
        ViewFormat::Authoring => {
            print!("{}", emit_authoring(&node, Some(&sidecar)));
        }
        ViewFormat::Inspection => {
            let flags = InspectFlags { debruijn, hashes };
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

    #[cfg(feature = "llvm")]
    {
        compile_with_llvm(input, output, emit_llvm_ir)
    }
    #[cfg(not(feature = "llvm"))]
    {
        let _ = (input, output, emit_llvm_ir);
        Err("tacit was not built with LLVM support (rebuild with --features llvm19-1)".into())
    }
}

#[cfg(feature = "llvm")]
fn compile_with_llvm(
    input: PathBuf,
    output: Option<PathBuf>,
    emit_llvm_ir: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use tacit_codegen::{compile_to_ir_string, compile_to_object};

    let src = std::fs::read(&input).map_err(|e| format!("{}: {}", input.display(), e))?;
    let (node, _sidecar) =
        parse_authoring(&src).map_err(|e| format!("{}: {}", input.display(), e))?;

    let module_name = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("module");

    if emit_llvm_ir {
        let ir = compile_to_ir_string(&node, module_name)?;
        print!("{}", ir);
    }

    if let Some(out) = output {
        let tmp = tempfile::tempdir()?;
        let obj_path = tmp.path().join(format!("{}.o", module_name));
        compile_to_object(&node, module_name, &obj_path)?;

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
