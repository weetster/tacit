//! Round-trip stability for the durable Phase 4 examples only.

use std::path::PathBuf;

use tacit_canonical::{emit, parse};
use tacit_views::authoring::{emit_authoring, parse_authoring};

fn examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/phase-4")
}

#[test]
fn phase_4_examples_round_trip() {
    for name in [
        "record-accumulator",
        "closure-pipeline",
        "stored-callback-record",
        "vector-combinators",
    ] {
        let path = examples_dir().join(format!("{name}.tac"));
        let bytes =
            std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {}", path.display(), e));
        let node = parse(&bytes).unwrap_or_else(|e| panic!("parse {}: {}", path.display(), e));
        let rendered = emit_authoring(&node, None);
        let (round_tripped, _) = parse_authoring(rendered.as_bytes())
            .unwrap_or_else(|e| panic!("parse rendered {}: {}", path.display(), e));
        assert_eq!(
            emit(&node),
            emit(&round_tripped),
            "round-trip changed canonical bytes for {}",
            path.display()
        );
    }
}
