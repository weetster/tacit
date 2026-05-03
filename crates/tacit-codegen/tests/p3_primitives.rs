//! Phase 3 primitive exit-gate tests (ADR 0047, ADR 0061, ADR 0062, ADR 0063).
//!
//! Exercises each new Phase 3 primitive end-to-end — authoring view → compile
//! → link → run.  One positive and one boundary case per primitive.
//!
//! Gated on the `llvm` feature aggregate (turned on by any of the per-version
//! `llvm<N>-<M>` features). Without the feature this module is empty.

#![cfg(feature = "llvm")]

use std::path::{Path, PathBuf};
use std::process::Command;

use tacit_codegen::compile::compile_to_object;
use tacit_views::authoring::parse_authoring;

fn examples_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("smoke")
}

fn pick_linker() -> Option<String> {
    for cand in ["cc", "clang", "gcc"] {
        if which(cand) {
            return Some(cand.to_string());
        }
    }
    None
}

fn which(cmd: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {} >/dev/null 2>&1", cmd))
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

struct Built {
    exe: PathBuf,
    _tmp: tempfile::TempDir,
}

fn build(program_path: &Path, name: &str) -> Built {
    let src = std::fs::read(program_path).expect("read program");
    let (node, _sidecar) = parse_authoring(&src).expect("parse authoring view");
    let tmp = tempfile::tempdir().expect("tempdir");
    let obj = tmp.path().join(format!("{}.o", name));
    compile_to_object(&node, name, &obj).expect("emit object");
    let exe = tmp.path().join(name);
    let linker = pick_linker()
        .expect("no C linker found (cc/clang/gcc); install Xcode CLT or build-essential");
    let status = Command::new(&linker)
        .arg(&obj)
        .arg("-o")
        .arg(&exe)
        .status()
        .expect("invoke linker");
    assert!(status.success(), "linker failed for {}", name);
    Built { exe, _tmp: tmp }
}

fn run(built: &Built) -> (Vec<u8>, i32) {
    let out = Command::new(&built.exe).output().expect("run exe");
    (out.stdout, out.status.code().unwrap_or(-1))
}

fn run_p3(stem: &str) -> (Vec<u8>, i32) {
    let path = examples_root().join(format!("{}.tac", stem));
    run(&build(&path, stem))
}

// ── @buf-alloc-dyn ────────────────────────────────────────────────────────────

#[test]
fn buf_alloc_dyn_positive() {
    // alloc 3 bytes at runtime, set buf[0]=77, get buf[0] → exit 77
    let (out, code) = run_p3("p3-buf-alloc-dyn");
    assert!(out.is_empty());
    assert_eq!(code, 77);
}

#[test]
fn buf_alloc_dyn_min_size() {
    // alloc exactly 1 byte (boundary: minimum useful size)
    let (out, code) = run_p3("p3-buf-alloc-dyn-min");
    assert!(out.is_empty());
    assert_eq!(code, 99);
}

// ── @buf-get / @buf-set ───────────────────────────────────────────────────────

#[test]
fn buf_get_set_nonzero_offset() {
    // set buf[2]=88, get buf[2] → exit 88 (positive: non-zero offset)
    let (out, code) = run_p3("p3-buf-get-set");
    assert!(out.is_empty());
    assert_eq!(code, 88);
}

#[test]
fn buf_get_set_zero_offset() {
    // set buf[0]=55, get buf[0] → exit 55 (boundary: offset 0)
    let (out, code) = run_p3("p3-buf-get-set-zero");
    assert!(out.is_empty());
    assert_eq!(code, 55);
}

// ── @buf-copy ────────────────────────────────────────────────────────────────

#[test]
fn buf_copy_positive() {
    // copy "Hi\n" from src to dst, write to stdout
    let (out, code) = run_p3("p3-buf-copy");
    assert_eq!(out, b"Hi\n");
    assert_eq!(code, 0);
}

#[test]
fn buf_copy_zero_len() {
    // copy 0 bytes → dst unchanged; get dst[0] which was set to 90 ('Z')
    let (out, code) = run_p3("p3-buf-copy-zero");
    assert!(out.is_empty());
    assert_eq!(code, 90);
}

// ── @buf-eq ───────────────────────────────────────────────────────────────────

#[test]
fn buf_eq_equal() {
    // two identical 2-byte regions → 1
    let (out, code) = run_p3("p3-buf-eq-equal");
    assert!(out.is_empty());
    assert_eq!(code, 1);
}

#[test]
fn buf_eq_not_equal() {
    // two regions differing at index 1 → 0
    let (out, code) = run_p3("p3-buf-eq-neq");
    assert!(out.is_empty());
    assert_eq!(code, 0);
}

// ── @scan-byte ────────────────────────────────────────────────────────────────

#[test]
fn scan_byte_found() {
    // "hello\n" in buf, scan for '\n' (10) → exit 5
    let (out, code) = run_p3("p3-scan-byte-found");
    assert!(out.is_empty());
    assert_eq!(code, 5);
}

#[test]
fn scan_byte_not_found() {
    // "hello" in buf (no '\n'), scan for 10 → exit off+len = 0+5 = 5
    let (out, code) = run_p3("p3-scan-byte-notfound");
    assert!(out.is_empty());
    assert_eq!(code, 5);
}

// ── @parse-i64 ────────────────────────────────────────────────────────────────

#[test]
fn parse_i64_positive() {
    // parse "42" from buf → exit 42
    let (out, code) = run_p3("p3-parse-i64");
    assert!(out.is_empty());
    assert_eq!(code, 42);
}

#[test]
fn parse_i64_empty_range() {
    // parse with len=0 → exit 0 (boundary: empty range)
    let (out, code) = run_p3("p3-parse-i64-empty");
    assert!(out.is_empty());
    assert_eq!(code, 0);
}

// ── @fmt-i64 ────────────────────────────────────────────────────────────────

#[test]
fn fmt_i64_positive() {
    // format 42 → stdout "42"
    let (out, code) = run_p3("p3-fmt-i64");
    assert_eq!(out, b"42");
    assert_eq!(code, 0);
}

#[test]
fn fmt_i64_zero() {
    // format 0 → stdout "0" (boundary: special zero case)
    let (out, code) = run_p3("p3-fmt-i64-zero");
    assert_eq!(out, b"0");
    assert_eq!(code, 0);
}

// ── @i64-alloc / @i64-get / @i64-set ─────────────────────────────────────────

#[test]
fn i64_get_set_positive_zero_negative() {
    // store positive, zero, and negative values; all equality checks pass → 3
    let (out, code) = run_p3("p3-i64-get-set");
    assert!(out.is_empty());
    assert_eq!(code, 3);
}

#[test]
fn i64_alloc_dynamic_count() {
    // allocate count from runtime expression, set/read index 2 → 44
    let (out, code) = run_p3("p3-i64-alloc-dyn");
    assert!(out.is_empty());
    assert_eq!(code, 44);
}

// ── @i64-swap ────────────────────────────────────────────────────────────────

#[test]
fn i64_swap_distinct_indexes() {
    // [4, 9] swapped → [9, 4] → 94
    let (out, code) = run_p3("p3-i64-swap");
    assert!(out.is_empty());
    assert_eq!(code, 94);
}

#[test]
fn i64_swap_same_index() {
    // swap index 0 with itself leaves the value unchanged
    let (out, code) = run_p3("p3-i64-swap-same");
    assert!(out.is_empty());
    assert_eq!(code, 33);
}

// ── @i64-copy ────────────────────────────────────────────────────────────────

#[test]
fn i64_copy_zero_count() {
    // zero-count copy leaves dst[0] unchanged
    let (out, code) = run_p3("p3-i64-copy-zero");
    assert!(out.is_empty());
    assert_eq!(code, 77);
}

#[test]
fn i64_copy_cross_vector() {
    // copy src[0..2] into dst[1..3], then dst[1] + dst[2] = 33
    let (out, code) = run_p3("p3-i64-copy-cross");
    assert!(out.is_empty());
    assert_eq!(code, 33);
}

#[test]
fn i64_copy_overlap_same_vector() {
    // [1,2,3,4], copy xs[0..3] to xs[1..4] → [1,1,2,3] → 23
    let (out, code) = run_p3("p3-i64-copy-overlap");
    assert!(out.is_empty());
    assert_eq!(code, 23);
}

// ── @line-index / @token-index / @token-index-any / accessors ────────────────

#[test]
fn range_accessors_read_pair_fields() {
    // table rows [(7,4), (9,2)]; row 1 start and row 0 len → 94
    let (out, code) = run_p3("p3-range-accessors");
    assert!(out.is_empty());
    assert_eq!(code, 94);
}

#[test]
fn line_index_empty_input_returns_zero_rows() {
    let (out, code) = run_p3("p3-line-index-empty");
    assert!(out.is_empty());
    assert_eq!(code, 0);
}

#[test]
fn line_index_no_trailing_lf_emits_final_segment() {
    // "AB" → one row of length 2; encoded result is 12
    let (out, code) = run_p3("p3-line-index-basic");
    assert!(out.is_empty());
    assert_eq!(code, 12);
}

#[test]
fn line_index_preserves_empty_lines_but_not_final_extra_row() {
    // "\nA\n\n" → rows: "", "A", ""; encoded result is 125
    let (out, code) = run_p3("p3-line-index-edge");
    assert!(out.is_empty());
    assert_eq!(code, 125);
}

#[test]
fn token_index_empty_input_returns_zero_rows() {
    let (out, code) = run_p3("p3-token-index-empty");
    assert!(out.is_empty());
    assert_eq!(code, 0);
}

#[test]
fn token_index_skips_delims_and_uses_absolute_offsets() {
    // In "xx a  b yy", indexing text[2..8) with delimiter low byte 32 gives
    // rows for "a" at 3 and "b" at 6; encoded result is 116.
    let (out, code) = run_p3("p3-token-index-offset");
    assert!(out.is_empty());
    assert_eq!(code, 116);
}

#[test]
fn token_index_any_empty_input_returns_zero_rows() {
    let (out, code) = run_p3("p3-token-index-any-empty");
    assert!(out.is_empty());
    assert_eq!(code, 0);
}

#[test]
fn token_index_any_skips_repeated_mixed_buffer_delims() {
    // " A\n\nB  C " with delimiters [' ', '\n'] gives starts 1, 4, 7.
    let (out, code) = run_p3("p3-token-index-any-mixed");
    assert!(out.is_empty());
    assert_eq!(code, 158);
}

#[test]
fn token_index_any_accepts_string_delims_and_absolute_offsets() {
    // In "xx,A;B yy", indexing text[2..7) with ",; " gives starts 3 and 5.
    let (out, code) = run_p3("p3-token-index-any-offset");
    assert!(out.is_empty());
    assert_eq!(code, 115);
}

#[test]
fn token_index_any_zero_delim_count_emits_whole_range() {
    let (out, code) = run_p3("p3-token-index-any-no-delims");
    assert!(out.is_empty());
    assert_eq!(code, 13);
}
