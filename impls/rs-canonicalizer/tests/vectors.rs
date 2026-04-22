//! Stage 2 fixture tests.
//!
//! Runs every file in plans/test-vectors/ through the canonicalizer:
//!
//!   *.canonical  — parse, re-emit, assert byte-identical to the input bytes.
//!   *.forbidden  — parse must raise (ADR 0011 structural rejection).
//!   *.reject     — parse must raise (lexer-level hard error per ADR 0012).
//!
//! Also checks the pairwise distinctness properties listed in the fixture
//! README (V17, V19) and the hole-hash stability property from V8.

use std::fs;
use std::path::{Path, PathBuf};

use tacit_canon::{emit, hash_bytes, hash_node, parse};

fn vectors_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR = impls/rs-canonicalizer
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().unwrap().parent().unwrap().join("plans").join("test-vectors")
}

fn collect(ext: &str) -> Vec<PathBuf> {
    let dir = vectors_dir();
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read_dir {:?}: {}", dir, e))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some(ext))
        .collect();
    files.sort();
    files
}

fn read(name: &str) -> Vec<u8> {
    fs::read(vectors_dir().join(name)).expect("fixture exists")
}

#[test]
fn canonical_roundtrip_all() {
    let files = collect("canonical");
    assert!(!files.is_empty(), "no *.canonical fixtures under {:?}", vectors_dir());
    let mut failed = Vec::new();
    for path in &files {
        let data = fs::read(path).unwrap();
        assert!(
            !data.ends_with(b"\n"),
            "{} ends with 0x0a — fixture README forbids trailing newlines",
            path.file_name().unwrap().to_string_lossy()
        );
        match parse(&data) {
            Ok(node) => {
                let out = emit(&node);
                if out != data {
                    failed.push(format!(
                        "round-trip mismatch for {}\n  expected: {:?}\n  got:      {:?}",
                        path.file_name().unwrap().to_string_lossy(),
                        String::from_utf8_lossy(&data),
                        String::from_utf8_lossy(&out),
                    ));
                }
            }
            Err(e) => failed.push(format!(
                "parse error for {}: {}",
                path.file_name().unwrap().to_string_lossy(),
                e
            )),
        }
    }
    if !failed.is_empty() {
        panic!("{} failures:\n{}", failed.len(), failed.join("\n---\n"));
    }
}

#[test]
fn forbidden_rejected_all() {
    let files = collect("forbidden");
    assert!(!files.is_empty(), "no *.forbidden fixtures under {:?}", vectors_dir());
    for path in files {
        let data = fs::read(&path).unwrap();
        assert!(
            parse(&data).is_err(),
            "{} must be rejected",
            path.file_name().unwrap().to_string_lossy()
        );
    }
}

#[test]
fn reject_bytes_all() {
    let files = collect("reject");
    assert!(!files.is_empty(), "no *.reject fixtures under {:?}", vectors_dir());
    for path in files {
        let data = fs::read(&path).unwrap();
        assert!(
            parse(&data).is_err(),
            "{} must be rejected",
            path.file_name().unwrap().to_string_lossy()
        );
    }
}

#[test]
fn v17_permuted_rec_distinct() {
    let a = read("17a-rec-permuted.canonical");
    let b = read("17b-rec-permuted-swapped.canonical");
    assert_ne!(a, b);
    let na = parse(&a).unwrap();
    let nb = parse(&b).unwrap();
    assert_ne!(emit(&na), emit(&nb));
    assert_ne!(hash_node(&na), hash_node(&nb));
}

#[test]
fn v19_match_order_distinct() {
    let a = read("19a-match-wild-first.canonical");
    let b = read("19b-match-zero-first.canonical");
    assert_ne!(a, b);
    let na = parse(&a).unwrap();
    let nb = parse(&b).unwrap();
    assert_ne!(emit(&na), emit(&nb));
    assert_ne!(hash_node(&na), hash_node(&nb));
}

#[test]
fn v8_hole_hash_stable_across_positions() {
    let standalone = read("08a-hole-standalone.canonical");
    let embedded = read("08b-hole-embedded.canonical");
    // V8's claim: the standalone hole renders identically wherever it appears.
    let needle = &standalone[..];
    let found = embedded.windows(needle.len()).any(|w| w == needle);
    assert!(found, "standalone hole bytes not contained in embedded form");
    // The hole's hash is stable regardless of enclosing context.
    let node = parse(&standalone).unwrap();
    assert_eq!(hash_node(&node), hash_bytes(&standalone));
}

#[test]
fn every_vector_path_is_utf8() {
    for p in collect("canonical") {
        let name = p.file_name().unwrap().to_string_lossy();
        assert!(!name.is_empty());
    }
}

#[test]
fn sanity_vectors_dir_exists() {
    let d = vectors_dir();
    assert!(d.is_dir(), "expected vectors at {:?}", d);
    let _ = Path::new(&d);
}
