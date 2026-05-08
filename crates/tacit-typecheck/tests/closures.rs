use std::path::PathBuf;

use tacit_canonical::parse as parse_canonical;
use tacit_typecheck::infer_module;

fn smoke_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/smoke")
}

#[test]
fn p4_closure_examples_typecheck() {
    for name in [
        "p4-closure-noncapturing-value",
        "p4-closure-capture",
        "p4-closure-return",
        "p4-closure-stored-record",
        "p4-closure-capture-function",
        "p4-closure-rec-value",
        "p4-closure-rec-partial",
        "p4-closure-pure-callback",
        "p4-closure-effectful-callback",
        "p4-combinator-map",
        "p4-combinator-fold",
        "p4-combinator-for-each-effectful",
    ] {
        let path = smoke_dir().join(format!("{name}.tac"));
        let src = std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {}", path.display(), e));
        let ast =
            parse_canonical(&src).unwrap_or_else(|e| panic!("parse {}: {:?}", path.display(), e));
        infer_module(&ast).unwrap_or_else(|diags| {
            let msgs: Vec<_> = diags
                .iter()
                .map(|d| format!("{}: {}", d.kind, d.message))
                .collect();
            panic!("typecheck {}:\n{}", path.display(), msgs.join("\n"));
        });
    }
}
