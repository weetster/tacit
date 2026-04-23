"""Per-task token counts for reference solutions.

Uses tiktoken o200k_base per ADR 0001. The aggregate Python count is the
Phase 3 token-count baseline per ADR 0019.

Sealed (held-out) tasks are excluded unless `--include-sealed` is passed.
"""

from __future__ import annotations

import argparse

import tiktoken

from tacit_corpus._paths import iter_task_dirs, rel_to_corpus


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--include-sealed",
        action="store_true",
        help="also count sealed held-out tasks",
    )
    args = parser.parse_args()

    enc = tiktoken.get_encoding("o200k_base")
    rows: list[tuple[str, int, int]] = []
    for task_dir in iter_task_dirs(include_sealed=args.include_sealed):
        rel = rel_to_corpus(task_dir)
        py = (task_dir / "reference.py").read_text()
        rs = (task_dir / "reference.rs").read_text()
        rows.append((rel, len(enc.encode(py)), len(enc.encode(rs))))

    print(f"{'task':<44} {'py':>6} {'rs':>6}")
    print("-" * 58)
    py_total = rs_total = 0
    for task, py_n, rs_n in rows:
        print(f"{task:<44} {py_n:>6} {rs_n:>6}")
        py_total += py_n
        rs_total += rs_n
    print("-" * 58)
    scope = "open+sealed" if args.include_sealed else "open"
    print(f"{'TOTAL (' + scope + ')':<44} {py_total:>6} {rs_total:>6}")


if __name__ == "__main__":
    main()
