use tacit_canonical::ast::Node;
use tacit_canonical::{emit, hash_node};
use tacit_typecheck::{
    emit_c_header, emit_rust_bindings, generate_host_interface, load_package, write_host_interface,
    HostTarget,
};

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
                atoms: effects.iter().map(|effect| effect.to_string()).collect(),
            }),
        }),
        eval_eff: Box::new(Node::EffSet { atoms: vec![] }),
    }
}

fn app2(head: &str, a: Node, b: Node) -> Node {
    Node::App {
        fn_: Box::new(Node::App {
            fn_: Box::new(Node::Sym { name: head.into() }),
            arg: Box::new(a),
        }),
        arg: Box::new(b),
    }
}

fn write_unit(root: &std::path::Path, unit: &Node) {
    let src = root.join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("lib.tac"), emit(unit)).unwrap();
}

#[test]
fn interface_metadata_header_and_rust_bindings_are_deterministic() {
    let dir = tempfile::tempdir().unwrap();
    let host_import = Node::HostImport {
        capability: "tacit.host.log".into(),
        operation: "write-byte".into(),
        sig: Box::new(sig("u8", "Int", &["IO"])),
    };
    let host_hash = hash(&host_import);
    let export_def = Node::Def {
        sig: Box::new(sig("u8", "Int", &["IO"])),
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
    let interface =
        generate_host_interface(&package, HostTarget::Native).expect("interface generates");
    assert_eq!(interface.format, "tacit-interface-v1");
    assert_eq!(interface.exports.len(), 1);
    assert_eq!(interface.imports.len(), 1);
    assert_eq!(interface.exports[0].hash, format!("blake3:{export_hash}"));
    assert_eq!(interface.imports[0].hash, format!("blake3:{host_hash}"));
    assert_eq!(
        interface.exports[0].parameters[0].name.as_deref(),
        Some("u8")
    );

    let header = emit_c_header(&interface);
    assert!(header.contains("uint8_t arg0"), "{header}");
    assert!(header.contains("tacit_status (*tacit_p_"), "{header}");
    let rust = emit_rust_bindings(&interface);
    assert!(rust.contains("extern \"C\""), "{rust}");

    let (_interface, outputs) =
        write_host_interface(&package, HostTarget::Native).expect("interface writes");
    assert!(outputs.metadata_path.ends_with("interface.json"));
    assert!(outputs.metadata_path.exists());
    assert!(outputs.c_header_path.exists());
    assert!(outputs.rust_bindings_path.exists());
}

#[test]
fn wasm_target_is_rejected_for_phase_6() {
    let dir = tempfile::tempdir().unwrap();
    let def = Node::Def {
        sig: Box::new(sig("Int", "Int", &[])),
        body: Box::new(Node::Lam {
            body: Box::new(Node::Var { index: 0 }),
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
    let diags = generate_host_interface(&package, HostTarget::Wasm).expect_err("wasm rejected");
    assert!(diags
        .iter()
        .any(|diag| diag.kind == "abi-unsupported-target"));
}

#[test]
fn function_value_parameter_is_not_abi_expressible() {
    let dir = tempfile::tempdir().unwrap();
    let def = Node::Def {
        sig: Box::new(Node::Sig {
            type_: Box::new(Node::FnTy {
                arg: Box::new(Node::FnTy {
                    arg: Box::new(sym("Int")),
                    ret: Box::new(sym("Int")),
                    eff: Box::new(Node::EffSet { atoms: vec![] }),
                }),
                ret: Box::new(sym("Int")),
                eff: Box::new(Node::EffSet { atoms: vec![] }),
            }),
            eval_eff: Box::new(Node::EffSet { atoms: vec![] }),
        }),
        body: Box::new(Node::Lam {
            body: Box::new(Node::Int { value: "0".into() }),
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
    let diags =
        generate_host_interface(&package, HostTarget::Native).expect_err("function param rejected");
    assert!(diags
        .iter()
        .any(|diag| diag.kind == "abi-inexpressible-type"));
}

#[test]
fn stateful_interface_emits_instance_metadata_and_bindings() {
    let dir = tempfile::tempdir().unwrap();
    let def = Node::Def {
        sig: Box::new(sig("Int", "Int", &["Mut"])),
        body: Box::new(Node::Lam {
            body: Box::new(app2(
                "state-store",
                Node::Sym { name: "pc".into() },
                Node::Var { index: 0 },
            )),
        }),
    };
    let def_hash = hash(&def);
    let unit = Node::Unit {
        imports: vec![],
        exports: vec![Node::Export {
            visibility: "public".into(),
            hash: def_hash,
        }],
        defs: vec![
            Node::State {
                name: "Self".into(),
                type_: Box::new(Node::Record {
                    fields: vec![("pc".into(), sym("Int")), ("ram".into(), sym("u8vec"))],
                }),
            },
            def,
        ],
    };
    write_unit(dir.path(), &unit);

    let package = load_package(dir.path()).expect("package loads");
    let interface =
        generate_host_interface(&package, HostTarget::Native).expect("interface generates");
    let instance = interface.instance.as_ref().expect("instance metadata");
    assert!(instance.create_symbol.contains("_create"));
    assert!(instance.destroy_symbol.contains("_destroy"));
    assert_eq!(instance.state_fields[0].name, "pc");
    assert!(interface.exports[0].instance_method);

    let header = emit_c_header(&interface);
    assert!(
        header.contains("TACIT_STATUS_OUT_OF_MEMORY = 4"),
        "{header}"
    );
    assert!(header.contains("typedef struct tacit_p_"), "{header}");
    assert!(header.contains("_instance *instance"), "{header}");
    let rust = emit_rust_bindings(&interface);
    assert!(rust.contains("pub struct Instance<'ctx>"), "{rust}");
    assert!(rust.contains("impl Drop for Instance<'_>"), "{rust}");
}
