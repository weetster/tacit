use tacit_canonical::ast::Node;
use tacit_canonical::{emit, hash_node, parse};

fn h(ch: char) -> String {
    ch.to_string().repeat(64)
}

fn sig() -> Node {
    Node::Sig {
        type_: Box::new(Node::FnTy {
            arg: Box::new(Node::Sym { name: "Int".into() }),
            ret: Box::new(Node::Sym { name: "Int".into() }),
            eff: Box::new(Node::EffSet { atoms: vec![] }),
        }),
        eval_eff: Box::new(Node::EffSet { atoms: vec![] }),
    }
}

#[test]
fn parses_and_emits_logical_unit_nodes() {
    let src = format!(
        "(unit (imports (imp \"{}\" (sig (fn-ty (sym Int) (sym Int) (eff-set)) (eff-set)))) (exports (exp public \"{}\")) (defs (def (sig (fn-ty (sym Int) (sym Int) (eff-set)) (eff-set)) (lam (ref \"{}\")))))",
        h('0'),
        h('1'),
        h('0')
    );
    let node = parse(src.as_bytes()).expect("parse unit");
    assert_eq!(String::from_utf8(emit(&node)).unwrap(), src);
}

#[test]
fn imports_exports_and_defs_emit_sorted_by_hash() {
    let def_a = Node::Def {
        sig: Box::new(sig()),
        body: Box::new(Node::Int { value: "1".into() }),
    };
    let def_b = Node::Def {
        sig: Box::new(sig()),
        body: Box::new(Node::Int { value: "2".into() }),
    };
    let mut expected_defs = [
        (hash_node(&def_a), String::from_utf8(emit(&def_a)).unwrap()),
        (hash_node(&def_b), String::from_utf8(emit(&def_b)).unwrap()),
    ];
    expected_defs.sort_by_key(|a| a.0);

    let unit = Node::Unit {
        imports: vec![
            Node::Import {
                hash: h('f'),
                sig: Box::new(sig()),
            },
            Node::Import {
                hash: h('0'),
                sig: Box::new(sig()),
            },
        ],
        exports: vec![
            Node::Export {
                visibility: "public".into(),
                hash: h('f'),
            },
            Node::Export {
                visibility: "package".into(),
                hash: h('0'),
            },
        ],
        defs: vec![def_b, def_a],
    };

    let out = String::from_utf8(emit(&unit)).unwrap();
    assert!(
        out.find(&format!("(imp \"{}\"", h('0'))).unwrap()
            < out.find(&format!("(imp \"{}\"", h('f'))).unwrap()
    );
    assert!(
        out.find(&format!("(exp package \"{}\"", h('0'))).unwrap()
            < out.find(&format!("(exp public \"{}\"", h('f'))).unwrap()
    );

    assert!(out.find(&expected_defs[0].1).unwrap() < out.find(&expected_defs[1].1).unwrap());
}

#[test]
fn rejects_bad_hash_width() {
    let src = b"(ref \"abc\")";
    assert!(parse(src).is_err());
}

#[test]
fn parses_emits_and_hash_sorts_host_imports() {
    let host = Node::HostImport {
        capability: "tacit.host.log".into(),
        operation: "write-byte".into(),
        sig: Box::new(Node::Sig {
            type_: Box::new(Node::FnTy {
                arg: Box::new(Node::Sym { name: "u8".into() }),
                ret: Box::new(Node::Sym { name: "Int".into() }),
                eff: Box::new(Node::EffSet {
                    atoms: vec!["IO".into()],
                }),
            }),
            eval_eff: Box::new(Node::EffSet { atoms: vec![] }),
        }),
    };
    let host_hash: String = hash_node(&host)
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect();
    let unit = Node::Unit {
        imports: vec![
            Node::Import {
                hash: h('f'),
                sig: Box::new(sig()),
            },
            host,
        ],
        exports: vec![],
        defs: vec![Node::Def {
            sig: Box::new(Node::Sig {
                type_: Box::new(Node::Sym { name: "Int".into() }),
                eval_eff: Box::new(Node::EffSet { atoms: vec![] }),
            }),
            body: Box::new(Node::Int { value: "0".into() }),
        }],
    };

    let emitted = emit(&unit);
    let text = String::from_utf8(emitted.clone()).unwrap();
    assert!(text.contains("(host-imp \"tacit.host.log\" \"write-byte\""));
    assert_eq!(emit(&parse(&emitted).expect("host import parses")), emitted);
    assert_eq!(host_hash.len(), 64);
}
