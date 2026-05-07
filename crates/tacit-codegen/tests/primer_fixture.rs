//! Phase 3 Stage 7 primer fixture.
//!
//! The primer is model-facing teaching material, so every fenced Tacit block
//! must stay executable or intentionally failing with a documented diagnostic.
//! This fixture also checks primer blocks against open corpus Tacit references.
//! It intentionally does not read `corpus/sealed/**`; agent-level sealing rules
//! forbid accessing that subtree.

#![cfg(feature = "llvm")]

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use tacit_canonical::ast::Node;
use tacit_codegen::analysis::{check_closed, check_no_holes};
use tacit_codegen::compile_to_ir_string;
use tacit_typecheck::infer_module;
use tacit_views::authoring::parse_authoring;

const STDLIB_APPENDIX_HEADING: &str =
    "## Stdlib Appendix: Indexed Storage, Text Ranges, Ordering, Grouping, Stream IO, ASCII, And UTF-8";

#[derive(Debug)]
struct Block {
    line: usize,
    info: String,
    source: String,
}

#[derive(Debug, PartialEq, Eq)]
enum Expectation {
    Success,
    Fail(String),
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn primer_path() -> PathBuf {
    repo_root()
        .join("plans")
        .join("primer")
        .join("tacit-lite-primer.md")
}

fn extract_tacit_blocks(markdown: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut current: Option<(usize, String, Vec<String>)> = None;

    for (idx, line) in markdown.lines().enumerate() {
        let line_no = idx + 1;
        if let Some(rest) = line.strip_prefix("```") {
            if let Some((start, info, lines)) = current.take() {
                blocks.push(Block {
                    line: start,
                    info,
                    source: lines.join("\n"),
                });
            } else {
                let info = rest.trim().to_string();
                if info.starts_with("tacit") {
                    current = Some((line_no, info, Vec::new()));
                }
            }
            continue;
        }

        if let Some((_, _, lines)) = current.as_mut() {
            lines.push(line.to_string());
        }
    }

    assert!(
        current.is_none(),
        "unterminated Tacit fence in {}",
        primer_path().display()
    );
    blocks
}

fn expectation(info: &str) -> Expectation {
    for raw in info.split_whitespace().skip(1) {
        let attr = raw.trim_matches(|c| c == '{' || c == '}');
        if let Some(value) = attr.strip_prefix("fail=") {
            let kind = value.trim_matches(|c| c == '"' || c == '\'');
            return Expectation::Fail(kind.to_string());
        }
    }
    Expectation::Success
}

fn collect_reference_sources(dir: &Path, out: &mut Vec<(PathBuf, String)>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {}", dir.display(), e)) {
        let entry = entry.unwrap_or_else(|e| panic!("read entry in {}: {}", dir.display(), e));
        let path = entry.path();
        if path.is_dir() {
            collect_reference_sources(&path, out);
        } else if path.file_name().and_then(|n| n.to_str()) == Some("reference.tac") {
            let src = fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("read {}: {}", path.display(), e));
            out.push((path, src));
        }
    }
}

fn lexical_units(src: &str) -> Vec<String> {
    let mut units = Vec::new();
    let mut buf = String::new();
    for ch in src.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '@') {
            buf.push(ch);
        } else {
            if !buf.is_empty() {
                units.push(std::mem::take(&mut buf));
            }
            if !ch.is_whitespace() {
                units.push(ch.to_string());
            }
        }
    }
    if !buf.is_empty() {
        units.push(buf);
    }
    units
}

fn windows(units: &[String], size: usize) -> HashSet<String> {
    units
        .windows(size)
        .map(|window| window.join("\u{1f}"))
        .collect()
}

fn assert_not_corpus_reference(block: &Block, references: &[(PathBuf, String)]) {
    let block_trimmed = block.source.trim();
    let block_units = lexical_units(block_trimmed);
    let block_windows = windows(&block_units, 32);

    for (path, reference) in references {
        let reference_trimmed = reference.trim();
        assert_ne!(
            block_trimmed,
            reference_trimmed,
            "primer Tacit block at line {} exactly matches {}",
            block.line,
            path.display()
        );

        let reference_units = lexical_units(reference_trimmed);
        for candidate in reference_units
            .windows(32)
            .map(|window| window.join("\u{1f}"))
        {
            assert!(
                !block_windows.contains(&candidate),
                "primer Tacit block at line {} shares a 32-token run with {}",
                block.line,
                path.display()
            );
        }
    }
}

fn parse_block(block: &Block) -> Node {
    let (node, _) = parse_authoring(block.source.as_bytes())
        .unwrap_or_else(|e| panic!("parse primer block at line {}: {}", block.line, e));
    node
}

fn assert_success_node_valid(block: &Block, node: &Node, module_name: &str) {
    check_no_holes(node).unwrap_or_else(|e| panic!("hole check line {}: {}", block.line, e));
    check_closed(node, 0).unwrap_or_else(|e| panic!("closed check line {}: {}", block.line, e));
    infer_module(node).unwrap_or_else(|diags| panic!("typecheck line {}: {:?}", block.line, diags));
    compile_to_ir_string(node, module_name)
        .unwrap_or_else(|e| panic!("codegen line {}: {}", block.line, e));
}

fn assert_success_block_valid(block: &Block, module_name: &str) {
    let node = parse_block(block);
    assert_success_node_valid(block, &node, module_name);
}

fn stdlib_appendix(markdown: &str) -> &str {
    let start = markdown
        .find(STDLIB_APPENDIX_HEADING)
        .expect("missing I64Vec stdlib appendix");
    let rest = &markdown[start..];
    let after_heading = &rest[STDLIB_APPENDIX_HEADING.len()..];
    if let Some(next_heading) = after_heading.find("\n## ") {
        &rest[..STDLIB_APPENDIX_HEADING.len() + next_heading]
    } else {
        rest
    }
}

#[test]
fn primer_tacit_fences_validate() {
    let primer = fs::read_to_string(primer_path()).expect("read primer");
    let blocks = extract_tacit_blocks(&primer);
    assert!(
        blocks.len() >= 20,
        "expected at least 20 Tacit primer blocks"
    );

    let mut references = Vec::new();
    collect_reference_sources(&repo_root().join("corpus").join("tasks"), &mut references);
    assert!(
        !references.is_empty(),
        "expected open corpus reference.tac files for contamination check"
    );

    let mut successes = 0usize;
    let mut failures = 0usize;

    for (idx, block) in blocks.iter().enumerate() {
        assert_not_corpus_reference(block, &references);
        let node = parse_block(block);

        match expectation(&block.info) {
            Expectation::Success => {
                successes += 1;
                assert_success_node_valid(block, &node, &format!("primer_block_{}", idx));
            }
            Expectation::Fail(kind) => {
                failures += 1;
                let diags = match infer_module(&node) {
                    Ok(_) => {
                        panic!(
                            "primer block at line {} was marked fail={} but typechecked",
                            block.line, kind
                        )
                    }
                    Err(diags) => diags,
                };
                assert!(
                    diags.iter().any(|diag| diag.kind == kind),
                    "primer block at line {} expected {}, got {:?}",
                    block.line,
                    kind,
                    diags.iter().map(|diag| &diag.kind).collect::<Vec<_>>()
                );
            }
        }
    }

    assert!(
        successes >= 12,
        "expected at least 12 successful Tacit blocks"
    );
    assert!(failures >= 8, "expected at least 8 failing Tacit blocks");
}

#[test]
fn primer_stdlib_appendix_examples_validate() {
    let primer = fs::read_to_string(primer_path()).expect("read primer");
    let appendix = stdlib_appendix(&primer);
    assert!(appendix.contains("`I64Vec`"));
    assert!(appendix.contains("`@line-index text len table`"));
    assert!(appendix.contains("`@token-index text off len delim table`"));
    assert!(appendix.contains("`@token-index-any text off len delims delim_count table`"));
    assert!(appendix.contains("`@sort-i64 xs count`"));
    assert!(appendix.contains("`@sort-ranges-by-bytes text table count`"));
    assert!(appendix.contains("`@stable-sort-pairs-i64 keys values count`"));
    assert!(appendix.contains("`@lower-bound-i64 xs count value`"));
    assert!(appendix.contains("`@count-equal-ranges text table count out`"));
    assert!(appendix.contains("`@dedup-adjacent-ranges text table count out`"));
    assert!(appendix.contains("`@stdin-slurp buf cap`"));
    assert!(appendix.contains("`@write-range"));
    assert!(appendix.contains("`@ascii-tolower b`"));
    assert!(appendix.contains("`@utf8-decode buf off`"));
    for repo_term in [
        "corpus",
        "canary",
        "reference.tac",
        "reference.stdlib",
        "repo",
        "repository",
    ] {
        assert!(
            !appendix.to_lowercase().contains(repo_term),
            "stdlib appendix should avoid repository-facing term {}",
            repo_term
        );
    }

    let blocks = extract_tacit_blocks(appendix);
    assert_eq!(
        blocks.len(),
        17,
        "expected one fixture-checked Tacit block per stdlib appendix example"
    );

    let appendix_primitives = [
        "@i64-",
        "@line-index",
        "@token-index",
        "@range-",
        "@sort-i64",
        "@sort-ranges-by-bytes",
        "@stable-sort-pairs-i64",
        "@lower-bound-i64",
        "@count-equal-ranges",
        "@dedup-adjacent-ranges",
        "@stdin-slurp",
        "@write-range",
        "@buf-rev",
        "@ascii-",
        "@utf8-",
    ];

    for (idx, block) in blocks.iter().enumerate() {
        assert_eq!(expectation(&block.info), Expectation::Success);
        assert!(
            appendix_primitives
                .iter()
                .any(|primitive| block.source.contains(primitive)),
            "stdlib appendix example at line {} should exercise stdlib primitives",
            block.line
        );
        assert_success_block_valid(block, &format!("stdlib_appendix_{}", idx));
    }
}
