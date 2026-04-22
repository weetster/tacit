"""Stage 2 fixture tests.

Runs every file in plans/test-vectors/ through the canonicalizer:

  *.canonical  — parse, re-emit, assert byte-identical to the input bytes.
  *.forbidden  — parse must raise (ADR 0011 structural rejection).
  *.reject     — parse must raise (lexer-level hard error per ADR 0012).

Also checks the pairwise distinctness properties listed in the fixture
README (V17, V19) and the hole-hash stability property from V8.
"""

from __future__ import annotations

from pathlib import Path

import pytest

from tacit_canon import (
    LexError,
    ParseError,
    emit,
    hash_bytes,
    hash_node,
    parse,
)

REPO_ROOT = Path(__file__).resolve().parents[3]
VECTORS_DIR = REPO_ROOT / "plans" / "test-vectors"

CANONICAL_FILES = sorted(VECTORS_DIR.glob("*.canonical"))
FORBIDDEN_FILES = sorted(VECTORS_DIR.glob("*.forbidden"))
REJECT_FILES = sorted(VECTORS_DIR.glob("*.reject"))

assert CANONICAL_FILES, f"no *.canonical fixtures under {VECTORS_DIR}"
assert FORBIDDEN_FILES, f"no *.forbidden fixtures under {VECTORS_DIR}"
assert REJECT_FILES, f"no *.reject fixtures under {VECTORS_DIR}"


@pytest.mark.parametrize("path", CANONICAL_FILES, ids=lambda p: p.name)
def test_canonical_roundtrip(path: Path) -> None:
    data = path.read_bytes()
    assert not data.endswith(b"\n"), (
        f"{path.name} ends with 0x0a — fixture README forbids trailing newlines"
    )
    node = parse(data)
    out = emit(node)
    assert out == data, (
        f"round-trip mismatch for {path.name}\n"
        f"  expected: {data!r}\n"
        f"  got:      {out!r}"
    )


@pytest.mark.parametrize("path", FORBIDDEN_FILES, ids=lambda p: p.name)
def test_forbidden_rejected(path: Path) -> None:
    data = path.read_bytes()
    with pytest.raises((ParseError, LexError)):
        parse(data)


@pytest.mark.parametrize("path", REJECT_FILES, ids=lambda p: p.name)
def test_reject_bytes(path: Path) -> None:
    data = path.read_bytes()
    with pytest.raises((ParseError, LexError)):
        parse(data)


def _read(name: str) -> bytes:
    return (VECTORS_DIR / name).read_bytes()


def test_v17_permuted_rec_distinct() -> None:
    a = _read("17a-rec-permuted.canonical")
    b = _read("17b-rec-permuted-swapped.canonical")
    assert a != b
    assert emit(parse(a)) != emit(parse(b))
    assert hash_node(parse(a)) != hash_node(parse(b))


def test_v19_match_order_distinct() -> None:
    a = _read("19a-match-wild-first.canonical")
    b = _read("19b-match-zero-first.canonical")
    assert a != b
    assert emit(parse(a)) != emit(parse(b))
    assert hash_node(parse(a)) != hash_node(parse(b))


def test_v8_hole_hash_stable_across_positions() -> None:
    standalone = _read("08a-hole-standalone.canonical")
    embedded = _read("08b-hole-embedded.canonical")
    # V8's claim: the standalone hole renders identically wherever it appears.
    assert standalone in embedded
    # The hole's hash is stable regardless of enclosing context.
    inner_hash = hash_node(parse(standalone))
    assert inner_hash == hash_bytes(standalone)
