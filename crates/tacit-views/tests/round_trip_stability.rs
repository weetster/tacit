//! Round-trip stability gate: for every canonical `.tac` file under
//! `examples/` and `corpus/`, verify that:
//!   canonical bytes → parse → emit-authoring → parse-authoring → emit
//! produces byte-identical canonical output.
//!
//! A failure here is a tooling bug: either the authoring emitter loses
//! structural information or the authoring parser introduces a difference.

use std::fs;
use std::path::PathBuf;

use tacit_canonical::{emit, parse};
use tacit_views::authoring::{emit_authoring, parse_authoring};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn collect_tac_files(dir: &PathBuf, out: &mut Vec<PathBuf>) {
    if dir.ends_with("corpus/sealed") {
        return;
    }

    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_tac_files(&path, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some("tac") {
            out.push(path);
        }
    }
}

#[test]
fn round_trip_stability_examples_and_corpus() {
    let root = workspace_root();
    let mut files: Vec<PathBuf> = Vec::new();
    for dir_name in &["examples", "corpus"] {
        collect_tac_files(&root.join(dir_name), &mut files);
    }
    files.sort();
    assert!(
        !files.is_empty(),
        "no .tac files found under examples/ or corpus/"
    );

    let mut failures: Vec<String> = Vec::new();

    for path in &files {
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .display()
            .to_string();

        let bytes = match fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                failures.push(format!("{}: read error: {}", rel, e));
                continue;
            }
        };

        let node = match parse(&bytes) {
            Ok(n) => n,
            Err(e) => {
                failures.push(format!("{}: canonical parse error: {}", rel, e));
                continue;
            }
        };

        let canonical_original = emit(&node);
        let authoring_text = emit_authoring(&node, None);

        let (node2, _sc) = match parse_authoring(authoring_text.as_bytes()) {
            Ok(r) => r,
            Err(e) => {
                failures.push(format!(
                    "{}: authoring parse error: {}\n  authoring: {}",
                    rel, e, authoring_text
                ));
                continue;
            }
        };

        let canonical_roundtripped = emit(&node2);

        if canonical_original != canonical_roundtripped {
            failures.push(format!(
                "{}: canonical bytes differ after round-trip\n  before: {}\n  after:  {}",
                rel,
                String::from_utf8_lossy(&canonical_original),
                String::from_utf8_lossy(&canonical_roundtripped),
            ));
        }
    }

    if !failures.is_empty() {
        panic!(
            "{} round-trip stability failure(s):\n{}",
            failures.len(),
            failures.join("\n---\n")
        );
    }
}
