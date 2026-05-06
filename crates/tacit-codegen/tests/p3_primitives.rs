//! Phase 3 primitive exit-gate tests (ADR 0047, ADR 0061, ADR 0062, ADR 0063).
//!
//! Exercises each new Phase 3 primitive end-to-end — authoring view → compile
//! → link → run.  One positive and one boundary case per primitive.
//!
//! Gated on the `llvm` feature aggregate (turned on by any of the per-version
//! `llvm<N>-<M>` features). Without the feature this module is empty.

#![cfg(feature = "llvm")]

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use tacit_codegen::compile::compile_to_object;
use tacit_canonical::parse as parse_canonical;

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
    let node = parse_canonical(&src).expect("parse canonical");
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

fn run_with_stdin(built: &Built, stdin_bytes: &[u8]) -> (Vec<u8>, i32) {
    let mut child = Command::new(&built.exe)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("spawn exe");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin_bytes)
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait");
    (out.stdout, out.status.code().unwrap_or(-1))
}

fn run_p3_with_stdin(stem: &str, stdin_bytes: &[u8]) -> (Vec<u8>, i32) {
    let path = examples_root().join(format!("{}.tac", stem));
    run_with_stdin(&build(&path, stem), stdin_bytes)
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

// ── ordering primitives ──────────────────────────────────────────────────────

#[test]
fn sort_i64_orders_signed_values() {
    let (out, code) = run_p3("p3-sort-i64");
    assert!(out.is_empty());
    assert_eq!(code, 4);
}

#[test]
fn sort_i64_zero_count_leaves_vector_unchanged() {
    let (out, code) = run_p3("p3-sort-i64-empty");
    assert!(out.is_empty());
    assert_eq!(code, 42);
}

#[test]
fn sort_ranges_by_bytes_orders_range_rows_lexicographically() {
    let (out, code) = run_p3("p3-sort-ranges-by-bytes");
    assert!(out.is_empty());
    assert_eq!(code, 8);
}

#[test]
fn stable_sort_pairs_i64_preserves_equal_key_order() {
    let (out, code) = run_p3("p3-stable-sort-pairs-i64");
    assert!(out.is_empty());
    assert_eq!(code, 8);
}

// ── search and adjacent range grouping primitives ───────────────────────────

#[test]
fn lower_bound_i64_finds_hit_and_insertion_point() {
    let (out, code) = run_p3("p3-lower-bound-i64");
    assert!(out.is_empty());
    assert_eq!(code, 24);
}

#[test]
fn lower_bound_i64_empty_prefix_returns_zero() {
    let (out, code) = run_p3("p3-lower-bound-i64-empty");
    assert!(out.is_empty());
    assert_eq!(code, 0);
}

#[test]
fn count_equal_ranges_writes_count_triples() {
    let (out, code) = run_p3("p3-count-equal-ranges");
    assert!(out.is_empty());
    assert_eq!(code, 10);
}

#[test]
fn count_equal_ranges_empty_input_returns_zero_groups() {
    let (out, code) = run_p3("p3-count-equal-ranges-empty");
    assert!(out.is_empty());
    assert_eq!(code, 0);
}

#[test]
fn dedup_adjacent_ranges_writes_unique_pairs() {
    let (out, code) = run_p3("p3-dedup-adjacent-ranges");
    assert!(out.is_empty());
    assert_eq!(code, 7);
}

#[test]
fn dedup_adjacent_ranges_empty_input_returns_zero_groups() {
    let (out, code) = run_p3("p3-dedup-adjacent-ranges-empty");
    assert!(out.is_empty());
    assert_eq!(code, 0);
}

#[test]
fn dedup_adjacent_ranges_can_compact_in_place() {
    let (out, code) = run_p3("p3-dedup-adjacent-ranges-in-place");
    assert!(out.is_empty());
    assert_eq!(code, 5);
}

// ── Bundle E: stream IO sugar (ADR 0067) ─────────────────────────────────────

#[test]
fn buf_rev_reverses_in_place() {
    // Reverse "Hello" (bytes 0..5), keep trailing '\n' intact (byte 5),
    // then write the whole 6-byte buffer → "olleH\n".
    let (out, code) = run_p3("p3-buf-rev");
    assert_eq!(out, b"olleH\n");
    assert_eq!(code, 0);
}

#[test]
fn buf_rev_zero_len_leaves_buffer_unchanged() {
    // buf[0]='A' (65), buf[1]='B' (66); reverse 0 bytes → buf[0] still 'A'.
    let (out, code) = run_p3("p3-buf-rev-len-zero");
    assert!(out.is_empty());
    assert_eq!(code, 65);
}

#[test]
fn write_range_emits_slice() {
    // "Hello\n" in buf; write off=1 len=4 → "ello".
    let (out, code) = run_p3("p3-write-range");
    assert_eq!(out, b"ello");
    assert_eq!(code, 0);
}

#[test]
fn write_range_zero_len_writes_nothing() {
    // write-range with len=0 emits no bytes; buf[0]=90 ('Z') unchanged.
    let (out, code) = run_p3("p3-write-range-zero-len");
    assert!(out.is_empty());
    assert_eq!(code, 90);
}

#[test]
fn stdin_slurp_reads_input_and_echoes_via_write_range() {
    // slurp piped stdin into a 16-byte buffer, write back exactly what we read.
    let (out, code) = run_p3_with_stdin("p3-stdin-slurp", b"hello\n");
    assert_eq!(out, b"hello\n");
    assert_eq!(code, 0);
}

#[test]
fn stdin_slurp_empty_input_returns_zero() {
    // No bytes on stdin → @stdin-slurp returns 0 (and 0 becomes the exit code).
    let (out, code) = run_p3_with_stdin("p3-stdin-slurp-empty", b"");
    assert!(out.is_empty());
    assert_eq!(code, 0);
}

// ── Bundle F: ASCII case + classification (ADR 0068) ─────────────────────────

#[test]
fn ascii_tolower_boundaries() {
    // Inputs sweep the lower-source range (A-Z, 65..=90):
    //   64='@' identity, 65='A'→97='a', 90='Z'→122='z',
    //   91='[' identity, 96='`' identity, 97='a' identity,
    //   122='z' identity, 123='{' identity.
    let (out, code) = run_p3("p3-ascii-tolower");
    assert_eq!(out, &[64u8, 97, 122, 91, 96, 97, 122, 123][..]);
    assert_eq!(code, 0);
}

#[test]
fn ascii_toupper_boundaries() {
    // Inputs sweep the upper-source range (a-z, 97..=122):
    //   64 identity, 65 identity, 90 identity, 91 identity,
    //   96 identity, 97='a'→65='A', 122='z'→90='Z', 123 identity.
    let (out, code) = run_p3("p3-ascii-toupper");
    assert_eq!(out, &[64u8, 65, 90, 91, 96, 65, 90, 123][..]);
    assert_eq!(code, 0);
}

#[test]
fn ascii_case_extended_inputs_are_unchanged() {
    // Case shifts must be identity on 0, 32, 127, 128, 255, and -1.
    // -1 stored low-byte → 0xFF; 128/255 likewise pass through unchanged.
    let (out, code) = run_p3("p3-ascii-case-extended");
    assert_eq!(
        out,
        &[
            0u8,  // tolower 0
            32,   // tolower 32
            127,  // tolower 127
            128,  // tolower 128
            0xFF, // tolower -1 (stored low byte)
            0,    // toupper 0
            32,   // toupper 32
            127,  // toupper 127
            255,  // toupper 255
            0xFF, // toupper -1
        ][..]
    );
    assert_eq!(code, 0);
}

#[test]
fn ascii_is_alpha_boundaries() {
    // Encoded as ASCII '0'/'1':
    //   64='@'→0, 65='A'→1, 90='Z'→1, 91='['→0,
    //   96='`'→0, 97='a'→1, 122='z'→1, 123='{'→0.
    let (out, code) = run_p3("p3-ascii-is-alpha");
    assert_eq!(out, b"01100110");
    assert_eq!(code, 0);
}

#[test]
fn ascii_is_digit_boundaries() {
    // 47=':' identity 0, 48='0'→1, 53='5'→1, 57='9'→1,
    // 58=':'→0, 65='A'→0.
    let (out, code) = run_p3("p3-ascii-is-digit");
    assert_eq!(out, b"011100");
    assert_eq!(code, 0);
}

#[test]
fn ascii_is_space_boundaries() {
    // Bytes 9..=13 and 32 are whitespace; everything else is not.
    //   8→0, 9→1, 10→1, 11→1, 12→1, 13→1, 14→0, 31→0, 32→1, 33→0, 65→0.
    let (out, code) = run_p3("p3-ascii-is-space");
    assert_eq!(out, b"01111100100");
    assert_eq!(code, 0);
}

#[test]
fn ascii_class_extended_inputs_return_zero() {
    // Classification primitives return 0 for inputs outside 0..=127 and for
    // ASCII bytes that aren't members of their class.
    //   is-alpha 0→0, is-alpha 127→0, is-alpha 128→0, is-alpha 255→0,
    //   is-digit 200→0, is-digit 0→0,
    //   is-space 200→0, is-space 0→0,
    //   is-alpha -1→0.
    let (out, code) = run_p3("p3-ascii-class-extended");
    assert_eq!(out, b"000000000");
    assert_eq!(code, 0);
}

// ── Bundle G: UTF-8 codepoint primitives (ADR 0069) ──────────────────────────

#[test]
fn utf8_decode_each_width() {
    // Decode 1, 2, 3, 4-byte sequences for U+0041, U+00E9, U+4E2D, U+1F600.
    // Each test asserts packed = cp*8 + byte_len; exit count = 4 if all four pass.
    let (out, code) = run_p3("p3-utf8-decode-widths");
    assert!(out.is_empty());
    assert_eq!(code, 4);
}

#[test]
fn utf8_decode_returns_zero_for_invalid_inputs() {
    // Five malformed byte sequences must each decode to 0:
    //   lone continuation 0x80, overlong U+0000 (0xC0 0x80),
    //   truncated 4-byte (0xF0 0x9F 0x00 0x00),
    //   surrogate U+D800 (0xED 0xA0 0x80),
    //   above-Unicode (0xF4 0x90 0x80 0x80 → cp 0x110000).
    let (out, code) = run_p3("p3-utf8-decode-invalid");
    assert!(out.is_empty());
    assert_eq!(code, 5);
}

#[test]
fn utf8_encode_round_trips_each_width() {
    // Encode U+0041, U+00E9, U+4E2D, U+1F600 into a 10-byte buffer and write
    // it back; exit code = 1+2+3+4 = 10 if every encode returned its width.
    let (out, code) = run_p3("p3-utf8-encode-roundtrip");
    assert_eq!(
        out,
        &[0x41u8, 0xC3, 0xA9, 0xE4, 0xB8, 0xAD, 0xF0, 0x9F, 0x98, 0x80][..]
    );
    assert_eq!(code, 10);
}

#[test]
fn utf8_encode_rejects_invalid_codepoints_without_writing() {
    // -1, 0xD800 (surrogate), and 0x110000 (above Unicode) must each return 0
    // and leave the sentinel buffer "cccc" untouched.
    let (out, code) = run_p3("p3-utf8-encode-invalid");
    assert_eq!(out, b"cccc");
    assert_eq!(code, 0);
}

#[test]
fn utf8_len_agrees_with_encode_and_rejects_invalid() {
    // utf8-len returns 1..=4 for each valid codepoint and 0 for invalid;
    // exit count = 7 if all seven assertions hold.
    let (out, code) = run_p3("p3-utf8-len");
    assert!(out.is_empty());
    assert_eq!(code, 7);
}
