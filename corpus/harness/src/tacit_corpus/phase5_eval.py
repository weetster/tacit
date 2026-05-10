"""Phase 5 benchmark adapter for model-in-the-loop maintenance evaluation.

This runner reuses the Phase 3 model-call and repair-loop helpers, but grades
against the open Phase 5 benchmark under plans/phase-5-benchmark/ instead of
the corpus task layout.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import tempfile
import time
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any, Literal

import tiktoken

from tacit_corpus._paths import REPO_ROOT
from tacit_corpus import eval as corpus_eval

TaskClass = Literal["repair", "edit", "explanation"]
EvaluationKind = Literal["program", "explanation"]
FailureStage = Literal["api", "extract", "typecheck", "compile", "run", "explanation"]

DEFAULT_OUTPUT_DIR = REPO_ROOT / "plans" / "phase-5-results"
DEFAULT_MANIFEST_PATH = REPO_ROOT / "plans" / "phase-5-benchmark" / "manifest.json"


@dataclass(frozen=True)
class Phase5Task:
    task_id: str
    task_class: TaskClass
    evaluation_kind: EvaluationKind
    canonical_source: Path
    authoring_input: Path
    prompt: str
    expected_exit: int | None = None
    expected_stdout: str | None = None
    required_substrings: tuple[str, ...] = ()


@dataclass(frozen=True)
class StartingArtifact:
    check_pass: bool
    compile_pass: bool
    check_diagnostics: dict[str, Any] | None
    compile_diagnostics: dict[str, Any] | None
    inspection_text: str | None
    inspection_diagnostics: dict[str, Any] | None
    run_exit: int | None = None
    run_stdout: str | None = None
    run_stderr: str | None = None


@dataclass(frozen=True)
class ProgramGrade:
    typecheck_pass: bool
    compile_pass: bool
    run_pass: bool
    failure_stage: FailureStage | None
    diagnostics: dict[str, Any] | None
    run_exit: int | None = None
    run_stdout: str | None = None
    run_stderr: str | None = None


@dataclass(frozen=True)
class ExplanationGrade:
    passed: bool
    failure_stage: FailureStage | None
    diagnostics: dict[str, Any] | None
    missing_substrings: tuple[str, ...] = ()


@dataclass(frozen=True)
class TurnRecord:
    turn_index: int
    prompt: str
    raw_output: str | None
    retries: int
    generation_tokens: int
    failure_stage: FailureStage | None
    diagnostics: dict[str, Any] | None
    source: str | None = None
    explanation: str | None = None
    typecheck_pass: bool = False
    compile_pass: bool = False
    run_pass: bool = False
    run_exit: int | None = None
    run_stdout: str | None = None
    run_stderr: str | None = None
    explanation_pass: bool = False
    missing_substrings: tuple[str, ...] = ()
    inspection_text: str | None = None


@dataclass(frozen=True)
class TaskResult:
    task_id: str
    task_class: TaskClass
    evaluation_kind: EvaluationKind
    turns: list[TurnRecord]
    starting: StartingArtifact
    passed: bool
    duration_ms: int


def _default_tacit_bin() -> Path | None:
    env = os.environ.get("TACIT_BIN")
    if env:
        return Path(env)
    candidate = REPO_ROOT / "target" / "debug" / "tacit"
    if candidate.is_file():
        return candidate
    return None


def _parse_task_selectors(raw: list[str] | None) -> set[str]:
    selectors: set[str] = set()
    for value in raw or []:
        for part in value.split(","):
            part = part.strip()
            if part:
                selectors.add(part)
    return selectors


def load_manifest(manifest_path: Path, selectors: set[str]) -> list[Phase5Task]:
    data = json.loads(manifest_path.read_text(encoding="utf-8"))
    if data.get("version") != 1:
        raise corpus_eval.EvalError("phase 5 manifest version must be 1")

    base = manifest_path.parent
    tasks: list[Phase5Task] = []
    for raw in data["tasks"]:
        task = Phase5Task(
            task_id=raw["id"],
            task_class=raw["class"],
            evaluation_kind=raw["evaluation_kind"],
            canonical_source=base / raw["canonical_source"],
            authoring_input=base / raw["authoring_input"],
            prompt=raw["prompt"],
            expected_exit=raw.get("expected_exit"),
            expected_stdout=raw.get("expected_stdout"),
            required_substrings=tuple(raw.get("required_substrings", [])),
        )
        if selectors and task.task_id not in selectors:
            continue
        if not task.authoring_input.is_file():
            raise corpus_eval.EvalError(f"missing benchmark input: {task.authoring_input}")
        if not task.canonical_source.is_file():
            raise corpus_eval.EvalError(f"missing benchmark canonical source: {task.canonical_source}")
        tasks.append(task)

    if selectors and not tasks:
        raise corpus_eval.EvalError(f"no phase 5 tasks matched selector(s): {', '.join(sorted(selectors))}")
    return tasks


def build_program_prompt(task: Phase5Task) -> str:
    return (
        f"{task.prompt}\n\n"
        "Return the corrected solution as a single Tacit-Lite program in one fenced\n"
        "block: ```tacit ... ```. Do not include the sidecar. Do not include\n"
        "explanatory prose.\n"
    )


def build_explanation_prompt(task: Phase5Task) -> str:
    return (
        f"{task.prompt}\n\n"
        "Return only the explanation in plain prose. Do not include Tacit code, fenced\n"
        "blocks, or sidecar metadata.\n"
    )


def _run_inspection(tacit_bin: Path, source: Path) -> tuple[str | None, dict[str, Any] | None]:
    proc = subprocess.run(
        [str(tacit_bin), "view", str(source), "--as", "inspection", "--types", "--effects"],
        capture_output=True,
        text=True,
        timeout=30,
    )
    if proc.returncode == 0:
        return proc.stdout, None
    return None, corpus_eval._parse_diagnostic_output(proc.stderr or proc.stdout, "inspection-error")


def _run_binary(binary_path: Path) -> tuple[int, str, str]:
    proc = subprocess.run(
        [str(binary_path)],
        capture_output=True,
        text=True,
        timeout=30,
    )
    return proc.returncode, proc.stdout, proc.stderr


def _behavior_diagnostics(task: Phase5Task, *, exit_code: int, stdout: str, stderr: str) -> dict[str, Any]:
    return corpus_eval.diagnostic_envelope(
        "behavior-mismatch",
        "compiled program did not meet the benchmark pass condition",
        expected={
            "exit": task.expected_exit,
            "stdout": task.expected_stdout,
        },
        actual={
            "exit": exit_code,
            "stdout": stdout,
            "stderr": stderr,
        },
    )


def capture_starting_artifacts(task: Phase5Task, tacit_bin: Path) -> StartingArtifact:
    check_pass, check_diags = corpus_eval._run_tacit_check(tacit_bin, task.authoring_input)
    inspection_text, inspection_diags = _run_inspection(tacit_bin, task.authoring_input)
    compile_pass, compile_diags = corpus_eval._run_tacit_compile(
        tacit_bin,
        task.authoring_input,
        Path(tempfile.mkdtemp()) / "starting-bin",
    )
    run_exit = run_stdout = run_stderr = None
    if compile_pass and task.evaluation_kind == "program":
        with tempfile.TemporaryDirectory() as td:
            binary = Path(td) / "starting"
            compile_pass, compile_diags = corpus_eval._run_tacit_compile(
                tacit_bin, task.authoring_input, binary
            )
            if compile_pass:
                run_exit, run_stdout, run_stderr = _run_binary(binary)
    return StartingArtifact(
        check_pass=check_pass,
        compile_pass=compile_pass,
        check_diagnostics=check_diags,
        compile_diagnostics=compile_diags,
        inspection_text=inspection_text,
        inspection_diagnostics=inspection_diags,
        run_exit=run_exit,
        run_stdout=run_stdout,
        run_stderr=run_stderr,
    )


def grade_program_source(task: Phase5Task, tacit_bin: Path, source_path: Path) -> tuple[ProgramGrade, str | None]:
    inspection_text, _ = _run_inspection(tacit_bin, source_path)
    typecheck_pass, check_diags = corpus_eval._run_tacit_check(tacit_bin, source_path)
    if not typecheck_pass:
        return (
            ProgramGrade(
                typecheck_pass=False,
                compile_pass=False,
                run_pass=False,
                failure_stage="typecheck",
                diagnostics=check_diags,
            ),
            inspection_text,
        )

    with tempfile.TemporaryDirectory() as td:
        binary = Path(td) / "generated"
        compile_pass, compile_diags = corpus_eval._run_tacit_compile(tacit_bin, source_path, binary)
        if not compile_pass:
            return (
                ProgramGrade(
                    typecheck_pass=True,
                    compile_pass=False,
                    run_pass=False,
                    failure_stage="compile",
                    diagnostics=compile_diags,
                ),
                inspection_text,
            )

        exit_code, stdout, stderr = _run_binary(binary)
        run_pass = exit_code == task.expected_exit and stdout == (task.expected_stdout or "")
        diagnostics = None if run_pass else _behavior_diagnostics(
            task, exit_code=exit_code, stdout=stdout, stderr=stderr
        )
        return (
            ProgramGrade(
                typecheck_pass=True,
                compile_pass=True,
                run_pass=run_pass,
                failure_stage=None if run_pass else "run",
                diagnostics=diagnostics,
                run_exit=exit_code,
                run_stdout=stdout,
                run_stderr=stderr,
            ),
            inspection_text,
        )


def grade_explanation(task: Phase5Task, text: str) -> ExplanationGrade:
    lowered = text.lower()
    missing = tuple(term for term in task.required_substrings if term.lower() not in lowered)
    diagnostics = None
    if missing:
        diagnostics = corpus_eval.diagnostic_envelope(
            "explanation-mismatch",
            "explanation omitted required benchmark concepts",
            expected={"required_substrings": list(task.required_substrings)},
            actual={"missing_substrings": list(missing)},
        )
    return ExplanationGrade(
        passed=not missing,
        failure_stage=None if not missing else "explanation",
        diagnostics=diagnostics,
        missing_substrings=missing,
    )


def build_program_repair_prompt(task: Phase5Task, previous: TurnRecord) -> str:
    previous_source = previous.source.rstrip() if previous.source else ""
    feedback_parts: list[str] = []
    if previous.diagnostics is not None:
        feedback_parts.append(json.dumps(previous.diagnostics, indent=2, sort_keys=True))
    if previous.inspection_text:
        feedback_parts.append(f"Inspection view:\n{previous.inspection_text}")
    feedback = "\n\n".join(feedback_parts) if feedback_parts else "No additional feedback."
    return (
        "The previous Tacit-Lite program failed.\n\n"
        f"Task:\n{task.prompt}\n\n"
        "Previous program:\n"
        "```tacit\n"
        f"{previous_source}\n"
        "```\n\n"
        f"Failure stage: {previous.failure_stage}\n\n"
        "Feedback:\n"
        f"{feedback}\n\n"
        "Return a corrected solution as a single Tacit-Lite program in one fenced\n"
        "block: ```tacit ... ```. Do not include the sidecar. Do not include\n"
        "explanatory prose.\n"
    )


def build_explanation_repair_prompt(task: Phase5Task, previous: TurnRecord, starting: StartingArtifact) -> str:
    feedback_parts: list[str] = []
    if starting.check_diagnostics is not None:
        feedback_parts.append(json.dumps(starting.check_diagnostics, indent=2, sort_keys=True))
    if starting.inspection_text:
        feedback_parts.append(f"Inspection view:\n{starting.inspection_text}")
    if previous.missing_substrings:
        feedback_parts.append(
            "Missing required concepts: " + ", ".join(previous.missing_substrings)
        )
    feedback = "\n\n".join(feedback_parts)
    return (
        "The previous explanation was incomplete.\n\n"
        f"Task:\n{task.prompt}\n\n"
        "Previous explanation:\n"
        f"{(previous.explanation or '').strip()}\n\n"
        "Feedback:\n"
        f"{feedback}\n\n"
        "Return only the corrected explanation in plain prose. Do not include Tacit\n"
        "code or fenced blocks.\n"
    )


def _write_json(path: Path, data: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _capture_starting_artifacts(run_dir: Path, starting: StartingArtifact, task: Phase5Task) -> None:
    start_dir = run_dir / task.task_id / "starting"
    start_dir.mkdir(parents=True, exist_ok=True)
    shutil_files = {
        "input.tac": task.canonical_source,
        "input.taca": task.authoring_input,
    }
    for name, src in shutil_files.items():
        (start_dir / name).write_text(src.read_text(encoding="utf-8"), encoding="utf-8")
    _write_json(start_dir / "check.json", {
        "passed": starting.check_pass,
        "diagnostics": starting.check_diagnostics,
    })
    _write_json(start_dir / "compile.json", {
        "passed": starting.compile_pass,
        "diagnostics": starting.compile_diagnostics,
        "run_exit": starting.run_exit,
        "run_stdout": starting.run_stdout,
        "run_stderr": starting.run_stderr,
    })
    if starting.inspection_text is not None:
        (start_dir / "inspection.txt").write_text(starting.inspection_text, encoding="utf-8")
    if starting.inspection_diagnostics is not None:
        _write_json(start_dir / "inspection.json", starting.inspection_diagnostics)


def _capture_turn(run_dir: Path, task: Phase5Task, turn: TurnRecord) -> None:
    turn_dir = run_dir / task.task_id / f"turn-{turn.turn_index}"
    turn_dir.mkdir(parents=True, exist_ok=True)
    (turn_dir / "prompt.txt").write_text(turn.prompt, encoding="utf-8")
    if turn.raw_output is not None:
        (turn_dir / "raw-response.txt").write_text(turn.raw_output, encoding="utf-8")
    if turn.source is not None:
        (turn_dir / "generated.taca").write_text(turn.source, encoding="utf-8")
    if turn.explanation is not None:
        (turn_dir / "explanation.txt").write_text(turn.explanation, encoding="utf-8")
    if turn.inspection_text is not None:
        (turn_dir / "inspection.txt").write_text(turn.inspection_text, encoding="utf-8")
    _write_json(turn_dir / "result.json", asdict(turn))


def _request_model_response(
    *,
    provider: corpus_eval.Provider,
    api_key: str | None,
    model: str,
    primer: str,
    prompt: str,
    max_tokens: int,
    temperature: float,
    timeout_seconds: int,
    max_retries: int,
) -> corpus_eval.ModelOutcome:
    if api_key is None:
        raise corpus_eval.EvalError("phase5-eval requires a model API key; no dry-run mode is implemented")
    return corpus_eval.call_model_with_retries(
        provider=provider,
        api_key=api_key,
        model=model,
        primer=primer,
        prompt=prompt,
        max_tokens=max_tokens,
        temperature=temperature,
        timeout_seconds=timeout_seconds,
        max_retries=max_retries,
    )


def evaluate_program_task(
    *,
    task: Phase5Task,
    primer: str,
    enc: tiktoken.Encoding,
    provider: corpus_eval.Provider,
    api_key: str | None,
    model: str,
    tacit_bin: Path,
    run_dir: Path,
    max_tokens: int,
    temperature: float,
    timeout_seconds: int,
    max_retries: int,
    repair_turns: int,
) -> TaskResult:
    started = time.monotonic()
    starting = capture_starting_artifacts(task, tacit_bin)
    _capture_starting_artifacts(run_dir, starting, task)
    prompt = build_program_prompt(task)
    turns: list[TurnRecord] = []

    for turn_index in range(repair_turns + 1):
        outcome = _request_model_response(
            provider=provider,
            api_key=api_key,
            model=model,
            primer=primer,
            prompt=prompt,
            max_tokens=max_tokens,
            temperature=temperature,
            timeout_seconds=timeout_seconds,
            max_retries=max_retries,
        )
        if outcome.response is None:
            turn = TurnRecord(
                turn_index=turn_index,
                prompt=prompt,
                raw_output=None,
                retries=outcome.retries,
                generation_tokens=0,
                failure_stage="api",
                diagnostics=outcome.diagnostics,
            )
            turns.append(turn)
            _capture_turn(run_dir, task, turn)
            break

        extraction = corpus_eval.extract_tacit_source(outcome.response)
        if extraction.source is None:
            turn = TurnRecord(
                turn_index=turn_index,
                prompt=prompt,
                raw_output=outcome.response.text,
                retries=outcome.retries,
                generation_tokens=0,
                failure_stage="extract",
                diagnostics=extraction.diagnostics,
            )
            turns.append(turn)
            _capture_turn(run_dir, task, turn)
        else:
            generation_tokens = len(enc.encode(extraction.source))
            with tempfile.TemporaryDirectory() as td:
                source_path = Path(td) / "generated.taca"
                source_path.write_text(extraction.source, encoding="utf-8")
                grade, inspection_text = grade_program_source(task, tacit_bin, source_path)
            turn = TurnRecord(
                turn_index=turn_index,
                prompt=prompt,
                raw_output=outcome.response.text,
                retries=outcome.retries,
                generation_tokens=generation_tokens,
                failure_stage=grade.failure_stage,
                diagnostics=grade.diagnostics,
                source=extraction.source,
                typecheck_pass=grade.typecheck_pass,
                compile_pass=grade.compile_pass,
                run_pass=grade.run_pass,
                run_exit=grade.run_exit,
                run_stdout=grade.run_stdout,
                run_stderr=grade.run_stderr,
                inspection_text=inspection_text,
            )
            turns.append(turn)
            _capture_turn(run_dir, task, turn)
            if grade.run_pass:
                break

        if turn_index == repair_turns:
            break
        prompt = build_program_repair_prompt(task, turns[-1])

    passed = bool(turns and turns[-1].run_pass)
    return TaskResult(
        task_id=task.task_id,
        task_class=task.task_class,
        evaluation_kind=task.evaluation_kind,
        turns=turns,
        starting=starting,
        passed=passed,
        duration_ms=int((time.monotonic() - started) * 1000),
    )


def evaluate_explanation_task(
    *,
    task: Phase5Task,
    primer: str,
    enc: tiktoken.Encoding,
    provider: corpus_eval.Provider,
    api_key: str | None,
    model: str,
    tacit_bin: Path,
    run_dir: Path,
    max_tokens: int,
    temperature: float,
    timeout_seconds: int,
    max_retries: int,
    repair_turns: int,
) -> TaskResult:
    started = time.monotonic()
    starting = capture_starting_artifacts(task, tacit_bin)
    _capture_starting_artifacts(run_dir, starting, task)
    prompt = build_explanation_prompt(task)
    turns: list[TurnRecord] = []

    for turn_index in range(repair_turns + 1):
        outcome = _request_model_response(
            provider=provider,
            api_key=api_key,
            model=model,
            primer=primer,
            prompt=prompt,
            max_tokens=max_tokens,
            temperature=temperature,
            timeout_seconds=timeout_seconds,
            max_retries=max_retries,
        )
        if outcome.response is None:
            turn = TurnRecord(
                turn_index=turn_index,
                prompt=prompt,
                raw_output=None,
                retries=outcome.retries,
                generation_tokens=0,
                failure_stage="api",
                diagnostics=outcome.diagnostics,
            )
            turns.append(turn)
            _capture_turn(run_dir, task, turn)
            break

        explanation = outcome.response.text.strip()
        grade = grade_explanation(task, explanation)
        turn = TurnRecord(
            turn_index=turn_index,
            prompt=prompt,
            raw_output=outcome.response.text,
            retries=outcome.retries,
            generation_tokens=len(enc.encode(explanation)),
            failure_stage=grade.failure_stage,
            diagnostics=grade.diagnostics,
            explanation=explanation,
            explanation_pass=grade.passed,
            missing_substrings=grade.missing_substrings,
            inspection_text=starting.inspection_text,
        )
        turns.append(turn)
        _capture_turn(run_dir, task, turn)
        if grade.passed or turn_index == repair_turns:
            break
        prompt = build_explanation_repair_prompt(task, turn, starting)

    passed = bool(turns and turns[-1].explanation_pass)
    return TaskResult(
        task_id=task.task_id,
        task_class=task.task_class,
        evaluation_kind=task.evaluation_kind,
        turns=turns,
        starting=starting,
        passed=passed,
        duration_ms=int((time.monotonic() - started) * 1000),
    )


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", required=True, help="model id to evaluate")
    parser.add_argument(
        "--provider",
        choices=["auto", "anthropic", "openrouter"],
        default="auto",
        help="model provider (default: infer from model id)",
    )
    parser.add_argument(
        "--tasks",
        action="append",
        help="task selector(s): r1-record-total; comma-separated allowed",
    )
    parser.add_argument(
        "--manifest",
        type=Path,
        default=DEFAULT_MANIFEST_PATH,
        help="phase 5 benchmark manifest path",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=DEFAULT_OUTPUT_DIR,
        help="directory for phase 5 run artifacts",
    )
    parser.add_argument(
        "--tacit-bin",
        type=Path,
        default=_default_tacit_bin(),
        help="path to tacit CLI binary",
    )
    parser.add_argument(
        "--repair-turns",
        type=int,
        default=2,
        help="number of repair turns after the initial generation (0-2; default: 2)",
    )
    parser.add_argument("--max-tokens", type=int, default=corpus_eval.DEFAULT_MAX_TOKENS)
    parser.add_argument("--temperature", type=float, default=corpus_eval.DEFAULT_TEMPERATURE)
    parser.add_argument(
        "--timeout-seconds", type=int, default=corpus_eval.DEFAULT_TIMEOUT_SECONDS
    )
    parser.add_argument("--max-retries", type=int, default=corpus_eval.DEFAULT_MAX_RETRIES)
    return parser.parse_args(argv)


def run(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    if args.repair_turns < 0 or args.repair_turns > 2:
        raise corpus_eval.EvalError("--repair-turns must be between 0 and 2")
    if args.tacit_bin is None or not args.tacit_bin.is_file():
        raise corpus_eval.EvalError("tacit binary not found; build it or pass --tacit-bin")

    provider = corpus_eval.infer_provider(args.model, args.provider)
    api_key = corpus_eval._require_api_key(provider)
    enc = tiktoken.get_encoding("o200k_base")
    primer, primer_hash, primer_tokens = corpus_eval.load_primer(enc)
    selectors = _parse_task_selectors(args.tasks)
    tasks = load_manifest(args.manifest, selectors)
    run_id = corpus_eval._uuid7()
    started_at = corpus_eval._utc_now()
    run_dir = args.output_dir / run_id
    run_dir.mkdir(parents=True, exist_ok=True)

    print(
        f"phase5-eval run_id={run_id} provider={provider} model={args.model} "
        f"tasks={len(tasks)} repair_turns={args.repair_turns}"
    )

    results: list[TaskResult] = []
    for task in tasks:
        if task.evaluation_kind == "program":
            result = evaluate_program_task(
                task=task,
                primer=primer,
                enc=enc,
                provider=provider,
                api_key=api_key,
                model=args.model,
                tacit_bin=args.tacit_bin,
                run_dir=run_dir,
                max_tokens=args.max_tokens,
                temperature=args.temperature,
                timeout_seconds=args.timeout_seconds,
                max_retries=args.max_retries,
                repair_turns=args.repair_turns,
            )
        else:
            result = evaluate_explanation_task(
                task=task,
                primer=primer,
                enc=enc,
                provider=provider,
                api_key=api_key,
                model=args.model,
                tacit_bin=args.tacit_bin,
                run_dir=run_dir,
                max_tokens=args.max_tokens,
                temperature=args.temperature,
                timeout_seconds=args.timeout_seconds,
                max_retries=args.max_retries,
                repair_turns=args.repair_turns,
            )
        results.append(result)
        print("." if result.passed else "F", end="", flush=True)
    print()

    completed_at = corpus_eval._utc_now()
    summary = {
        "run_id": run_id,
        "started_at": corpus_eval._iso_z(started_at),
        "completed_at": corpus_eval._iso_z(completed_at),
        "model": {
            "provider": provider,
            "id": args.model,
            "primer_hash": primer_hash,
            "primer_tokens": primer_tokens,
        },
        "manifest": str(args.manifest.relative_to(REPO_ROOT)),
        "repair_turns": args.repair_turns,
        "task_count": len(results),
        "passed_count": sum(1 for result in results if result.passed),
        "tasks": [asdict(result) for result in results],
        "tacit_binary": corpus_eval._binary_metadata(args.tacit_bin),
    }
    _write_json(run_dir / "run.json", summary)
    print(f"wrote {run_dir / 'run.json'}")
    return 0


def main() -> None:
    try:
        raise SystemExit(run())
    except corpus_eval.EvalError as exc:
        print(f"error: {exc}")
        raise SystemExit(2)


if __name__ == "__main__":
    main()
