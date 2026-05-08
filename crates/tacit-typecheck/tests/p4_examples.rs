use std::path::PathBuf;

use tacit_canonical::parse as parse_canonical;
use tacit_typecheck::infer_module;

fn examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/phase-4")
}

#[test]
fn phase_4_examples_typecheck() {
    for name in [
        "record-accumulator",
        "closure-pipeline",
        "stored-callback-record",
        "vector-combinators",
    ] {
        let path = examples_dir().join(format!("{name}.tac"));
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
