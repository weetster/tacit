"""Verify that files under corpus/sealed/ match corpus/sealed-hashes.txt.

Walks every file under `sealed/`, computes BLAKE3, and asserts the
recorded hash matches. Also errors on extra files not in the manifest
and on manifest entries with missing files. Exit code is zero only if
the sealed tree matches the manifest exactly.

Intended to run in CI. Per ADR 0020, this is the load-bearing
tamper-detection mechanism for the Phase 3 held-out subset.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import blake3

from tacit_corpus._paths import CORPUS_ROOT, SEALED_HASHES_FILE, SEALED_ROOT


def _hash_file(path: Path) -> str:
    h = blake3.blake3()
    with path.open("rb") as f:
        while chunk := f.read(1 << 16):
            h.update(chunk)
    return h.hexdigest()


def _read_manifest() -> dict[str, str]:
    if not SEALED_HASHES_FILE.is_file():
        print(f"error: {SEALED_HASHES_FILE} missing", file=sys.stderr)
        sys.exit(2)
    entries: dict[str, str] = {}
    for lineno, raw in enumerate(SEALED_HASHES_FILE.read_text().splitlines(), 1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split()
        if len(parts) != 2:
            print(f"error: {SEALED_HASHES_FILE}:{lineno}: malformed line", file=sys.stderr)
            sys.exit(2)
        path, hexhash = parts
        entries[path] = hexhash
    return entries


def _walk_sealed() -> list[Path]:
    return sorted(p for p in SEALED_ROOT.rglob("*") if p.is_file())


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--write",
        action="store_true",
        help="rewrite sealed-hashes.txt from the current sealed/ tree (use after adding new sealed tasks)",
    )
    args = parser.parse_args()

    files = _walk_sealed()

    if args.write:
        lines = [
            "# Hashes of every file under corpus/sealed/, one line per file.",
            "# Format: <relative-path-from-corpus-root> <blake3-hex>",
            "# Sorted by path. Regenerate via: `uv run corpus-verify-sealed --write`.",
            "# Any edit to a sealed file must be accompanied by a regen and an ADR.",
            "",
        ]
        for f in files:
            rel = str(f.relative_to(CORPUS_ROOT))
            lines.append(f"{rel} {_hash_file(f)}")
        SEALED_HASHES_FILE.write_text("\n".join(lines) + "\n")
        print(f"wrote {len(files)} entries to {SEALED_HASHES_FILE.relative_to(CORPUS_ROOT)}")
        return 0

    expected = _read_manifest()
    observed = {str(f.relative_to(CORPUS_ROOT)): _hash_file(f) for f in files}

    missing = sorted(set(expected) - set(observed))
    extra = sorted(set(observed) - set(expected))
    mismatched = sorted(p for p in expected if p in observed and expected[p] != observed[p])

    for p in missing:
        print(f"MISSING  {p} (in manifest, not on disk)")
    for p in extra:
        print(f"EXTRA    {p} (on disk, not in manifest)")
    for p in mismatched:
        print(f"CHANGED  {p}")
        print(f"  expected {expected[p]}")
        print(f"  observed {observed[p]}")

    ok = not (missing or extra or mismatched)
    if ok:
        print(f"sealed tree matches manifest ({len(expected)} files)")
        return 0
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
