"""Local preflight before paid corpus runs.

Verifies the tacit binary exists, smoke-checks the new stdlib primitives,
runs the 12 stdlib-canary `reference.stdlib.tac` files against their tests,
and reports their token total against current Tacit and Python references.
Records the binary path, modification time, and BLAKE3 hash so source/binary
skew is visible.

Run before any paid `corpus-eval` invocation that uses the same `--tacit-bin`.
Exits non-zero if any stage fails so it can be wired into a wrapper script.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

import blake3
import tiktoken

from tacit_corpus._paths import OPEN_TASKS_ROOT, REPO_ROOT


CANARY_TASK_IDS: tuple[str, ...] = (
    "collections/021-unique-in-order",
    "collections/025-partition-eo",
    "collections/026-group-counts",
    "algorithms/033-two-sum",
    "algorithms/035-bubble-sort",
    "algorithms/036-quicksort",
    "algorithms/037-merge-sort",
    "algorithms/044-count-islands",
    "algorithms/049-matrix-multiply",
    "io/055-sort-lines",
    "io/056-unique-lines",
    "strings/017-common-prefix",
)


SMOKE_PROGRAMS: dict[str, str] = {
    "@token-index-any": (
        "let text = @buf-alloc 8 in\n"
        "let _ = @buf-set text 0 97 in\n"
        "let _ = @buf-set text 1 32 in\n"
        "let _ = @buf-set text 2 98 in\n"
        "let table = @i64-alloc 16 in\n"
        "let _ = @token-index-any text 0 3 \" \" 1 table in\n"
        "0\n"
    ),
    "@sort-i64": (
        "let xs = @i64-alloc 3 in\n"
        "let _ = @i64-set xs 0 3 in\n"
        "let _ = @i64-set xs 1 1 in\n"
        "let _ = @i64-set xs 2 2 in\n"
        "let _ = @sort-i64 xs 3 in\n"
        "let _ = @i64-get xs 0 in\n"
        "0\n"
    ),
    "@sort-ranges-by-bytes": (
        "let text = @buf-alloc 4 in\n"
        "let _ = @buf-set text 0 98 in\n"
        "let _ = @buf-set text 1 97 in\n"
        "let table = @i64-alloc 4 in\n"
        "let _ = @i64-set table 0 0 in\n"
        "let _ = @i64-set table 1 1 in\n"
        "let _ = @i64-set table 2 1 in\n"
        "let _ = @i64-set table 3 1 in\n"
        "let _ = @sort-ranges-by-bytes text table 2 in\n"
        "let _ = @range-start table 0 in\n"
        "0\n"
    ),
    "@count-equal-ranges": (
        "let text = @buf-alloc 2 in\n"
        "let _ = @buf-set text 0 97 in\n"
        "let _ = @buf-set text 1 97 in\n"
        "let table = @i64-alloc 4 in\n"
        "let _ = @i64-set table 0 0 in\n"
        "let _ = @i64-set table 1 1 in\n"
        "let _ = @i64-set table 2 1 in\n"
        "let _ = @i64-set table 3 1 in\n"
        "let out = @i64-alloc 6 in\n"
        "let _ = @count-equal-ranges text table 2 out in\n"
        "0\n"
    ),
    "@dedup-adjacent-ranges": (
        "let text = @buf-alloc 2 in\n"
        "let _ = @buf-set text 0 97 in\n"
        "let _ = @buf-set text 1 97 in\n"
        "let table = @i64-alloc 4 in\n"
        "let _ = @i64-set table 0 0 in\n"
        "let _ = @i64-set table 1 1 in\n"
        "let _ = @i64-set table 2 1 in\n"
        "let _ = @i64-set table 3 1 in\n"
        "let out = @i64-alloc 4 in\n"
        "let _ = @dedup-adjacent-ranges text table 2 out in\n"
        "0\n"
    ),
}


@dataclass(frozen=True)
class PreflightFailure:
    section: str
    detail: str


@dataclass(frozen=True)
class TokenTotals:
    python: int
    tacit: int
    stdlib_tacit: int
    paired_count: int


def _default_tacit_bin() -> Path | None:
    env = os.environ.get("TACIT_BIN")
    if env:
        return Path(env)
    candidate = REPO_ROOT / "target" / "debug" / "tacit"
    if candidate.is_file():
        return candidate
    found = shutil.which("tacit")
    if found:
        return Path(found)
    return None


def binary_metadata(tacit_bin: Path) -> dict[str, str]:
    stat = tacit_bin.stat()
    digest = blake3.blake3(tacit_bin.read_bytes()).hexdigest()
    mtime = datetime.fromtimestamp(stat.st_mtime, tz=UTC)
    return {
        "tacit_bin_path": str(tacit_bin.resolve()),
        "tacit_bin_mtime": mtime.isoformat(timespec="seconds").replace("+00:00", "Z"),
        "tacit_bin_blake3": digest,
    }


def smoke_check_compile(tacit_bin: Path) -> list[PreflightFailure]:
    failures: list[PreflightFailure] = []
    with tempfile.TemporaryDirectory() as td:
        tdp = Path(td)
        for primitive, source in SMOKE_PROGRAMS.items():
            stem = primitive.lstrip("@").replace("-", "_")
            src_path = tdp / f"{stem}.tac"
            src_path.write_text(source, encoding="utf-8")

            check = subprocess.run(
                [str(tacit_bin), "check", str(src_path)],
                capture_output=True, text=True, timeout=30,
            )
            if check.returncode != 0:
                failures.append(PreflightFailure(
                    f"smoke {primitive} check",
                    (check.stderr or check.stdout or "").strip(),
                ))
                continue

            binary = tdp / f"{stem}.bin"
            comp = subprocess.run(
                [str(tacit_bin), "compile", str(src_path), "-o", str(binary)],
                capture_output=True, text=True, timeout=30,
            )
            if comp.returncode != 0:
                failures.append(PreflightFailure(
                    f"smoke {primitive} compile",
                    (comp.stderr or comp.stdout or "").strip(),
                ))
                continue

            run = subprocess.run(
                [str(binary)],
                capture_output=True, text=True, timeout=30,
            )
            if run.returncode != 0:
                failures.append(PreflightFailure(
                    f"smoke {primitive} run",
                    f"exit={run.returncode} stderr={run.stderr.strip()!r}",
                ))
    return failures


def _load_tests(task_dir: Path) -> list[dict]:
    with (task_dir / "tests.jsonl").open(encoding="utf-8") as f:
        return [json.loads(line) for line in f if line.strip()]


def run_canary(
    tacit_bin: Path,
) -> tuple[int, int, list[PreflightFailure]]:
    passed = 0
    total = 0
    failures: list[PreflightFailure] = []
    with tempfile.TemporaryDirectory() as td:
        tdp = Path(td)
        for task_id in CANARY_TASK_IDS:
            task_dir = OPEN_TASKS_ROOT / task_id
            ref = task_dir / "reference.stdlib.tac"
            if not ref.is_file():
                failures.append(PreflightFailure(
                    f"canary {task_id}",
                    f"missing {ref.relative_to(REPO_ROOT)}",
                ))
                continue

            check = subprocess.run(
                [str(tacit_bin), "check", str(ref)],
                capture_output=True, text=True, timeout=60,
            )
            if check.returncode != 0:
                failures.append(PreflightFailure(
                    f"canary {task_id} check",
                    (check.stderr or check.stdout or "").strip(),
                ))
                continue

            binary = tdp / f"{task_id.replace('/', '_')}.bin"
            comp = subprocess.run(
                [str(tacit_bin), "compile", str(ref), "-o", str(binary)],
                capture_output=True, text=True, timeout=60,
            )
            if comp.returncode != 0:
                failures.append(PreflightFailure(
                    f"canary {task_id} compile",
                    (comp.stderr or comp.stdout or "").strip(),
                ))
                continue

            for t in _load_tests(task_dir):
                total += 1
                proc = subprocess.run(
                    [str(binary)],
                    input=t["stdin"],
                    capture_output=True, text=True, timeout=60,
                )
                if proc.returncode == 0 and proc.stdout == t["stdout"]:
                    passed += 1
                else:
                    detail = f"test={t['name']} exit={proc.returncode}"
                    if proc.stdout != t["stdout"]:
                        detail += " stdout-mismatch"
                    failures.append(PreflightFailure(
                        f"canary {task_id}", detail,
                    ))
    return passed, total, failures


def report_token_totals(enc: tiktoken.Encoding) -> TokenTotals:
    py_total = tac_total = stdlib_total = paired = 0
    for task_id in CANARY_TASK_IDS:
        task_dir = OPEN_TASKS_ROOT / task_id
        py_path = task_dir / "reference.py"
        tac_path = task_dir / "reference.tac"
        stdlib_path = task_dir / "reference.stdlib.tac"
        if py_path.is_file():
            py_total += len(enc.encode(py_path.read_text()))
        tac_n: int | None = None
        stdlib_n: int | None = None
        if tac_path.is_file():
            tac_n = len(enc.encode(tac_path.read_text()))
            tac_total += tac_n
        if stdlib_path.is_file():
            stdlib_n = len(enc.encode(stdlib_path.read_text()))
            stdlib_total += stdlib_n
        if tac_n is not None and stdlib_n is not None:
            paired += 1
    return TokenTotals(
        python=py_total,
        tacit=tac_total,
        stdlib_tacit=stdlib_total,
        paired_count=paired,
    )


def _delta(num: int, base: int) -> float | None:
    return None if base == 0 else (num - base) / base


def _fmt_delta(value: float | None) -> str:
    return "n/a" if value is None else f"{value * 100:+.1f}%"


def build_payload(tacit_bin: Path) -> dict[str, Any]:
    meta = binary_metadata(tacit_bin)
    smoke_failures = smoke_check_compile(tacit_bin)
    canary_passed, canary_total, canary_failures = run_canary(tacit_bin)
    enc = tiktoken.get_encoding("o200k_base")
    totals = report_token_totals(enc)
    return {
        "binary": meta,
        "smoke": {
            "primitives": list(SMOKE_PROGRAMS.keys()),
            "failures": [
                {"section": f.section, "detail": f.detail} for f in smoke_failures
            ],
        },
        "canary": {
            "tasks": list(CANARY_TASK_IDS),
            "tests_passed": canary_passed,
            "tests_total": canary_total,
            "failures": [
                {"section": f.section, "detail": f.detail} for f in canary_failures
            ],
        },
        "tokens": {
            "tasks": list(CANARY_TASK_IDS),
            "python_total": totals.python,
            "tacit_total": totals.tacit,
            "stdlib_tacit_total": totals.stdlib_tacit,
            "paired_count": totals.paired_count,
            "stdlib_vs_tacit_delta": _delta(totals.stdlib_tacit, totals.tacit),
            "stdlib_vs_python_delta": _delta(totals.stdlib_tacit, totals.python),
        },
    }


def _print_human_report(payload: dict[str, Any]) -> None:
    bin_info = payload["binary"]
    print("== tacit binary ==")
    print(f"  path : {bin_info['tacit_bin_path']}")
    print(f"  mtime: {bin_info['tacit_bin_mtime']}")
    print(f"  hash : {bin_info['tacit_bin_blake3']}")
    print()
    print("== stdlib primitive smoke programs ==")
    smoke_failures_by_primitive: dict[str, list[str]] = {}
    for f in payload["smoke"]["failures"]:
        prim = f["section"].split(" ", 2)[1]
        smoke_failures_by_primitive.setdefault(prim, []).append(
            f"{f['section']}: {f['detail']}"
        )
    for primitive in payload["smoke"]["primitives"]:
        ok = primitive not in smoke_failures_by_primitive
        print(f"  {'OK  ' if ok else 'FAIL'} {primitive}")
    for messages in smoke_failures_by_primitive.values():
        for msg in messages:
            print(f"    -> {msg}")
    print()
    canary = payload["canary"]
    print(
        f"== stdlib canary references "
        f"({canary['tests_passed']}/{canary['tests_total']} tests passed) =="
    )
    for f in canary["failures"]:
        print(f"  FAIL {f['section']}: {f['detail']}")
    print()
    tokens = payload["tokens"]
    print(f"== canary token totals (n={tokens['paired_count']} paired) ==")
    print(f"  python      : {tokens['python_total']:>7}")
    print(f"  tacit       : {tokens['tacit_total']:>7}")
    print(f"  stdlib tacit: {tokens['stdlib_tacit_total']:>7}")
    print(f"  stdlib vs tacit : {_fmt_delta(tokens['stdlib_vs_tacit_delta'])}")
    print(f"  stdlib vs python: {_fmt_delta(tokens['stdlib_vs_python_delta'])}")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--tacit-bin",
        type=Path,
        default=_default_tacit_bin(),
        help="path to tacit CLI binary (default: TACIT_BIN, ../../target/debug/tacit, or PATH)",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="emit machine-readable JSON instead of the human-readable report",
    )
    args = parser.parse_args(argv)

    if args.tacit_bin is None or not args.tacit_bin.is_file():
        print(
            "error: tacit binary not found; build it with "
            "`cargo build --features tacit-codegen/llvm19-1,tacit-cli/llvm19-1` "
            "or pass --tacit-bin",
            file=sys.stderr,
        )
        return 2

    payload = build_payload(args.tacit_bin)

    if args.json:
        print(json.dumps(payload, indent=2, sort_keys=True))
    else:
        _print_human_report(payload)

    smoke_failed = bool(payload["smoke"]["failures"])
    canary_failed = bool(payload["canary"]["failures"]) or payload["canary"]["tests_total"] == 0
    return 1 if smoke_failed or canary_failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
