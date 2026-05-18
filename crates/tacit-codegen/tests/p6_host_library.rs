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
    sig_node(sym(arg), sym(ret), effects)
}

fn sig_node(arg: Node, ret: Node, effects: &[&str]) -> Node {
    Node::Sig {
        type_: Box::new(Node::FnTy {
            arg: Box::new(arg),
            ret: Box::new(ret),
            eff: Box::new(Node::EffSet {
                atoms: effects.iter().map(|e| e.to_string()).collect(),
            }),
        }),
        eval_eff: Box::new(Node::EffSet { atoms: vec![] }),
    }
}

fn app(fn_: Node, arg: Node) -> Node {
    Node::App {
        fn_: Box::new(fn_),
        arg: Box::new(arg),
    }
}

fn ann_int(value: &str, ty: &str) -> Node {
    Node::Ann {
        expr: Box::new(Node::Int {
            value: value.into(),
        }),
        type_: Box::new(sym(ty)),
    }
}

fn record_ty(fields: &[(&str, Node)]) -> Node {
    Node::Record {
        fields: fields
            .iter()
            .map(|(name, ty)| ((*name).to_string(), ty.clone()))
            .collect(),
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

#[test]
fn library_codegen_accepts_record_result() {
    let dir = tempfile::tempdir().unwrap();
    let ret_ty = record_ty(&[("hi", sym("u8")), ("lo", sym("u8"))]);
    let def = Node::Def {
        sig: Box::new(sig_node(sym("Int"), ret_ty, &[])),
        body: Box::new(Node::Lam {
            body: Box::new(Node::Record {
                fields: vec![
                    ("lo".into(), ann_int("52", "u8")),
                    ("hi".into(), ann_int("18", "u8")),
                ],
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
    let (interface, library) =
        package_library(&package, HostTarget::Native).expect("library spec builds");
    assert_eq!(interface.records.len(), 1);
    let ir = compile_library_to_ir_string(&library, "record_ret").expect("library IR generates");
    assert!(ir.contains(&library.exports[0].symbol), "{ir}");
    assert!(ir.contains("store { i8, i8 }"), "{ir}");
}

#[test]
fn library_codegen_accepts_record_parameter() {
    let dir = tempfile::tempdir().unwrap();
    let arg_ty = record_ty(&[("a", sym("u8")), ("b", sym("u8"))]);
    let fn_ty = Node::FnTy {
        arg: Box::new(arg_ty.clone()),
        ret: Box::new(sym("u8")),
        eff: Box::new(Node::EffSet { atoms: vec![] }),
    };
    let body = app(
        app(
            sym("u8-add-wrap"),
            Node::Proj {
                record: Box::new(Node::Var { index: 0 }),
                field: "a".into(),
            },
        ),
        Node::Proj {
            record: Box::new(Node::Var { index: 0 }),
            field: "b".into(),
        },
    );
    let def = Node::Def {
        sig: Box::new(sig_node(arg_ty.clone(), sym("u8"), &[])),
        body: Box::new(Node::Ann {
            expr: Box::new(Node::Lam {
                body: Box::new(body),
            }),
            type_: Box::new(fn_ty),
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
    let (interface, library) =
        package_library(&package, HostTarget::Native).expect("library spec builds");
    assert_eq!(interface.records.len(), 1);
    let ir = compile_library_to_ir_string(&library, "record_param").expect("library IR generates");
    assert!(ir.contains("abi_record_in"), "{ir}");
}

#[test]
fn library_codegen_accepts_borrowed_u8vec_export_parameter() {
    let dir = tempfile::tempdir().unwrap();
    let body = app(
        app(sym("u8vec-get"), Node::Var { index: 0 }),
        Node::Int { value: "0".into() },
    );
    let def = Node::Def {
        sig: Box::new(sig("u8vec", "u8", &[])),
        body: Box::new(Node::Lam {
            body: Box::new(body),
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
    let (interface, library) =
        package_library(&package, HostTarget::Native).expect("library spec builds");
    assert_eq!(interface.exports[0].parameters[0].kind, "borrowed_vector");
    let ir = compile_library_to_ir_string(&library, "u8vec_export").expect("library IR generates");
    assert!(ir.contains("arg0_vec_invalid"), "{ir}");
    assert!(ir.contains(&library.exports[0].symbol), "{ir}");
}

#[test]
fn library_codegen_accepts_borrowed_u8vec_host_callback_parameter() {
    let dir = tempfile::tempdir().unwrap();
    let host_import = Node::HostImport {
        capability: "demo.video".into(),
        operation: "present-frame".into(),
        sig: Box::new(sig("u8vec", "u8", &["IO"])),
    };
    let host_hash = hash(&host_import);
    let export_def = Node::Def {
        sig: Box::new(sig("u8vec", "u8", &["IO"])),
        body: Box::new(Node::Lam {
            body: Box::new(app(
                Node::Ref {
                    hash: host_hash.clone(),
                },
                Node::Var { index: 0 },
            )),
        }),
    };
    let export_hash = hash(&export_def);
    let unit = Node::Unit {
        imports: vec![host_import],
        exports: vec![Node::Export {
            visibility: "public".into(),
            hash: export_hash,
        }],
        defs: vec![export_def],
    };
    write_unit(dir.path(), &unit);

    let package = load_package(dir.path()).expect("package loads");
    let (interface, library) =
        package_library(&package, HostTarget::Native).expect("library spec builds");
    assert_eq!(interface.imports[0].parameters[0].kind, "borrowed_vector");
    let ir =
        compile_library_to_ir_string(&library, "u8vec_callback").expect("library IR generates");
    assert!(
        ir.contains(&format!("{}_dispatch", library.imports[0].callback)),
        "{ir}"
    );
    assert!(ir.contains("borrowed_vec_data"), "{ir}");
}
