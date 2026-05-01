"""Phase 3 model-generation evaluation harness.

`corpus-eval` loads the Phase 3 primer plus task statements, asks one model
for a Tacit-Lite program per task, compiles the generated program with the
repo-local `tacit` CLI, runs the task tests, and writes ADR 0055 metrics.

Dry-run mode exercises the same grading and metrics writer using open
`reference.tac` files as synthetic model output, without invoking a paid API.
"""

from __future__ import annotations

import argparse
import email.utils
import json
import os
import random
import re
import shutil
import subprocess
import sys
import tempfile
import time
import tomllib
import urllib.error
import urllib.request
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any, Literal

import blake3
import tiktoken

from tacit_corpus._paths import (
    CORPUS_ROOT,
    OPEN_TASKS_ROOT,
    REPO_ROOT,
    SEALED_ROOT,
)

Provider = Literal["anthropic", "openrouter"]
Track = Literal["primary", "cross-family"]
Scope = Literal["open", "sealed"]

DEFAULT_TEMPERATURE = 0
DEFAULT_MAX_TOKENS = 8192
DEFAULT_TIMEOUT_SECONDS = 120
DEFAULT_MAX_RETRIES = 3
HARNESS_VERSION = "0.1.0"
PROMPT_SUFFIX = (
    "Write the solution as a single Tacit-Lite program in a fenced "
    "block: ```tacit ... ```. Do not include the sidecar."
)
RESULTS_DIR = REPO_ROOT / "plans" / "phase-3-results"
PRIMER_PATH = REPO_ROOT / "plans" / "primer" / "tacit-lite-primer.md"
DOMINANCE_FILE = CORPUS_ROOT / "stdlib-dominance.toml"


@dataclass(frozen=True)
class EvalTask:
    task_id: str
    task_dir: Path
    sealed: bool
    stdlib_dominated: bool
    python_baseline_tokens: int


@dataclass(frozen=True)
class ModelResponse:
    text: str
    stop_reason: str | None


@dataclass(frozen=True)
class ModelOutcome:
    response: ModelResponse | None
    retries: int
    diagnostics: dict[str, Any] | None = None


@dataclass(frozen=True)
class ExtractionOutcome:
    source: str | None
    diagnostics: dict[str, Any] | None


@dataclass(frozen=True)
class GradeOutcome:
    compile_pass: bool
    typecheck_pass: bool
    tests_pass: int
    tests_total: int
    diagnostics: dict[str, Any] | None


@dataclass(frozen=True)
class TaskMetric:
    task_id: str
    stdlib_dominated: bool
    compile_pass: bool
    typecheck_pass: bool
    tests_pass: int
    tests_total: int
    generation_tokens: int
    python_baseline_tokens: int
    diagnostics: dict[str, Any] | None
    duration_ms: int
    retries: int

    def to_json(self) -> dict[str, Any]:
        return {
            "task_id": self.task_id,
            "stdlib_dominated": self.stdlib_dominated,
            "compile_pass": self.compile_pass,
            "typecheck_pass": self.typecheck_pass,
            "tests_pass": self.tests_pass,
            "tests_total": self.tests_total,
            "generation_tokens": self.generation_tokens,
            "python_baseline_tokens": self.python_baseline_tokens,
            "diagnostics": self.diagnostics,
            "duration_ms": self.duration_ms,
            "retries": self.retries,
        }


class EvalError(RuntimeError):
    pass


def task_key(task_dir: Path) -> str:
    return f"{task_dir.parent.name}/{task_dir.name}"


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


def _utc_now() -> datetime:
    return datetime.now(UTC)


def _iso_z(dt: datetime) -> str:
    return dt.isoformat(timespec="seconds").replace("+00:00", "Z")


def _uuid7() -> str:
    """Generate a UUIDv7-shaped identifier without an external dependency."""

    timestamp_ms = int(time.time() * 1000) & ((1 << 48) - 1)
    rand_a = random.getrandbits(12)
    rand_b = random.getrandbits(62)
    value = (timestamp_ms << 80) | (0x7 << 76) | (rand_a << 64) | (0b10 << 62) | rand_b
    hexed = f"{value:032x}"
    return f"{hexed[:8]}-{hexed[8:12]}-{hexed[12:16]}-{hexed[16:20]}-{hexed[20:]}"


def _git_sha() -> str:
    proc = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        timeout=10,
    )
    if proc.returncode == 0:
        return proc.stdout.strip()
    return "unknown"


def _load_dominance_map() -> dict[str, bool]:
    with DOMINANCE_FILE.open("rb") as fh:
        data = tomllib.load(fh)
    return dict(data["stdlib_dominated"])


def _task_dirs(scope: Scope) -> list[tuple[Path, bool]]:
    root = SEALED_ROOT if scope == "sealed" else OPEN_TASKS_ROOT
    return [(p, scope == "sealed") for p in sorted(root.glob("*/*")) if p.is_dir()]


def _parse_task_selectors(raw: list[str] | None) -> set[str]:
    selectors: set[str] = set()
    for value in raw or []:
        for part in value.split(","):
            part = part.strip()
            if part:
                selectors.add(part)
    return selectors


def _matches_selector(task: EvalTask, selectors: set[str]) -> bool:
    if not selectors:
        return True
    number = task.task_dir.name.split("-", 1)[0]
    return any(
        selector in {task.task_id, task.task_dir.name, number}
        for selector in selectors
    )


def load_tasks(
    *,
    scope: Scope,
    selectors: set[str],
    enc: tiktoken.Encoding,
) -> list[EvalTask]:
    dominance = _load_dominance_map()
    tasks: list[EvalTask] = []
    for task_dir, sealed in _task_dirs(scope):
        key = task_key(task_dir)
        if key not in dominance:
            raise EvalError(
                f"stdlib-dominance.toml is missing an entry for {key}; "
                "add one per ADR 0021 before running corpus-eval"
            )
        python_ref = task_dir / "reference.py"
        py_tokens = len(enc.encode(python_ref.read_text(encoding="utf-8")))
        task = EvalTask(
            task_id=key,
            task_dir=task_dir,
            sealed=sealed,
            stdlib_dominated=dominance[key],
            python_baseline_tokens=py_tokens,
        )
        if _matches_selector(task, selectors):
            tasks.append(task)

    tasks.sort(key=lambda task: (task.task_dir.name.split("-", 1)[0], task.task_id))
    if selectors and not tasks:
        raise EvalError(f"no tasks matched selector(s): {', '.join(sorted(selectors))}")
    return tasks


def load_tests(task_dir: Path) -> list[dict[str, str]]:
    with (task_dir / "tests.jsonl").open(encoding="utf-8") as f:
        return [json.loads(line) for line in f if line.strip()]


def load_primer(enc: tiktoken.Encoding) -> tuple[str, str, int]:
    primer_bytes = PRIMER_PATH.read_bytes()
    primer_text = primer_bytes.decode("utf-8")
    primer_hash = blake3.blake3(primer_bytes).hexdigest()
    primer_tokens = len(enc.encode(primer_text))
    return primer_text, primer_hash, primer_tokens


def build_task_prompt(task_dir: Path) -> str:
    task_text = (task_dir / "task.md").read_text(encoding="utf-8").rstrip()
    return f"{task_text}\n\n{PROMPT_SUFFIX}\n"


def infer_provider(model: str, requested: str) -> Provider:
    if requested in {"anthropic", "openrouter"}:
        return requested  # type: ignore[return-value]
    if model.startswith("claude-"):
        return "anthropic"
    return "openrouter"


def _require_api_key(provider: Provider) -> str:
    env_name = "ANTHROPIC_API_KEY" if provider == "anthropic" else "OPENROUTER_API_KEY"
    value = os.environ.get(env_name)
    if not value:
        raise EvalError(f"{env_name} must be set before running a real corpus-eval")
    return value


def _http_json(
    *,
    url: str,
    headers: dict[str, str],
    payload: dict[str, Any],
    timeout_seconds: int,
) -> tuple[int, dict[str, Any], dict[str, str]]:
    data = json.dumps(payload).encode("utf-8")
    request = urllib.request.Request(url, data=data, headers=headers, method="POST")
    try:
        with urllib.request.urlopen(request, timeout=timeout_seconds) as response:
            body = response.read().decode("utf-8")
            return response.status, json.loads(body), _normalize_headers(response.headers.items())
    except urllib.error.HTTPError as e:
        body = e.read().decode("utf-8", errors="replace")
        try:
            parsed = json.loads(body)
        except json.JSONDecodeError:
            parsed = {"error": body}
        return e.code, parsed, _normalize_headers(e.headers.items()) if e.headers is not None else {}


def _transient_status(status: int) -> bool:
    return status in {408, 429} or 500 <= status <= 599


def _normalize_headers(items: Any) -> dict[str, str]:
    return {str(key).lower(): str(value) for key, value in items}


def _anthropic_call(
    *,
    api_key: str,
    model: str,
    primer: str,
    prompt: str,
    max_tokens: int,
    temperature: float,
    timeout_seconds: int,
) -> ModelResponse:
    status, data, response_headers = _http_json(
        url="https://api.anthropic.com/v1/messages",
        headers={
            "content-type": "application/json",
            "x-api-key": api_key,
            "anthropic-version": "2023-06-01",
        },
        payload={
            "model": model,
            "system": [
                {
                    "type": "text",
                    "text": primer,
                    "cache_control": {"type": "ephemeral"},
                }
            ],
            "max_tokens": max_tokens,
            "temperature": temperature,
            "messages": [{"role": "user", "content": prompt}],
        },
        timeout_seconds=timeout_seconds,
    )
    if status >= 400:
        raise ApiStatusError(status, data, headers=response_headers)
    text_parts: list[str] = []
    for block in data.get("content", []):
        if isinstance(block, dict) and block.get("type") == "text":
            text_parts.append(str(block.get("text", "")))
    return ModelResponse(text="".join(text_parts), stop_reason=data.get("stop_reason"))


def _openrouter_call(
    *,
    api_key: str,
    model: str,
    primer: str,
    prompt: str,
    max_tokens: int,
    temperature: float,
    timeout_seconds: int,
) -> ModelResponse:
    status, data, response_headers = _http_json(
        url="https://openrouter.ai/api/v1/chat/completions",
        headers={
            "content-type": "application/json",
            "authorization": f"Bearer {api_key}",
            "x-openrouter-title": "Tacit Phase 3 corpus-eval",
        },
        payload={
            "model": model,
            "max_tokens": max_tokens,
            "temperature": temperature,
            "messages": [
                {"role": "system", "content": primer},
                {"role": "user", "content": prompt},
            ],
        },
        timeout_seconds=timeout_seconds,
    )
    if status >= 400:
        raise ApiStatusError(status, data, headers=response_headers)
    choices = data.get("choices") or []
    if not choices:
        raise ApiStatusError(502, {"error": "OpenRouter response had no choices"}, headers={})
    choice = choices[0]
    message = choice.get("message", {})
    content = message.get("content", "")
    if isinstance(content, list):
        text = "".join(
            str(part.get("text", ""))
            for part in content
            if isinstance(part, dict) and part.get("type") in {"text", "output_text"}
        )
    else:
        text = str(content)
    return ModelResponse(text=text, stop_reason=choice.get("finish_reason"))


class ApiStatusError(RuntimeError):
    def __init__(self, status: int, body: dict[str, Any], headers: dict[str, str] | None = None):
        super().__init__(f"API returned HTTP {status}: {body!r}")
        self.status = status
        self.body = body
        self.headers = {str(key).lower(): str(value) for key, value in (headers or {}).items()}


def call_model_with_retries(
    *,
    provider: Provider,
    api_key: str,
    model: str,
    primer: str,
    prompt: str,
    max_tokens: int,
    temperature: float,
    timeout_seconds: int,
    max_retries: int,
) -> ModelOutcome:
    retries = 0
    for attempt in range(max_retries + 1):
        retry_headers: dict[str, str] | None = None
        try:
            if provider == "anthropic":
                response = _anthropic_call(
                    api_key=api_key,
                    model=model,
                    primer=primer,
                    prompt=prompt,
                    max_tokens=max_tokens,
                    temperature=temperature,
                    timeout_seconds=timeout_seconds,
                )
            else:
                response = _openrouter_call(
                    api_key=api_key,
                    model=model,
                    primer=primer,
                    prompt=prompt,
                    max_tokens=max_tokens,
                    temperature=temperature,
                    timeout_seconds=timeout_seconds,
                )
            return ModelOutcome(response=response, retries=retries)
        except ApiStatusError as e:
            if not _transient_status(e.status) or attempt == max_retries:
                return ModelOutcome(
                    response=None,
                    retries=retries,
                    diagnostics=diagnostic_envelope(
                        "api-error", f"model API call failed: HTTP {e.status}"
                    ),
                )
            retry_headers = e.headers
        except (TimeoutError, urllib.error.URLError, OSError) as e:
            if attempt == max_retries:
                return ModelOutcome(
                    response=None,
                    retries=retries,
                    diagnostics=diagnostic_envelope(
                        "api-error", f"model API call failed: {e}"
                    ),
                )

        retries += 1
        time.sleep(_retry_delay_seconds(attempt=attempt, headers=retry_headers))

    return ModelOutcome(
        response=None,
        retries=retries,
        diagnostics=diagnostic_envelope("api-error", "model API call failed"),
    )


def _retry_delay_seconds(*, attempt: int, headers: dict[str, str] | None) -> float:
    retry_after = _parse_retry_after(headers.get("retry-after")) if headers else None
    if retry_after is not None:
        return retry_after
    return float(2**attempt)


def _parse_retry_after(value: str | None) -> float | None:
    if value is None:
        return None
    value = value.strip()
    if not value:
        return None
    try:
        seconds = float(value)
    except ValueError:
        pass
    else:
        return max(0.0, seconds)
    try:
        dt = email.utils.parsedate_to_datetime(value)
    except (TypeError, ValueError, IndexError):
        return None
    if dt.tzinfo is None:
        dt = dt.replace(tzinfo=UTC)
    delta = (dt - datetime.now(UTC)).total_seconds()
    return max(0.0, delta)


TACIT_FENCE_RE = re.compile(
    r"```[ \t]*tacit[ \t]*\r?\n(?P<body>.*?)(?:\r?\n)?```",
    re.DOTALL | re.IGNORECASE,
)
TACIT_FENCE_START_RE = re.compile(r"```[ \t]*tacit\b", re.IGNORECASE)


def extract_tacit_source(response: ModelResponse) -> ExtractionOutcome:
    if response.stop_reason == "max_tokens":
        return ExtractionOutcome(
            source=None,
            diagnostics=diagnostic_envelope(
                "extraction-error", "model response stopped at max_tokens"
            ),
        )

    starts = TACIT_FENCE_START_RE.findall(response.text)
    matches = list(TACIT_FENCE_RE.finditer(response.text))
    if not starts:
        return ExtractionOutcome(
            source=None,
            diagnostics=diagnostic_envelope(
                "extraction-error", "model response contained no ```tacit fenced block"
            ),
        )
    if len(starts) != 1 or len(matches) != 1:
        message = (
            "model response contained multiple ```tacit fenced blocks"
            if len(starts) > 1 or len(matches) > 1
            else "model response opened a ```tacit fenced block but did not close it"
        )
        return ExtractionOutcome(
            source=None,
            diagnostics=diagnostic_envelope("extraction-error", message),
        )

    source = matches[0].group("body").strip()
    if not source:
        return ExtractionOutcome(
            source=None,
            diagnostics=diagnostic_envelope(
                "extraction-error", "model response contained an empty Tacit block"
            ),
        )
    return ExtractionOutcome(source=f"{source}\n", diagnostics=None)


def diagnostic_envelope(
    kind: str,
    message: str,
    *,
    expected: Any = None,
    actual: Any = None,
) -> dict[str, Any]:
    return {
        "schema_version": "p2.0",
        "errors": [
            {
                "kind": kind,
                "severity": "error",
                "location": {"ast_path": [], "source_span": None},
                "message": message,
                "expected": expected,
                "actual": actual,
                "fix": None,
                "related": [],
            }
        ],
    }


def _parse_diagnostic_output(text: str, fallback_kind: str) -> dict[str, Any]:
    stripped = text.strip()
    if stripped:
        try:
            data = json.loads(stripped)
            if data.get("schema_version") == "p2.0" and isinstance(data.get("errors"), list):
                return data
        except json.JSONDecodeError:
            pass
    return diagnostic_envelope(fallback_kind, stripped or "command failed without diagnostics")


def _run_tacit_check(tacit_bin: Path, source: Path) -> tuple[bool, dict[str, Any] | None]:
    proc = subprocess.run(
        [str(tacit_bin), "check", str(source), "--format", "json"],
        capture_output=True,
        text=True,
        timeout=30,
    )
    if proc.returncode == 0:
        return True, None
    return False, _parse_diagnostic_output(proc.stdout or proc.stderr, "typecheck-error")


def _run_tacit_compile(
    tacit_bin: Path,
    source: Path,
    binary: Path,
) -> tuple[bool, dict[str, Any] | None]:
    proc = subprocess.run(
        [str(tacit_bin), "compile", str(source), "-o", str(binary)],
        capture_output=True,
        text=True,
        timeout=60,
    )
    if proc.returncode == 0:
        return True, None
    return False, _parse_diagnostic_output(proc.stderr or proc.stdout, "compile-error")


def _compare_test_output(
    proc: subprocess.CompletedProcess[bytes],
    test: dict[str, str],
    *,
    sealed: bool,
) -> tuple[bool, str]:
    if proc.returncode != 0:
        return False, (
            f"nonzero exit {proc.returncode}"
            if sealed
            else f"nonzero exit {proc.returncode}; stderr={proc.stderr!r}"
        )
    expected_stdout = test["stdout"].encode("utf-8")
    if proc.stdout != expected_stdout:
        if sealed:
            return False, "stdout mismatch"
        return (
            False,
            f"stdout mismatch for {test['name']}: expected {test['stdout']!r}, got {proc.stdout!r}",
        )
    return True, ""


def grade_source(
    *,
    tacit_bin: Path,
    task: EvalTask,
    source_path: Path,
    binary_path: Path,
) -> GradeOutcome:
    typecheck_pass, check_diags = _run_tacit_check(tacit_bin, source_path)
    if not typecheck_pass:
        return GradeOutcome(
            compile_pass=False,
            typecheck_pass=False,
            tests_pass=0,
            tests_total=0,
            diagnostics=check_diags,
        )

    compile_pass, compile_diags = _run_tacit_compile(tacit_bin, source_path, binary_path)
    if not compile_pass:
        return GradeOutcome(
            compile_pass=False,
            typecheck_pass=True,
            tests_pass=0,
            tests_total=0,
            diagnostics=compile_diags,
        )

    tests = load_tests(task.task_dir)
    passed = 0
    failures: list[str] = []
    for test in tests:
        proc = subprocess.run(
            [str(binary_path)],
            input=test["stdin"].encode("utf-8"),
            capture_output=True,
            text=False,
            timeout=30,
        )
        ok, message = _compare_test_output(proc, test, sealed=task.sealed)
        if ok:
            passed += 1
        else:
            failures.append(message)

    if failures:
        if task.sealed:
            message = f"{len(failures)} test case(s) failed; details redacted for sealed task"
        else:
            message = "; ".join(failures[:3])
            if len(failures) > 3:
                message = f"{message}; {len(failures) - 3} more failure(s)"
        diagnostics = diagnostic_envelope("test-failure", message)
    else:
        diagnostics = None

    return GradeOutcome(
        compile_pass=True,
        typecheck_pass=True,
        tests_pass=passed,
        tests_total=len(tests),
        diagnostics=diagnostics,
    )


def synthesize_sidecar(task: EvalTask, generated_source: Path) -> None:
    sidecar_path = generated_source.with_suffix(generated_source.suffix + ".sidecar.toml")
    harness_spec = task.task_dir / "harness-spec.toml"
    reference_sidecar = task.task_dir / "reference.tac.sidecar.toml"
    if harness_spec.is_file():
        shutil.copyfile(harness_spec, sidecar_path)
    elif reference_sidecar.is_file():
        shutil.copyfile(reference_sidecar, sidecar_path)
    else:
        sidecar_path.write_text(
            '[types.main]\ntype = "Int"\neffects = ["Alloc", "IO", "Mut"]\n',
            encoding="utf-8",
        )


def failure_dir(base: Path, run_id: str, task_id: str) -> Path:
    return base / "failures" / run_id / task_id


def capture_failure(
    *,
    output_dir: Path,
    run_id: str,
    task: EvalTask,
    raw_output: str | None,
    source: str | None,
    diagnostics: dict[str, Any] | None,
) -> None:
    path = failure_dir(output_dir, run_id, task.task_id)
    path.mkdir(parents=True, exist_ok=True)
    if raw_output is not None:
        (path / "raw-response.txt").write_text(raw_output, encoding="utf-8")
    if source is not None:
        (path / "generated.tac").write_text(source, encoding="utf-8")
    if diagnostics is not None:
        (path / "diagnostics.json").write_text(
            json.dumps(diagnostics, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )


def _dry_run_response(task: EvalTask) -> ModelResponse:
    reference = task.task_dir / "reference.tac"
    if not reference.is_file():
        return ModelResponse(
            text="```tacit\n0\n```",
            stop_reason="end_turn",
        )
    source = reference.read_text(encoding="utf-8").strip()
    return ModelResponse(text=f"```tacit\n{source}\n```", stop_reason="end_turn")


def evaluate_task(
    *,
    task: EvalTask,
    run_id: str,
    dry_run: bool,
    provider: Provider,
    api_key: str | None,
    model: str,
    primer: str,
    enc: tiktoken.Encoding,
    tacit_bin: Path,
    output_dir: Path,
    retain_outputs: bool,
    max_tokens: int,
    temperature: float,
    timeout_seconds: int,
    max_retries: int,
) -> TaskMetric:
    started = time.monotonic()
    raw_output: str | None = None
    retries = 0

    if dry_run:
        model_response = _dry_run_response(task)
    else:
        assert api_key is not None
        prompt = build_task_prompt(task.task_dir)
        outcome = call_model_with_retries(
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
        retries = outcome.retries
        if outcome.response is None:
            diagnostics = outcome.diagnostics
            duration_ms = int((time.monotonic() - started) * 1000)
            capture_failure(
                output_dir=output_dir,
                run_id=run_id,
                task=task,
                raw_output=None,
                source=None,
                diagnostics=diagnostics,
            )
            return TaskMetric(
                task_id=task.task_id,
                stdlib_dominated=task.stdlib_dominated,
                compile_pass=False,
                typecheck_pass=False,
                tests_pass=0,
                tests_total=0,
                generation_tokens=0,
                python_baseline_tokens=task.python_baseline_tokens,
                diagnostics=diagnostics,
                duration_ms=duration_ms,
                retries=retries,
            )
        model_response = outcome.response

    raw_output = model_response.text
    if retain_outputs and not task.sealed:
        out_path = output_dir / f"{run_id}.outputs" / f"{task.task_id}.txt"
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(raw_output, encoding="utf-8")

    extraction = extract_tacit_source(model_response)
    if extraction.source is None:
        diagnostics = extraction.diagnostics
        duration_ms = int((time.monotonic() - started) * 1000)
        capture_failure(
            output_dir=output_dir,
            run_id=run_id,
            task=task,
            raw_output=raw_output,
            source=None,
            diagnostics=diagnostics,
        )
        return TaskMetric(
            task_id=task.task_id,
            stdlib_dominated=task.stdlib_dominated,
            compile_pass=False,
            typecheck_pass=False,
            tests_pass=0,
            tests_total=0,
            generation_tokens=0,
            python_baseline_tokens=task.python_baseline_tokens,
            diagnostics=diagnostics,
            duration_ms=duration_ms,
            retries=retries,
        )

    generation_tokens = len(enc.encode(extraction.source))

    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
        generated = tmp / "generated.tac"
        binary = tmp / "generated"
        generated.write_text(extraction.source, encoding="utf-8")
        synthesize_sidecar(task, generated)
        grade = grade_source(
            tacit_bin=tacit_bin,
            task=task,
            source_path=generated,
            binary_path=binary,
        )

    diagnostics = grade.diagnostics
    if diagnostics is not None:
        capture_failure(
            output_dir=output_dir,
            run_id=run_id,
            task=task,
            raw_output=raw_output,
            source=extraction.source,
            diagnostics=diagnostics,
        )

    duration_ms = int((time.monotonic() - started) * 1000)
    return TaskMetric(
        task_id=task.task_id,
        stdlib_dominated=task.stdlib_dominated,
        compile_pass=grade.compile_pass,
        typecheck_pass=grade.typecheck_pass,
        tests_pass=grade.tests_pass,
        tests_total=grade.tests_total,
        generation_tokens=generation_tokens,
        python_baseline_tokens=task.python_baseline_tokens,
        diagnostics=diagnostics,
        duration_ms=duration_ms,
        retries=retries,
    )


def _rate(numerator: int, denominator: int) -> float:
    return numerator / denominator if denominator else 0.0


def _bucket(tasks: list[TaskMetric], primer_tokens: int) -> dict[str, Any]:
    task_count = len(tasks)
    compile_passes = sum(1 for task in tasks if task.compile_pass)
    typecheck_passes = sum(1 for task in tasks if task.typecheck_pass)
    full_test_passes = sum(
        1 for task in tasks if task.tests_total > 0 and task.tests_pass == task.tests_total
    )
    generation_total = sum(task.generation_tokens for task in tasks)
    tacit_total = sum(primer_tokens + task.generation_tokens for task in tasks)
    python_total = sum(task.python_baseline_tokens for task in tasks)
    tests_pass_total = sum(task.tests_pass for task in tasks)
    tests_total_total = sum(task.tests_total for task in tasks)
    token_delta = ((tacit_total - python_total) / python_total) if python_total else 0.0
    return {
        "task_count": task_count,
        "compile_pass_rate": _rate(compile_passes, task_count),
        "typecheck_pass_rate": _rate(typecheck_passes, task_count),
        "tests_pass_rate": _rate(full_test_passes, task_count),
        "tacit_tokens_total": tacit_total,
        "python_tokens_total": python_total,
        "token_delta": token_delta,
        "primer_amortized_total": (primer_tokens + generation_total) if task_count else 0,
        "tests_pass_total": tests_pass_total,
        "tests_total_total": tests_total_total,
    }


def primary_aggregates(tasks: list[TaskMetric], primer_tokens: int) -> dict[str, Any]:
    stdlib = [task for task in tasks if task.stdlib_dominated]
    non_stdlib = [task for task in tasks if not task.stdlib_dominated]
    return {
        "full": _bucket(tasks, primer_tokens),
        "stdlib_dominated": _bucket(stdlib, primer_tokens),
        "non_stdlib_dominated": _bucket(non_stdlib, primer_tokens),
    }


def primary_gates(aggregates: dict[str, Any]) -> dict[str, Any]:
    full = aggregates["full"]
    non_stdlib = aggregates["non_stdlib_dominated"]
    gates = {
        "primary_pass_rate_gate": {
            "applies": True,
            "metric": "tests_pass_rate",
            "scope": "full",
            "threshold": 0.70,
            "value": full["tests_pass_rate"],
            "passed": full["tests_pass_rate"] >= 0.70,
        },
        "primary_token_gate_full": {
            "applies": True,
            "metric": "token_delta",
            "scope": "full",
            "threshold": -0.30,
            "value": full["token_delta"],
            "passed": full["token_delta"] <= -0.30,
        },
        "primary_token_gate_non_stdlib": {
            "applies": True,
            "metric": "token_delta",
            "scope": "non_stdlib_dominated",
            "threshold": -0.30,
            "value": non_stdlib["token_delta"],
            "passed": non_stdlib["token_delta"] <= -0.30,
        },
    }
    gates["passed_overall"] = all(
        value["passed"] for value in gates.values() if isinstance(value, dict)
    )
    return gates


def build_metrics(
    *,
    run_id: str,
    track: Track,
    scope: Scope,
    provider: Provider,
    model_id: str,
    primer_hash: str,
    primer_tokens: int,
    tasks: list[TaskMetric],
) -> dict[str, Any]:
    aggregates = primary_aggregates(tasks, primer_tokens)
    gates = primary_gates(aggregates) if track == "primary" else reporting_only_gates()
    return {
        "schema_version": "p3.0",
        "run_id": run_id,
        "track": track,
        "scope": scope,
        "model": {
            "provider": provider,
            "id": model_id,
            "primer_hash": primer_hash,
            "primer_tokens": primer_tokens,
        },
        "tasks": [task.to_json() for task in tasks],
        "aggregates": aggregates,
        "gates": gates,
    }


def reporting_only_gates() -> dict[str, Any]:
    return {
        "primary_pass_rate_gate": {
            "applies": False,
            "metric": "tests_pass_rate",
            "scope": "full",
            "threshold": 0.70,
            "value": 0.0,
            "passed": False,
        },
        "primary_token_gate_full": {
            "applies": False,
            "metric": "token_delta",
            "scope": "full",
            "threshold": -0.30,
            "value": 0.0,
            "passed": False,
        },
        "primary_token_gate_non_stdlib": {
            "applies": False,
            "metric": "token_delta",
            "scope": "non_stdlib_dominated",
            "threshold": -0.30,
            "value": 0.0,
            "passed": False,
        },
    }


def validate_metrics_shape(metrics: dict[str, Any]) -> None:
    """Small built-in guard matching the committed JSON Schema's required fields."""

    required = {
        "schema_version",
        "run_id",
        "track",
        "scope",
        "model",
        "tasks",
        "aggregates",
        "gates",
    }
    missing = sorted(required - metrics.keys())
    if missing:
        raise EvalError(f"metrics object missing required field(s): {', '.join(missing)}")
    if metrics["schema_version"] != "p3.0":
        raise EvalError("metrics schema_version must be p3.0")
    if metrics["track"] not in {"primary", "maintenance", "cross-family"}:
        raise EvalError("metrics track is invalid")
    if metrics["scope"] not in {"open", "sealed", "maintenance"}:
        raise EvalError("metrics scope is invalid")
    model = metrics["model"]
    if model["provider"] not in {"anthropic", "openrouter"}:
        raise EvalError("metrics model.provider is invalid")
    if not re.fullmatch(r"[0-9a-f]{64}", model["primer_hash"]):
        raise EvalError("metrics model.primer_hash is not a 64-char BLAKE3 hex")


def build_run_metadata(
    *,
    run_id: str,
    provider: Provider,
    model_id: str,
    scope: Scope,
    task_ids: list[str],
    primer_hash: str,
    primer_tokens: int,
    temperature: float,
    max_tokens: int,
    started_at: datetime,
    completed_at: datetime,
    dry_run: bool,
) -> dict[str, Any]:
    sha = _git_sha()
    return {
        "run_id": run_id,
        "harness_git_sha": sha,
        "corpus_git_sha": sha,
        "primer_blake3": primer_hash,
        "primer_token_count": primer_tokens,
        "model_id": model_id,
        "provider": provider,
        "sampling": {
            "temperature": temperature,
            "max_tokens": max_tokens,
        },
        "tasks_scope": scope,
        "task_ids": task_ids,
        "started_at": _iso_z(started_at),
        "completed_at": _iso_z(completed_at),
        "tiktoken_encoding": "o200k_base",
        "harness_version": HARNESS_VERSION,
        "dry_run": dry_run,
    }


def write_json(path: Path, data: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def display_path(path: Path) -> str:
    try:
        return str(path.relative_to(REPO_ROOT))
    except ValueError:
        return str(path)


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", help="model id to evaluate")
    parser.add_argument(
        "--provider",
        choices=["auto", "anthropic", "openrouter"],
        default="auto",
        help="model provider (default: infer from model id)",
    )
    parser.add_argument(
        "--track",
        choices=["auto", "primary", "cross-family"],
        default="auto",
        help="metrics track (default: primary for Anthropic, cross-family for OpenRouter)",
    )
    parser.add_argument(
        "--include-sealed",
        action="store_true",
        help="run sealed held-out tasks instead of open tasks (Phase 3 grading mode)",
    )
    parser.add_argument(
        "--tasks",
        action="append",
        help="task selector(s): 001, 001-sum-to-n, arithmetic/001-sum-to-n; comma-separated allowed",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=RESULTS_DIR,
        help="directory for run.json, metrics.json, and failures/",
    )
    parser.add_argument(
        "--tacit-bin",
        type=Path,
        default=_default_tacit_bin(),
        help="path to tacit CLI binary (default: TACIT_BIN, ../../target/debug/tacit, or PATH)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="use open Tacit references as synthetic model output; no API call",
    )
    parser.add_argument(
        "--retain-outputs",
        action="store_true",
        help="write raw model outputs under <run-id>.outputs/ for open tasks",
    )
    parser.add_argument("--max-tokens", type=int, default=DEFAULT_MAX_TOKENS)
    parser.add_argument("--temperature", type=float, default=DEFAULT_TEMPERATURE)
    parser.add_argument("--timeout-seconds", type=int, default=DEFAULT_TIMEOUT_SECONDS)
    parser.add_argument("--max-retries", type=int, default=DEFAULT_MAX_RETRIES)
    return parser.parse_args(argv)


def run(argv: list[str] | None = None) -> int:
    args = parse_args(argv)

    if args.tacit_bin is None or not args.tacit_bin.is_file():
        raise EvalError("tacit binary not found; build it or pass --tacit-bin")

    model = args.model or ("claude-sonnet-4-6" if args.dry_run else None)
    if model is None:
        raise EvalError("--model is required unless --dry-run is used")

    provider = infer_provider(model, args.provider)
    track: Track
    if args.track == "auto":
        track = "primary" if provider == "anthropic" else "cross-family"
    else:
        track = args.track

    scope: Scope = "sealed" if args.include_sealed else "open"
    if args.retain_outputs and scope == "sealed":
        raise EvalError("--retain-outputs is disabled for sealed runs")

    enc = tiktoken.get_encoding("o200k_base")
    primer, primer_hash, primer_tokens = load_primer(enc)
    selectors = _parse_task_selectors(args.tasks)
    tasks = load_tasks(scope=scope, selectors=selectors, enc=enc)
    if args.dry_run and not selectors:
        tasks = tasks[:1]
    if not tasks:
        raise EvalError("no tasks selected")

    api_key = None if args.dry_run else _require_api_key(provider)
    run_id = _uuid7()
    started_at = _utc_now()
    task_metrics: list[TaskMetric] = []

    print(
        f"corpus-eval run_id={run_id} scope={scope} track={track} "
        f"provider={provider} model={model} tasks={len(tasks)}"
    )
    for task in tasks:
        metric = evaluate_task(
            task=task,
            run_id=run_id,
            dry_run=args.dry_run,
            provider=provider,
            api_key=api_key,
            model=model,
            primer=primer,
            enc=enc,
            tacit_bin=args.tacit_bin,
            output_dir=args.output_dir,
            retain_outputs=args.retain_outputs,
            max_tokens=args.max_tokens,
            temperature=args.temperature,
            timeout_seconds=args.timeout_seconds,
            max_retries=args.max_retries,
        )
        task_metrics.append(metric)
        ok = metric.tests_pass == metric.tests_total and metric.tests_total > 0
        print("." if ok else "F", end="", flush=True)
    print()

    completed_at = _utc_now()
    metrics = build_metrics(
        run_id=run_id,
        track=track,
        scope=scope,
        provider=provider,
        model_id=model,
        primer_hash=primer_hash,
        primer_tokens=primer_tokens,
        tasks=task_metrics,
    )
    validate_metrics_shape(metrics)
    run_metadata = build_run_metadata(
        run_id=run_id,
        provider=provider,
        model_id=model,
        scope=scope,
        task_ids=[task.task_id for task in tasks],
        primer_hash=primer_hash,
        primer_tokens=primer_tokens,
        temperature=args.temperature,
        max_tokens=args.max_tokens,
        started_at=started_at,
        completed_at=completed_at,
        dry_run=args.dry_run,
    )

    run_path = args.output_dir / f"{run_id}.run.json"
    metrics_path = args.output_dir / f"{run_id}.metrics.json"
    write_json(run_path, run_metadata)
    write_json(metrics_path, metrics)

    full = metrics["aggregates"]["full"]
    print(f"wrote {display_path(run_path)} and {display_path(metrics_path)}")
    print(
        f"tasks_passed={full['tests_pass_rate']:.3f} "
        f"compile_pass={full['compile_pass_rate']:.3f} "
        f"token_delta={full['token_delta']:.3f}"
    )
    return 0


def main() -> None:
    try:
        raise SystemExit(run())
    except EvalError as e:
        print(f"error: {e}", file=sys.stderr)
        raise SystemExit(2)


if __name__ == "__main__":
    main()
