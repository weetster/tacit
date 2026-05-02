"""Per-task token counts for reference solutions.

Uses tiktoken o200k_base per ADR 0001. Reports three aggregates per ADR
0021: full, stdlib-dominated, non-stdlib-dominated. The Python aggregates
are the Phase 3 token-count baseline per ADR 0019.

Sealed (held-out) tasks are excluded unless `--include-sealed` is passed.
"""

from __future__ import annotations

import argparse
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path

import tiktoken

from tacit_corpus._paths import CORPUS_ROOT, iter_task_dirs, rel_to_corpus

DOMINANCE_FILE = CORPUS_ROOT / "stdlib-dominance.toml"
MISSING = "-"
LABEL_WIDTH = 56


@dataclass(frozen=True)
class TokenRow:
    task: str
    stdlib_dominated: bool
    python: int
    tacit: int | None
    stdlib_tacit: int | None
    rust: int


def load_dominance_map() -> dict[str, bool]:
    with DOMINANCE_FILE.open("rb") as fh:
        data = tomllib.load(fh)
    return dict(data["stdlib_dominated"])


def task_key(task_dir: Path) -> str:
    return f"{task_dir.parent.name}/{task_dir.name}"


def delta(base_n: int | None, candidate_n: int | None) -> str:
    if base_n is None or candidate_n is None:
        return MISSING
    if base_n == 0:
        return "n/a"
    pct = ((candidate_n - base_n) / base_n) * 100
    return f"{pct:+.0f}%"


def fmt_count(value: int | None) -> str:
    return MISSING if value is None else str(value)


def build_rows(
    task_dirs: list[Path],
    dominance: dict[str, bool],
    enc: tiktoken.Encoding,
) -> list[TokenRow]:
    rows: list[TokenRow] = []
    for task_dir in task_dirs:
        rel = rel_to_corpus(task_dir)
        py = (task_dir / "reference.py").read_text()
        rs = (task_dir / "reference.rs").read_text()
        tacit_path = task_dir / "reference.tac"
        stdlib_tacit_path = task_dir / "reference.stdlib.tac"
        tacit = tacit_path.read_text() if tacit_path.is_file() else None
        stdlib_tacit = (
            stdlib_tacit_path.read_text() if stdlib_tacit_path.is_file() else None
        )
        rows.append(
            TokenRow(
                task=rel,
                stdlib_dominated=dominance[task_key(task_dir)],
                python=len(enc.encode(py)),
                tacit=len(enc.encode(tacit)) if tacit is not None else None,
                stdlib_tacit=(
                    len(enc.encode(stdlib_tacit)) if stdlib_tacit is not None else None
                ),
                rust=len(enc.encode(rs)),
            )
        )
    return rows


def _total_line(
    label: str,
    *,
    python: int,
    tacit: int | None = None,
    stdlib_tacit: int | None = None,
    rust: int | None = None,
) -> str:
    return (
        f"{label:<{LABEL_WIDTH}} {'':>3} {python:>6} "
        f"{fmt_count(tacit):>6} {fmt_count(stdlib_tacit):>7} "
        f"{fmt_count(rust):>6} {delta(python, tacit):>7} "
        f"{delta(python, stdlib_tacit):>7} {delta(tacit, stdlib_tacit):>8}"
    )


def _scope_rows(rows: list[TokenRow], *, dominated: bool | None = None) -> list[TokenRow]:
    if dominated is None:
        return rows
    return [row for row in rows if row.stdlib_dominated is dominated]


def render_report(rows: list[TokenRow], scope: str) -> str:
    lines = [
        f"{'task':<{LABEL_WIDTH}} {'dom':>3} {'py':>6} {'tacit':>6} "
        f"{'stdlib':>7} {'rs':>6} {'tacΔ':>7} {'stdΔ':>7} {'std/tac':>8}",
        "-" * 118,
    ]

    for row in rows:
        flag = "D" if row.stdlib_dominated else "-"
        lines.append(
            f"{row.task:<{LABEL_WIDTH}} {flag:>3} {row.python:>6} "
            f"{fmt_count(row.tacit):>6} {fmt_count(row.stdlib_tacit):>7} "
            f"{row.rust:>6} {delta(row.python, row.tacit):>7} "
            f"{delta(row.python, row.stdlib_tacit):>7} "
            f"{delta(row.tacit, row.stdlib_tacit):>8}"
        )

    lines.append("-" * 118)

    scopes = [
        ("all", None),
        ("stdlib-dominated", True),
        ("non-stdlib-dominated", False),
    ]

    for label_suffix, dominated in scopes:
        bucket = _scope_rows(rows, dominated=dominated)
        lines.append(
            _total_line(
                f"TOTAL ({scope}, Python/Rust {label_suffix})",
                python=sum(row.python for row in bucket),
                rust=sum(row.rust for row in bucket),
            )
        )

    for label_suffix, dominated in scopes:
        bucket = [
            row for row in _scope_rows(rows, dominated=dominated) if row.tacit is not None
        ]
        lines.append(
            _total_line(
                f"TOTAL ({scope}, Tacit refs {label_suffix}; n={len(bucket)})",
                python=sum(row.python for row in bucket),
                tacit=sum(row.tacit for row in bucket if row.tacit is not None),
            )
        )

    for label_suffix, dominated in scopes:
        bucket = [
            row
            for row in _scope_rows(rows, dominated=dominated)
            if row.stdlib_tacit is not None
        ]
        lines.append(
            _total_line(
                f"TOTAL ({scope}, Stdlib Tacit refs {label_suffix}; n={len(bucket)})",
                python=sum(row.python for row in bucket),
                stdlib_tacit=sum(
                    row.stdlib_tacit for row in bucket if row.stdlib_tacit is not None
                ),
            )
        )

    for label_suffix, dominated in scopes:
        bucket = [
            row
            for row in _scope_rows(rows, dominated=dominated)
            if row.tacit is not None and row.stdlib_tacit is not None
        ]
        lines.append(
            _total_line(
                f"TOTAL ({scope}, Stdlib vs Tacit paired {label_suffix}; n={len(bucket)})",
                python=sum(row.python for row in bucket),
                tacit=sum(row.tacit for row in bucket if row.tacit is not None),
                stdlib_tacit=sum(
                    row.stdlib_tacit for row in bucket if row.stdlib_tacit is not None
                ),
            )
        )

    return "\n".join(lines) + "\n"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--include-sealed",
        action="store_true",
        help="also count sealed held-out tasks",
    )
    args = parser.parse_args()

    enc = tiktoken.get_encoding("o200k_base")
    dominance = load_dominance_map()

    task_dirs = iter_task_dirs(include_sealed=args.include_sealed)
    missing = [task_key(d) for d in task_dirs if task_key(d) not in dominance]
    if missing:
        print(
            "error: stdlib-dominance.toml is missing entries for:",
            file=sys.stderr,
        )
        for key in missing:
            print(f"  {key}", file=sys.stderr)
        print(
            "\nAdd an entry per ADR 0021 before running corpus-tokens.",
            file=sys.stderr,
        )
        sys.exit(1)

    scope = "open+sealed" if args.include_sealed else "open"
    print(render_report(build_rows(task_dirs, dominance, enc), scope), end="")


if __name__ == "__main__":
    main()
