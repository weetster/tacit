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
fn parses_and_emits_logical_module_unit_nodes() {
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
    let mut expected_defs = vec![
        (hash_node(&def_a), String::from_utf8(emit(&def_a)).unwrap()),
        (hash_node(&def_b), String::from_utf8(emit(&def_b)).unwrap()),
    ];
    expected_defs.sort_by(|a, b| a.0.cmp(&b.0));

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
