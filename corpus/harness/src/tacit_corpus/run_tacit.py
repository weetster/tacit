"""Run Tacit-Lite reference solutions against each task's tests.jsonl.

Compiles each open-task reference.tac once, after typechecking it, and runs the
compiled binary against every test case. Stdout is compared byte-for-byte
against the expected stdout; exit code must be zero.

Only open tasks under corpus/tasks are considered. Sealed held-out tasks do not
ship Tacit references and this command deliberately has no sealed mode.
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
from pathlib import Path

from tacit_corpus._paths import REPO_ROOT, iter_task_dirs, rel_to_corpus


@dataclass
class TestResult:
    task: str
    test_name: str
    passed: bool
    message: str = ""


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


def load_tests(task_dir: Path) -> list[dict]:
    with (task_dir / "tests.jsonl").open(encoding="utf-8") as f:
        return [json.loads(line) for line in f if line.strip()]


def _compare(proc: subprocess.CompletedProcess[str], test: dict) -> tuple[bool, str]:
    if proc.returncode != 0:
        return False, f"nonzero exit {proc.returncode}; stderr={proc.stderr!r}"
    if proc.stdout != test["stdout"]:
        return False, f"stdout mismatch\n  expected {test['stdout']!r}\n  got      {proc.stdout!r}"
    return True, ""


def _compile_reference(tacit_bin: Path, task_dir: Path, binary: Path) -> str | None:
    ref = task_dir / "reference.tac"
    check_proc = subprocess.run(
        [str(tacit_bin), "check", str(ref)],
        capture_output=True,
        text=True,
        timeout=30,
    )
    if check_proc.returncode != 0:
        return f"check error: {check_proc.stderr or check_proc.stdout}"

    compile_proc = subprocess.run(
        [str(tacit_bin), "compile", str(ref), "-o", str(binary)],
        capture_output=True,
        text=True,
        timeout=30,
    )
    if compile_proc.returncode != 0:
        return f"compile error: {compile_proc.stderr or compile_proc.stdout}"
    return None


def run_tacit(tacit_bin: Path, task_dir: Path, tests: list[dict]) -> list[TestResult]:
    rel = rel_to_corpus(task_dir)
    with tempfile.TemporaryDirectory() as td:
        binary = Path(td) / "reference"
        compile_error = _compile_reference(tacit_bin, task_dir, binary)
        if compile_error is not None:
            return [TestResult(rel, t["name"], False, compile_error) for t in tests]

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
            out.append(TestResult(rel, t["name"], passed, msg))
        return out


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--tacit-bin",
        type=Path,
        default=_default_tacit_bin(),
        help="path to tacit CLI binary (default: TACIT_BIN, ../../target/debug/tacit, or PATH)",
    )
    args = parser.parse_args()

    if args.tacit_bin is None or not args.tacit_bin.is_file():
        print(
            "error: tacit binary not found; build it or pass --tacit-bin",
            file=sys.stderr,
        )
        return 2

    task_dirs = [
        task_dir
        for task_dir in iter_task_dirs(include_sealed=False)
        if (task_dir / "reference.tac").is_file()
    ]
    if not task_dirs:
        print("no open Tacit references found", file=sys.stderr)
        return 2

    total = passed = 0
    failures: list[TestResult] = []
    for task_dir in task_dirs:
        tests = load_tests(task_dir)
        for result in run_tacit(args.tacit_bin, task_dir, tests):
            total += 1
            if result.passed:
                passed += 1
                print(".", end="", flush=True)
            else:
                print("F", end="", flush=True)
                failures.append(result)

    print()
    for failure in failures:
        print(f"FAIL {failure.task} [tacit] test={failure.test_name}")
        for line in failure.message.splitlines():
            print(f"     {line}")

    print(f"{passed}/{total} passed across {len(task_dirs)} open Tacit references")
    return 0 if passed == total else 1


if __name__ == "__main__":
    raise SystemExit(main())
