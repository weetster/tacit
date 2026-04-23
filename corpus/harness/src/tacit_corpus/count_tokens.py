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
from pathlib import Path

import tiktoken

from tacit_corpus._paths import CORPUS_ROOT, iter_task_dirs, rel_to_corpus

DOMINANCE_FILE = CORPUS_ROOT / "stdlib-dominance.toml"


def load_dominance_map() -> dict[str, bool]:
    with DOMINANCE_FILE.open("rb") as fh:
        data = tomllib.load(fh)
    return dict(data["stdlib_dominated"])


def task_key(task_dir: Path) -> str:
    return f"{task_dir.parent.name}/{task_dir.name}"


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

    rows: list[tuple[str, bool, int, int]] = []
    for task_dir in task_dirs:
        rel = rel_to_corpus(task_dir)
        dom = dominance[task_key(task_dir)]
        py = (task_dir / "reference.py").read_text()
        rs = (task_dir / "reference.rs").read_text()
        rows.append((rel, dom, len(enc.encode(py)), len(enc.encode(rs))))

    print(f"{'task':<44} {'dom':>3} {'py':>6} {'rs':>6}")
    print("-" * 64)

    py_all = rs_all = 0
    py_dom = rs_dom = 0
    py_non = rs_non = 0
    for task, dom, py_n, rs_n in rows:
        flag = "D" if dom else "-"
        print(f"{task:<44} {flag:>3} {py_n:>6} {rs_n:>6}")
        py_all += py_n
        rs_all += rs_n
        if dom:
            py_dom += py_n
            rs_dom += rs_n
        else:
            py_non += py_n
            rs_non += rs_n

    scope = "open+sealed" if args.include_sealed else "open"
    print("-" * 64)
    print(f"{'TOTAL (' + scope + ', all)':<48} {py_all:>6} {rs_all:>6}")
    print(f"{'TOTAL (' + scope + ', stdlib-dominated)':<48} {py_dom:>6} {rs_dom:>6}")
    print(f"{'TOTAL (' + scope + ', non-stdlib-dominated)':<48} {py_non:>6} {rs_non:>6}")


if __name__ == "__main__":
    main()
