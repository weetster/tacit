"""Parse-and-normalize tests called out in plans/test-vectors/README.md.

Some spec rules can't be checked purely through round-trip on fixture
bytes — they require constructing a non-canonical AST (or non-canonical
input) and confirming the canonicalizer normalizes it.
"""

from __future__ import annotations

from tacit_canon import ast as A
from tacit_canon import emit, parse


def test_i1_signed_zero_normalizes() -> None:
    # Input `(int -0)` is non-canonical but semantically a zero integer.
    # ADR 0010 I1: must emit as `(int 0)`.
    assert emit(parse(b"(int -0)")) == b"(int 0)"
    # Constructed AST with value 0 also emits without sign.
    assert emit(A.Int_(0)) == b"(int 0)"
    assert emit(A.Int_(-0)) == b"(int 0)"


def test_s1_named_escape_preferred_over_hex() -> None:
    # Bytes with a named escape must not come out as \u{HEX}.
    assert emit(A.Str_('"')) == b'(str "\\"")'
    assert emit(A.Str_("\\")) == b'(str "\\\\")'
    assert emit(A.Str_("\t")) == b'(str "\\t")'
    assert emit(A.Str_("\n")) == b'(str "\\n")'
    assert emit(A.Str_("\r")) == b'(str "\\r")'


def test_s2_non_ascii_emits_as_hex() -> None:
    # Raw non-ASCII input is accepted by the parser and re-emitted escaped.
    raw = '(str "A\U0001f600")'.encode("utf-8")
    assert emit(parse(raw)) == b'(str "A\\u{1f600}")'


def test_s3_nul_uses_single_zero() -> None:
    assert emit(A.Str_("\x00")) == b'(str "\\u{0}")'


def test_s3_shortest_lowercase_hex() -> None:
    # Width boundary probes: 1, 4, 5, and 6 digit values.
    assert emit(A.Str_("\x7f")) == b'(str "\\u{7f}")'
    assert emit(A.Str_("￿")) == b'(str "\\u{ffff}")'
    assert emit(A.Str_("\U0001f600")) == b'(str "\\u{1f600}")'
    assert emit(A.Str_("\U0010ffff")) == b'(str "\\u{10ffff}")'


def test_i2_bignum_roundtrip() -> None:
    # Well beyond i128.
    fifty_nines = "9" * 50
    src = f"(int {fifty_nines})".encode("ascii")
    assert emit(parse(src)) == src
    assert parse(src) == A.Int_(int(fifty_nines))
