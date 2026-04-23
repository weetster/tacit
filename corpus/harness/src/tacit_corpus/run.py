"""Run reference solutions against each task's tests.jsonl.

Compiles each Rust reference once per task and runs both Python and Rust
references against every test case. Stdout is compared byte-for-byte against
the test's expected `stdout`; exit code must be zero.

Sealed (held-out) tasks are excluded unless `--include-sealed` is passed.
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

from tacit_corpus._paths import iter_task_dirs, rel_to_corpus


def _find_rustc() -> str:
    found = shutil.which("rustc")
    if found:
        return found
    fallback = Path.home() / ".cargo" / "bin" / "rustc"
    if fallback.is_file():
        return str(fallback)
    print("error: rustc not found on PATH or in ~/.cargo/bin", file=sys.stderr)
    sys.exit(2)


@dataclass
class TestResult:
    task: str
    language: str
    test_name: str
    passed: bool
    message: str = ""


def load_tests(task_dir: Path) -> list[dict]:
    with (task_dir / "tests.jsonl").open() as f:
        return [json.loads(line) for line in f if line.strip()]


def _compare(proc: subprocess.CompletedProcess[str], test: dict) -> tuple[bool, str]:
    if proc.returncode != 0:
        return False, f"nonzero exit {proc.returncode}; stderr={proc.stderr!r}"
    if proc.stdout != test["stdout"]:
        return False, f"stdout mismatch\n  expected {test['stdout']!r}\n  got      {proc.stdout!r}"
    return True, ""


def run_python(task_dir: Path, tests: list[dict]) -> list[TestResult]:
    ref = task_dir / "reference.py"
    rel = rel_to_corpus(task_dir)
    out: list[TestResult] = []
    for t in tests:
        proc = subprocess.run(
            [sys.executable, str(ref)],
            input=t["stdin"],
            capture_output=True,
            text=True,
            timeout=30,
        )
        passed, msg = _compare(proc, t)
        out.append(TestResult(rel, "py", t["name"], passed, msg))
    return out


def run_rust(task_dir: Path, tests: list[dict]) -> list[TestResult]:
    ref = task_dir / "reference.rs"
    rel = rel_to_corpus(task_dir)
    rustc = _find_rustc()
    with tempfile.TemporaryDirectory() as td:
        binary = Path(td) / "ref"
        compile_proc = subprocess.run(
            [rustc, "--edition", "2024", "-O", str(ref), "-o", str(binary)],
            capture_output=True,
            text=True,
        )
        if compile_proc.returncode != 0:
            msg = f"compile error: {compile_proc.stderr}"
            return [TestResult(rel, "rs", t["name"], False, msg) for t in tests]
        out: list[TestResult] = []
        for t in tests:
            proc = subprocess.run(
                [str(binary)],
                input=t["stdin"],
                capture_output=True,
                text=True,
                timeout=30,
            )
            passed, msg = _compare(proc, t)
            out.append(TestResult(rel, "rs", t["name"], passed, msg))
        return out


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--include-sealed",
        action="store_true",
        help="also run sealed held-out tasks (Phase 3 grading mode)",
    )
    args = parser.parse_args()

    task_dirs = iter_task_dirs(include_sealed=args.include_sealed)
    if not task_dirs:
        print("no tasks found", file=sys.stderr)
        return 2
    total = passed = 0
    failures: list[TestResult] = []
    for task_dir in task_dirs:
        tests = load_tests(task_dir)
        for runner in (run_python, run_rust):
            for r in runner(task_dir, tests):
                total += 1
                if r.passed:
                    passed += 1
                    print(".", end="", flush=True)
                else:
                    print("F", end="", flush=True)
                    failures.append(r)
    print()
    for f in failures:
        print(f"FAIL {f.task} [{f.language}] test={f.test_name}")
        for line in f.message.splitlines():
            print(f"     {line}")
    scope = "open+sealed" if args.include_sealed else "open"
    print(f"{passed}/{total} passed across {len(task_dirs)} tasks ({scope})")
    return 0 if passed == total else 1


if __name__ == "__main__":
    raise SystemExit(main())
