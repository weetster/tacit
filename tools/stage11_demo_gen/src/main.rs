//! Phase 6 Stage 11 embedding-demo kernel generator.
//!
//! Re-run with `cargo run -p stage11-demo-gen` after editing the kernel
//! definitions in this file. The generator constructs the unit AST, computes
//! definition hashes, writes the canonical text + sidecar + tacit.toml to
//! `examples/phase-6/embedding-demo/kernel/`, and prints the hash list for
//! cross-reference.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use tacit_canonical::ast::Node;
use tacit_canonical::{emit, hash_node};

fn main() {
    let kernel_dir = repo_root().join("examples/phase-6/embedding-demo/kernel");
    let src_dir = kernel_dir.join("src");
    std::fs::create_dir_all(&src_dir).expect("create src dir");

    // ---- host import -----------------------------------------------------
    let log_byte_import = host_import("demo.log", "write-byte", &["Int"], "Int", &["IO"]);
    let log_byte_hash = hex_hash(&log_byte_import);

    // ---- public exports --------------------------------------------------
    let decode_op = def_pure_fn(
        &["Int"],
        "Int",
        // body: mod byte 16
        app(app(sym("mod"), var(0)), int(16)),
    );
    let decode_op_hash = hex_hash(&decode_op);

    let step_cpu = def_pure_fn(
        &["Int", "Int", "Int"],
        "Int",
        // lam acc => lam op => lam operand => match (ann op Int) of arms
        // Inside innermost body: var 0 = operand, var 1 = op, var 2 = acc.
        // The Ann constrains var 1 to Int at the use site so pat-int can
        // typecheck — bottom-up inference would otherwise leave var 1 as a
        // fresh metavariable when match runs its pattern check.
        Node::Match {
            scrutinee: Box::new(Node::Ann {
                expr: Box::new(var(1)),
                type_: Box::new(sym("Int")),
            }),
            arms: vec![
                arm_int("0", var(2)),                                             // NOP
                arm_int("1", mod_(add(var(2), var(0)), int(256))),                // ADD-mod-256
                arm_int("2", mod_(add(sub(var(2), var(0)), int(256)), int(256))), // SUB-mod-256
                arm_int("3", mod_(mul(var(2), var(0)), int(256))),                // MUL-mod-256
                arm_int("8", var(0)),                                             // LOAD operand
                arm_int("9", int(0)),                                             // ZERO
                arm_wild(var(2)), // unknown op = NOP
            ],
        },
    );
    let step_cpu_hash = hex_hash(&step_cpu);

    let log_acc = def_eff_fn(
        &["Int"],
        "Int",
        &["IO"],
        // body: log-byte acc — host import call
        app(ref_(&log_byte_hash), var(0)),
    );
    let log_acc_hash = hex_hash(&log_acc);

    // ---- test definitions (private, Bool, pure) --------------------------
    let test_decode_op = def_bool(
        // eq (decode-op 171) 11  -> 0xAB & 0x0F == 11
        app(
            app(sym("eq"), app(ref_(&decode_op_hash), int(171))),
            int(11),
        ),
    );
    let test_decode_op_hash = hex_hash(&test_decode_op);

    let test_step_add = def_bool(
        // eq (step-cpu 5 1 3) 8
        app(
            app(
                sym("eq"),
                app(app(app(ref_(&step_cpu_hash), int(5)), int(1)), int(3)),
            ),
            int(8),
        ),
    );
    let test_step_add_hash = hex_hash(&test_step_add);

    let test_step_load = def_bool(
        // eq (step-cpu 99 8 42) 42
        app(
            app(
                sym("eq"),
                app(app(app(ref_(&step_cpu_hash), int(99)), int(8)), int(42)),
            ),
            int(42),
        ),
    );
    let test_step_load_hash = hex_hash(&test_step_load);

    let test_step_nop = def_bool(
        // eq (step-cpu 7 0 99) 7
        app(
            app(
                sym("eq"),
                app(app(app(ref_(&step_cpu_hash), int(7)), int(0)), int(99)),
            ),
            int(7),
        ),
    );
    let test_step_nop_hash = hex_hash(&test_step_nop);

    // ---- assemble unit ---------------------------------------------------
    // defs go in the unit's defs vector, exports declare the public hashes.
    let defs = vec![
        decode_op,
        step_cpu,
        log_acc,
        test_decode_op,
        test_step_add,
        test_step_load,
        test_step_nop,
    ];

    let exports = vec![
        export_public(&decode_op_hash),
        export_public(&step_cpu_hash),
        export_public(&log_acc_hash),
    ];

    let unit = Node::Unit {
        imports: vec![log_byte_import],
        exports,
        defs,
    };

    // ---- write files -----------------------------------------------------
    let unit_bytes = emit(&unit);
    let lib_path = src_dir.join("lib.tac");
    std::fs::write(&lib_path, &unit_bytes).expect("write lib.tac");

    // tacit.toml with package metadata, export aliases, and tests.
    let mut export_aliases: BTreeMap<&str, &String> = BTreeMap::new();
    export_aliases.insert("decode-op", &decode_op_hash);
    export_aliases.insert("step-cpu", &step_cpu_hash);
    export_aliases.insert("log-acc", &log_acc_hash);

    let toml = build_tacit_toml(
        "tacit-embedding-demo-kernel",
        &export_aliases,
        &[
            ("test_decode_op_low_nibble", &test_decode_op_hash, &[]),
            ("test_step_add", &test_step_add_hash, &[]),
            ("test_step_load", &test_step_load_hash, &[]),
            ("test_step_nop", &test_step_nop_hash, &[]),
        ],
    );
    let toml_path = kernel_dir.join("tacit.toml");
    std::fs::write(&toml_path, &toml).expect("write tacit.toml");

    // Status report
    println!("wrote {} ({} bytes)", lib_path.display(), unit_bytes.len());
    println!("wrote {}", toml_path.display());
    println!();
    println!("hashes:");
    println!("  host.log-byte     blake3:{log_byte_hash}");
    println!("  decode-op         blake3:{decode_op_hash}");
    println!("  step-cpu          blake3:{step_cpu_hash}");
    println!("  log-acc           blake3:{log_acc_hash}");
    println!("  test_decode_op_*  blake3:{test_decode_op_hash}");
    println!("  test_step_add     blake3:{test_step_add_hash}");
    println!("  test_step_load    blake3:{test_step_load_hash}");
    println!("  test_step_nop     blake3:{test_step_nop_hash}");
}

// ── Node helpers ────────────────────────────────────────────────────────────

fn sym(name: &str) -> Node {
    Node::Sym { name: name.into() }
}

fn var(index: u64) -> Node {
    Node::Var { index }
}

fn int(value: i64) -> Node {
    Node::Int {
        value: value.to_string(),
    }
}

fn app(fn_: Node, arg: Node) -> Node {
    Node::App {
        fn_: Box::new(fn_),
        arg: Box::new(arg),
    }
}

fn ref_(hash: &str) -> Node {
    Node::Ref {
        hash: hash.to_string(),
    }
}

fn add(a: Node, b: Node) -> Node {
    app(app(sym("add"), a), b)
}

fn sub(a: Node, b: Node) -> Node {
    app(app(sym("sub"), a), b)
}

fn mod_(a: Node, b: Node) -> Node {
    app(app(sym("mod"), a), b)
}

fn mul(a: Node, b: Node) -> Node {
    app(app(sym("mul"), a), b)
}

fn arm_int(value: &str, body: Node) -> Node {
    Node::Arm {
        pattern: Box::new(Node::PatInt {
            value: value.into(),
        }),
        body: Box::new(body),
    }
}

fn arm_wild(body: Node) -> Node {
    Node::Arm {
        pattern: Box::new(Node::PatWild),
        body: Box::new(body),
    }
}

fn fn_ty_chain(params: &[&str], result: &str, effects: &[&str]) -> Node {
    let mut ty = sym(result);
    for (i, param) in params.iter().enumerate().rev() {
        let eff_atoms: Vec<String> = if i == 0 {
            effects.iter().map(|e| e.to_string()).collect()
        } else {
            Vec::new()
        };
        ty = Node::FnTy {
            arg: Box::new(sym(param)),
            ret: Box::new(ty),
            eff: Box::new(Node::EffSet { atoms: eff_atoms }),
        };
    }
    ty
}

fn sig(ty: Node) -> Node {
    Node::Sig {
        type_: Box::new(ty),
        eval_eff: Box::new(Node::EffSet { atoms: vec![] }),
    }
}

fn def_pure_fn(params: &[&str], result: &str, body_after_lams: Node) -> Node {
    let lam_body = wrap_lams(params.len(), body_after_lams);
    Node::Def {
        sig: Box::new(sig(fn_ty_chain(params, result, &[]))),
        body: Box::new(lam_body),
    }
}

fn def_eff_fn(params: &[&str], result: &str, effects: &[&str], body_after_lams: Node) -> Node {
    let lam_body = wrap_lams(params.len(), body_after_lams);
    Node::Def {
        sig: Box::new(sig(fn_ty_chain(params, result, effects))),
        body: Box::new(lam_body),
    }
}

fn def_bool(body: Node) -> Node {
    Node::Def {
        sig: Box::new(sig(sym("Bool"))),
        body: Box::new(body),
    }
}

fn wrap_lams(arity: usize, body: Node) -> Node {
    let mut acc = body;
    for _ in 0..arity {
        acc = Node::Lam {
            body: Box::new(acc),
        };
    }
    acc
}

fn host_import(
    capability: &str,
    operation: &str,
    params: &[&str],
    result: &str,
    effects: &[&str],
) -> Node {
    Node::HostImport {
        capability: capability.into(),
        operation: operation.into(),
        sig: Box::new(sig(fn_ty_chain(params, result, effects))),
    }
}

fn export_public(hash: &str) -> Node {
    Node::Export {
        visibility: "public".into(),
        hash: hash.to_string(),
    }
}

fn hex_hash(node: &Node) -> String {
    hash_node(node)
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect()
}

fn build_tacit_toml(
    name: &str,
    exports: &BTreeMap<&str, &String>,
    tests: &[(&str, &String, &[&str])],
) -> String {
    let mut out = String::new();
    out.push_str("[package]\n");
    out.push_str(&format!("name = \"{name}\"\n"));
    out.push_str("version = \"0.0.0\"\n");
    out.push('\n');
    out.push_str("[exports]\n");
    for (alias, hash) in exports {
        out.push_str(&format!("{alias} = \"blake3:{hash}\"\n"));
    }
    for (test_name, hash, effects) in tests {
        out.push('\n');
        out.push_str("[[tests]]\n");
        out.push_str(&format!("name = \"{test_name}\"\n"));
        out.push_str(&format!("target = \"blake3:{hash}\"\n"));
        if !effects.is_empty() {
            let atoms: Vec<String> = effects.iter().map(|e| format!("\"{e}\"")).collect();
            out.push_str(&format!("effects = [{}]\n", atoms.join(", ")));
        }
    }
    out
}

fn repo_root() -> PathBuf {
    // Cargo runs the binary from the workspace root by default. Resolve up
    // from CARGO_MANIFEST_DIR.
    let manifest =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR set when run via cargo");
    Path::new(&manifest)
        .parent()
        .and_then(|p| p.parent())
        .expect("manifest two levels deep")
        .to_path_buf()
}
