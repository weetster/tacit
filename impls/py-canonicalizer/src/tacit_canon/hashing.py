"""BLAKE3 content hashing over canonical text (ADR 0009).

    hash(node) = BLAKE3(canonical_text(node))

Children are fully inlined; the Merkle property falls out because
identical subtrees produce identical canonical bytes.
"""

from __future__ import annotations

from blake3 import blake3

from . import ast as A
from .emit import emit


def hash_bytes(data: bytes) -> bytes:
    return blake3(data).digest()


def hash_node(node: A.Node) -> bytes:
    return hash_bytes(emit(node))
