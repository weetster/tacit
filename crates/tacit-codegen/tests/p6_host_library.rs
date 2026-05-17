#![cfg(feature = "llvm")]

use tacit_canonical::ast::Node;
use tacit_canonical::{emit, hash_node};
use tacit_codegen::compile_library_to_ir_string;
use tacit_typecheck::{load_package, package_library, HostTarget};

fn sym(name: &str) -> Node {
    Node::Sym { name: name.into() }
}

fn hash(node: &Node) -> String {
    hash_node(node)
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect()
}

fn sig(arg: &str, ret: &str, effects: &[&str]) -> Node {
    Node::Sig {
        type_: Box::new(Node::FnTy {
            arg: Box::new(sym(arg)),
            ret: Box::new(sym(ret)),
            eff: Box::new(Node::EffSet {
                atoms: effects.iter().map(|e| e.to_string()).collect(),
            }),
        }),
        eval_eff: Box::new(Node::EffSet { atoms: vec![] }),
    }
}

fn write_unit(root: &std::path::Path, unit: &Node) {
    let src = root.join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("lib.tac"), emit(unit)).unwrap();
}

#[test]
fn library_codegen_emits_export_wrapper_and_host_dispatch() {
    let dir = tempfile::tempdir().unwrap();
    let host_import = Node::HostImport {
        capability: "demo.log".into(),
        operation: "write-byte".into(),
        sig: Box::new(sig("u8", "u8", &["IO"])),
    };
    let host_hash = hash(&host_import);

    // Export: lam (call host_imp x) — type u8 -> u8 / {IO}
    let export_def = Node::Def {
        sig: Box::new(sig("u8", "u8", &["IO"])),
        body: Box::new(Node::Lam {
            body: Box::new(Node::App {
                fn_: Box::new(Node::Ref {
                    hash: host_hash.clone(),
                }),
                arg: Box::new(Node::Var { index: 0 }),
            }),
        }),
    };
    let export_hash = hash(&export_def);
    let unit = Node::Unit {
        imports: vec![host_import],
        exports: vec![Node::Export {
            visibility: "public".into(),
            hash: export_hash.clone(),
        }],
        defs: vec![export_def],
    };
    write_unit(dir.path(), &unit);

    let package = load_package(dir.path()).expect("package loads");
    let (interface, library) =
        package_library(&package, HostTarget::Native).expect("library spec builds");
    assert_eq!(interface.exports.len(), 1);
    assert_eq!(library.imports.len(), 1);

    let ir = compile_library_to_ir_string(&library, "demo_lib").expect("library IR generates");
    let prefix = &library.package_prefix;
    assert!(ir.contains(&format!("{prefix}_current_ctx")), "{ir}");
    let export_symbol = &library.exports[0].symbol;
    assert!(ir.contains(export_symbol), "{ir}");
    // wrapper returns the tacit_status i32
    assert!(ir.contains(&format!("define i32 @{export_symbol}")), "{ir}");
    // host dispatch funtion was emitted
    let callback = &library.imports[0].callback;
    assert!(ir.contains(&format!("{callback}_dispatch")), "{ir}");
}

#[test]
fn library_codegen_pure_scalar_export_has_no_host_dispatch() {
    let dir = tempfile::tempdir().unwrap();
    let def = Node::Def {
        sig: Box::new(sig("u8", "u8", &[])),
        body: Box::new(Node::Lam {
            body: Box::new(Node::App {
                fn_: Box::new(Node::App {
                    fn_: Box::new(sym("u8-add-wrap")),
                    arg: Box::new(Node::Var { index: 0 }),
                }),
                arg: Box::new(Node::Ann {
                    expr: Box::new(Node::Int { value: "1".into() }),
                    type_: Box::new(sym("u8")),
                }),
            }),
        }),
    };
    let def_hash = hash(&def);
    let unit = Node::Unit {
        imports: vec![],
        exports: vec![Node::Export {
            visibility: "public".into(),
            hash: def_hash,
        }],
        defs: vec![def],
    };
    write_unit(dir.path(), &unit);

    let package = load_package(dir.path()).expect("package loads");
    let (_interface, library) =
        package_library(&package, HostTarget::Native).expect("library spec builds");
    assert!(library.imports.is_empty());
    let ir = compile_library_to_ir_string(&library, "pure_lib").expect("library IR generates");
    assert!(ir.contains(&library.exports[0].symbol), "{ir}");
    // Returns tacit_status (i32) from the wrapper
    assert!(
        ir.contains(&format!("define i32 @{}", library.exports[0].symbol)),
        "{ir}"
    );
}
