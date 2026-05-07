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
from tacit_corpus.preflight import binary_metadata as _binary_metadata

Provider = Literal["anthropic", "openrouter"]
Track = Literal["primary", "cross-family"]
Scope = Literal["open", "sealed"]
ResultLabel = Literal["core-language", "library-mediated"]
FailureStage = Literal["api", "extract", "typecheck", "compile", "test"]

DEFAULT_TEMPERATURE = 0
DEFAULT_MAX_TOKENS = 8192
DEFAULT_TIMEOUT_SECONDS = 120
DEFAULT_MAX_RETRIES = 3
HARNESS_VERSION = "0.4.0"
PROMPT_SUFFIX = (
    "Write the solution as a single Tacit-Lite program in a fenced "
    "block: ```tacit ... ```. Do not include any extra files, metadata, "
    "explanation, or second block."
)
REPAIR_RAW_TEXT_LIMIT = 2000
REPAIR_TEST_FAILURE_LIMIT = 2
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
    failure_stage: FailureStage | None = None
    test_failures: tuple["TestFailureDetail", ...] = ()


@dataclass(frozen=True)
class TestFailureDetail:
    name: str
    stdin: str
    expected_stdout: str
    actual_stdout: str | None = None
    failure_reason: str | None = None

    def to_json(self) -> dict[str, Any]:
        return {
            "name": self.name,
            "stdin": self.stdin,
            "expected_stdout": self.expected_stdout,
            "actual_stdout": self.actual_stdout,
            "failure_reason": self.failure_reason,
        }


@dataclass(frozen=True)
class TurnResult:
    turn_index: int
    compile_pass: bool
    typecheck_pass: bool
    tests_pass: int
    tests_total: int
    generation_tokens: int
    diagnostics: dict[str, Any] | None
    retries: int
    raw_output: str | None
    source: str | None
    failure_stage: FailureStage | None
    structural_equivalent: bool = False
    test_failures: tuple[TestFailureDetail, ...] = ()

    @property
    def tests_success(self) -> bool:
        return self.tests_total > 0 and self.tests_pass == self.tests_total


@dataclass(frozen=True)
class RepairMetric:
    turns_used: int | None
    first_pass_compile_pass: bool
    first_pass_typecheck_pass: bool
    first_pass_tests_pass: bool
    final_compile_pass: bool
    final_typecheck_pass: bool
    final_tests_pass: bool
    repair_success: bool
    failure_stage_by_turn: dict[str, str | None]
    generation_tokens_by_turn: dict[str, int]
    diagnostics_by_turn: dict[str, Any]

    def to_json(self) -> dict[str, Any]:
        return {
            "turns_used": self.turns_used,
            "first_pass_compile_pass": self.first_pass_compile_pass,
            "first_pass_typecheck_pass": self.first_pass_typecheck_pass,
            "first_pass_tests_pass": self.first_pass_tests_pass,
            "final_compile_pass": self.final_compile_pass,
            "final_typecheck_pass": self.final_typecheck_pass,
            "final_tests_pass": self.final_tests_pass,
            "repair_success": self.repair_success,
            "failure_stage_by_turn": self.failure_stage_by_turn,
            "generation_tokens_by_turn": self.generation_tokens_by_turn,
            "diagnostics_by_turn": self.diagnostics_by_turn,
        }


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
    structural_equivalent: bool = False
    repair: RepairMetric | None = None

    def to_json(self) -> dict[str, Any]:
        data = {
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
            "structural_equivalent": self.structural_equivalent,
        }
        if self.repair is not None:
            data.update(self.repair.to_json())
        return data


class EvalError(RuntimeError):
    pass


def _unquote_dotenv_value(value: str) -> str:
    if len(value) >= 2 and value[0] == value[-1] and value[0] in {'"', "'"}:
        value = value[1:-1]
    return value.replace(r"\n", "\n").replace(r"\r", "\r").replace(r"\t", "\t")


def _load_cwd_dotenv() -> None:
    path = Path.cwd() / ".env"
    if not path.is_file():
        return

    for raw_line in path.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("export "):
            line = line[len("export ") :].lstrip()
        if "=" not in line:
            continue
        key, value = line.split("=", 1)
        key = key.strip()
        if not key or key in os.environ:
            continue
        value = value.strip()
        if "#" in value and not (value.startswith("'") or value.startswith('"')):
            value = value.split("#", 1)[0].rstrip()
        os.environ[key] = _unquote_dotenv_value(value)


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


def load_task_statement(task_dir: Path) -> str:
    return (task_dir / "task.md").read_text(encoding="utf-8").rstrip()


def build_task_prompt(task_dir: Path) -> str:
    task_text = load_task_statement(task_dir)
    return f"{task_text}\n\n{PROMPT_SUFFIX}\n"


def build_repair_prompt(task_dir: Path, previous: TurnResult) -> str:
    task_text = load_task_statement(task_dir)
    previous_source = previous.source.rstrip() if previous.source is not None else ""
    stage = previous.failure_stage or "test"
    feedback = repair_feedback(previous)
    return (
        "The previous Tacit-Lite program failed.\n\n"
        "Task:\n"
        f"{task_text}\n\n"
        "Previous program:\n"
        "```tacit\n"
        f"{previous_source}\n"
        "```\n\n"
        f"Failure stage: {stage}\n\n"
        "Feedback:\n"
        f"{feedback}\n\n"
        "Return a corrected solution as a single Tacit-Lite program in one fenced\n"
        "block: ```tacit ... ```. Do not include the sidecar. Do not include\n"
        "explanatory prose.\n"
    )


def repair_feedback(previous: TurnResult) -> str:
    parts: list[str] = []
    if previous.failure_stage == "test":
        parts.append(compact_test_failure_summary(previous))
    else:
        if previous.diagnostics is None:
            parts.append("No structured diagnostic was available.")
        else:
            parts.append(json.dumps(previous.diagnostics, indent=2, sort_keys=True))

        if previous.failure_stage == "extract" and previous.raw_output:
            if len(previous.raw_output) <= REPAIR_RAW_TEXT_LIMIT:
                parts.append(
                    f"Raw model text as a JSON string:\n{json.dumps(previous.raw_output)}"
                )
            else:
                parts.append(
                    "Raw model text omitted because it was "
                    f"{len(previous.raw_output)} characters, over the "
                    f"{REPAIR_RAW_TEXT_LIMIT}-character repair limit."
                )

    classification = generic_failure_classification(previous)
    if classification:
        parts.append(classification)
    return "\n\n".join(parts)


def compact_test_failure_summary(previous: TurnResult) -> str:
    if not previous.test_failures:
        if previous.diagnostics is None:
            return "Tests failed, but no concrete failing case detail was captured."
        return json.dumps(previous.diagnostics, indent=2, sort_keys=True)

    shown = previous.test_failures[:REPAIR_TEST_FAILURE_LIMIT]
    lines: list[str] = [
        f"{len(previous.test_failures)} failing test case(s) captured; "
        f"showing first {len(shown)}."
    ]
    for idx, failure in enumerate(shown, start=1):
        lines.append(f"\nFailure {idx}:")
        lines.append(f"name: {failure.name}")
        lines.append(f"stdin: {failure.stdin!r}")
        lines.append(f"expected stdout: {failure.expected_stdout!r}")
        if failure.actual_stdout is not None:
            lines.append(f"actual stdout: {failure.actual_stdout!r}")
        if failure.failure_reason is not None:
            lines.append(f"failure reason: {failure.failure_reason}")
    remaining = len(previous.test_failures) - len(shown)
    if remaining > 0:
        lines.append(f"\n{remaining} additional failing case(s) omitted.")
    return "\n".join(lines)


def generic_failure_classification(previous: TurnResult) -> str | None:
    hints = _test_failure_classification(previous.test_failures)
    hints.extend(_diagnostic_classification(previous.diagnostics))
    if not hints:
        return None

    deduped = list(dict.fromkeys(hints))
    lines = ["Generic failure classification:"]
    lines.extend(f"- {hint}" for hint in deduped)
    return "\n".join(lines)


def _diagnostic_classification(diagnostics: dict[str, Any] | None) -> list[str]:
    if diagnostics is None:
        return []

    text = json.dumps(diagnostics, sort_keys=True).lower()
    hints: list[str] = []
    if (
        "unknown primitive" in text
        or "unknown type" in text
        or "unresolved-type" in text
    ):
        hints.append(
            "unknown primitive or primitive spelling issue; use only primitive names "
            "listed in the primer, keep the leading @, and treat unprefixed names as "
            "local variables rather than primitives."
        )
    if "expected 'else'" in text or 'expected "else"' in text:
        hints.append(
            "Tacit if syntax issue; every if expression requires both then and else "
            "branches, and each branch must be a complete expression."
        )
    return hints


def _test_failure_classification(
    failures: tuple[TestFailureDetail, ...],
) -> list[str]:
    hints: list[str] = []
    if any(_has_exit_code(failure, -11) for failure in failures):
        hints.append(
            "segmentation fault; likely causes are out-of-bounds buffer access, "
            "invalid range-table access, unbounded recursion, or excessive stack "
            "allocation. Reduce allocation size, add zero-count guards, and check "
            "range/table bounds before changing the algorithm."
        )
    if any(_has_exit_1_empty_stderr(failure) for failure in failures):
        hints.append(
            "exit 1 with empty stderr; the program likely returned an explicit "
            "nonzero status or a runtime error sentinel. Inspect the final "
            "expression and early exits."
        )
    if any(_is_empty_input_formatting_failure(failure) for failure in failures):
        hints.append(
            "empty-input formatting bug; ensure the program prints the required "
            "separators and newline even when no rows or tokens are present."
        )
    if _has_repeated_separator_failures(failures):
        hints.append(
            "output formatting bug; check spaces, colons, and trailing newlines."
        )
    return hints


def _has_exit_code(failure: TestFailureDetail, code: int) -> bool:
    reason = failure.failure_reason or ""
    return f"nonzero exit {code}" in reason


def _has_exit_1_empty_stderr(failure: TestFailureDetail) -> bool:
    reason = failure.failure_reason
    return reason == "nonzero exit 1"


def _is_empty_input_formatting_failure(failure: TestFailureDetail) -> bool:
    if failure.actual_stdout is None:
        return False
    if failure.stdin != "" and "empty" not in failure.name.lower():
        return False
    return failure.expected_stdout == "\n" and failure.actual_stdout != "\n"


def _has_repeated_separator_failures(
    failures: tuple[TestFailureDetail, ...],
) -> bool:
    separator_only = [
        failure
        for failure in failures
        if failure.actual_stdout is not None
        and failure.expected_stdout != failure.actual_stdout
        and _strip_separators(failure.expected_stdout)
        == _strip_separators(failure.actual_stdout)
    ]
    return len(separator_only) >= 2


def _strip_separators(text: str) -> str:
    return re.sub(r"[ \t\r\n:]+", "", text)


def infer_provider(model: str, requested: str) -> Provider:
    if requested in {"anthropic", "openrouter"}:
        return requested  # type: ignore[return-value]
    if model.startswith("claude-"):
        return "anthropic"
    return "openrouter"


def _require_api_key(provider: Provider) -> str:
    _load_cwd_dotenv()
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


def _decode_bytes_for_feedback(value: bytes | None) -> str:
    if value is None:
        return ""
    try:
        return value.decode("utf-8")
    except UnicodeDecodeError:
        return repr(value)


def _run_test_case(
    *,
    binary_path: Path,
    test: dict[str, str],
    sealed: bool,
) -> tuple[bool, str, TestFailureDetail | None]:
    try:
        proc = subprocess.run(
            [str(binary_path)],
            input=test["stdin"].encode("utf-8"),
            capture_output=True,
            text=False,
            timeout=30,
        )
    except subprocess.TimeoutExpired as e:
        message = "timeout after 30 seconds"
        if sealed:
            return False, message, None
        return (
            False,
            message,
            TestFailureDetail(
                name=test["name"],
                stdin=test["stdin"],
                expected_stdout=test["stdout"],
                actual_stdout=_decode_bytes_for_feedback(e.output),
                failure_reason=message,
            ),
        )

    ok, message = _compare_test_output(proc, test, sealed=sealed)
    if ok:
        return True, "", None
    if sealed:
        return False, message, None

    actual_stdout = _decode_bytes_for_feedback(proc.stdout)
    failure_reason = None
    if proc.returncode != 0:
        stderr = _decode_bytes_for_feedback(proc.stderr)
        failure_reason = f"nonzero exit {proc.returncode}"
        if stderr:
            failure_reason = f"{failure_reason}; stderr={stderr!r}"

    return (
        False,
        message,
        TestFailureDetail(
            name=test["name"],
            stdin=test["stdin"],
            expected_stdout=test["stdout"],
            actual_stdout=actual_stdout,
            failure_reason=failure_reason,
        ),
    )


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
            failure_stage="typecheck",
        )

    compile_pass, compile_diags = _run_tacit_compile(tacit_bin, source_path, binary_path)
    if not compile_pass:
        return GradeOutcome(
            compile_pass=False,
            typecheck_pass=True,
            tests_pass=0,
            tests_total=0,
            diagnostics=compile_diags,
            failure_stage="compile",
        )

    tests = load_tests(task.task_dir)
    passed = 0
    failures: list[str] = []
    failure_details: list[TestFailureDetail] = []
    for test in tests:
        ok, message, detail = _run_test_case(
            binary_path=binary_path,
            test=test,
            sealed=task.sealed,
        )
        if ok:
            passed += 1
        else:
            failures.append(message)
            if detail is not None:
                failure_details.append(detail)

    if failures:
        if task.sealed:
            message = f"{len(failures)} test case(s) failed; details redacted for sealed task"
        else:
            message = "; ".join(failures[:3])
            if len(failures) > 3:
                message = f"{message}; {len(failures) - 3} more failure(s)"
        diagnostics = diagnostic_envelope("test-failure", message)
        failure_stage: FailureStage | None = "test"
    else:
        diagnostics = None
        failure_stage = None

    return GradeOutcome(
        compile_pass=True,
        typecheck_pass=True,
        tests_pass=passed,
        tests_total=len(tests),
        diagnostics=diagnostics,
        failure_stage=failure_stage,
        test_failures=tuple(failure_details),
    )


def _canonicalize_and_hash(tacit_bin: Path, authoring_path: Path) -> str | None:
    """Canonicalize a .taca file and return the BLAKE3 hash of the canonical bytes.

    Returns None if canonicalization fails (parse error, hole nodes, etc.).
    """
    proc = subprocess.run(
        [str(tacit_bin), "canonicalize", "--strict", str(authoring_path)],
        capture_output=True,
        timeout=30,
    )
    if proc.returncode != 0:
        return None
    tacd_path = authoring_path.with_suffix(".tacd")
    if not tacd_path.is_file():
        return None
    try:
        data = json.loads(tacd_path.read_text(encoding="utf-8"))
        return data.get("targets_hash_blake3")
    except (json.JSONDecodeError, OSError):
        return None


def _reference_hash(task: EvalTask) -> str | None:
    """Return the BLAKE3 hash from the task's reference.tacd, or None if absent."""
    tacd_path = task.task_dir / "reference.tacd"
    if not tacd_path.is_file():
        return None
    try:
        data = json.loads(tacd_path.read_text(encoding="utf-8"))
        return data.get("targets_hash_blake3")
    except (json.JSONDecodeError, OSError):
        return None


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
    turn_index: int | None = None,
) -> None:
    path = failure_dir(output_dir, run_id, task.task_id)
    if turn_index is not None:
        path = path / f"turn-{turn_index}"
    path.mkdir(parents=True, exist_ok=True)
    if raw_output is not None:
        (path / "raw-response.txt").write_text(raw_output, encoding="utf-8")
    if source is not None:
        (path / "generated.taca").write_text(source, encoding="utf-8")
    if diagnostics is not None:
        (path / "diagnostics.json").write_text(
            json.dumps(diagnostics, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )


def _dry_run_response(task: EvalTask) -> ModelResponse:
    reference = task.task_dir / "reference.taca"
    if not reference.is_file():
        return ModelResponse(
            text="```tacit\n0\n```",
            stop_reason="end_turn",
        )
    source = reference.read_text(encoding="utf-8").strip()
    return ModelResponse(text=f"```tacit\n{source}\n```", stop_reason="end_turn")


def _capture_turn_failure(
    *,
    output_dir: Path,
    run_id: str,
    task: EvalTask,
    raw_output: str | None,
    source: str | None,
    diagnostics: dict[str, Any] | None,
    turn_index: int,
    repair_mode: bool,
) -> None:
    capture_failure(
        output_dir=output_dir,
        run_id=run_id,
        task=task,
        raw_output=raw_output,
        source=source,
        diagnostics=diagnostics,
        turn_index=turn_index if repair_mode else None,
    )


def _retain_turn_output(
    *,
    output_dir: Path,
    run_id: str,
    task: EvalTask,
    raw_output: str,
    turn_index: int,
    repair_mode: bool,
) -> None:
    if repair_mode:
        out_path = output_dir / f"{run_id}.outputs" / task.task_id / f"turn-{turn_index}.txt"
    else:
        out_path = output_dir / f"{run_id}.outputs" / f"{task.task_id}.txt"
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(raw_output, encoding="utf-8")


def evaluate_turn(
    *,
    task: EvalTask,
    run_id: str,
    turn_index: int,
    prompt: str,
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
    repair_mode: bool,
) -> TurnResult:
    if dry_run:
        model_response = _dry_run_response(task)
        retries = 0
    else:
        assert api_key is not None
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
            _capture_turn_failure(
                output_dir=output_dir,
                run_id=run_id,
                task=task,
                raw_output=None,
                source=None,
                diagnostics=diagnostics,
                turn_index=turn_index,
                repair_mode=repair_mode,
            )
            return TurnResult(
                turn_index=turn_index,
                compile_pass=False,
                typecheck_pass=False,
                tests_pass=0,
                tests_total=0,
                generation_tokens=0,
                diagnostics=diagnostics,
                retries=retries,
                raw_output=None,
                source=None,
                failure_stage="api",
            )
        model_response = outcome.response

    raw_output = model_response.text
    if retain_outputs and not task.sealed:
        _retain_turn_output(
            output_dir=output_dir,
            run_id=run_id,
            task=task,
            raw_output=raw_output,
            turn_index=turn_index,
            repair_mode=repair_mode,
        )

    extraction = extract_tacit_source(model_response)
    if extraction.source is None:
        diagnostics = extraction.diagnostics
        _capture_turn_failure(
            output_dir=output_dir,
            run_id=run_id,
            task=task,
            raw_output=raw_output,
            source=None,
            diagnostics=diagnostics,
            turn_index=turn_index,
            repair_mode=repair_mode,
        )
        return TurnResult(
            turn_index=turn_index,
            compile_pass=False,
            typecheck_pass=False,
            tests_pass=0,
            tests_total=0,
            generation_tokens=0,
            diagnostics=diagnostics,
            retries=retries,
            raw_output=raw_output,
            source=None,
            failure_stage="extract",
        )

    generation_tokens = len(enc.encode(extraction.source))

    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
        generated = tmp / "generated.taca"
        binary = tmp / "generated"
        generated.write_text(extraction.source, encoding="utf-8")

        canonical_hash = _canonicalize_and_hash(tacit_bin, generated)
        reference_hash = _reference_hash(task)
        structural_equivalent = (
            canonical_hash is not None
            and reference_hash is not None
            and canonical_hash == reference_hash
        )

        grade = grade_source(
            tacit_bin=tacit_bin,
            task=task,
            source_path=generated,
            binary_path=binary,
        )

    diagnostics = grade.diagnostics
    if diagnostics is not None:
        _capture_turn_failure(
            output_dir=output_dir,
            run_id=run_id,
            task=task,
            raw_output=raw_output,
            source=extraction.source,
            diagnostics=diagnostics,
            turn_index=turn_index,
            repair_mode=repair_mode,
        )

    return TurnResult(
        turn_index=turn_index,
        compile_pass=grade.compile_pass,
        typecheck_pass=grade.typecheck_pass,
        tests_pass=grade.tests_pass,
        tests_total=grade.tests_total,
        generation_tokens=generation_tokens,
        diagnostics=diagnostics,
        retries=retries,
        raw_output=raw_output,
        source=extraction.source,
        failure_stage=grade.failure_stage,
        structural_equivalent=structural_equivalent,
        test_failures=grade.test_failures,
    )


def build_repair_metric(turns: list[TurnResult]) -> RepairMetric:
    first = turns[0]
    final = turns[-1]
    passing = next((turn for turn in turns if turn.tests_success), None)
    return RepairMetric(
        turns_used=passing.turn_index if passing is not None else None,
        first_pass_compile_pass=first.compile_pass,
        first_pass_typecheck_pass=first.typecheck_pass,
        first_pass_tests_pass=first.tests_success,
        final_compile_pass=final.compile_pass,
        final_typecheck_pass=final.typecheck_pass,
        final_tests_pass=final.tests_success,
        repair_success=not first.tests_success and final.tests_success,
        failure_stage_by_turn={
            f"turn-{turn.turn_index}": turn.failure_stage for turn in turns
        },
        generation_tokens_by_turn={
            f"turn-{turn.turn_index}": turn.generation_tokens for turn in turns
        },
        diagnostics_by_turn={
            f"turn-{turn.turn_index}": turn.diagnostics for turn in turns
        },
    )


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
    repair_turns: int,
) -> TaskMetric:
    started = time.monotonic()
    repair_mode = repair_turns > 0
    first_prompt = build_task_prompt(task.task_dir)
    turns = [
        evaluate_turn(
            task=task,
            run_id=run_id,
            turn_index=0,
            prompt=first_prompt,
            dry_run=dry_run,
            provider=provider,
            api_key=api_key,
            model=model,
            primer=primer,
            enc=enc,
            tacit_bin=tacit_bin,
            output_dir=output_dir,
            retain_outputs=retain_outputs,
            max_tokens=max_tokens,
            temperature=temperature,
            timeout_seconds=timeout_seconds,
            max_retries=max_retries,
            repair_mode=repair_mode,
        )
    ]

    for turn_index in range(1, repair_turns + 1):
        previous = turns[-1]
        if previous.tests_success or previous.failure_stage == "api":
            break
        repair_prompt = build_repair_prompt(task.task_dir, previous)
        turns.append(
            evaluate_turn(
                task=task,
                run_id=run_id,
                turn_index=turn_index,
                prompt=repair_prompt,
                dry_run=dry_run,
                provider=provider,
                api_key=api_key,
                model=model,
                primer=primer,
                enc=enc,
                tacit_bin=tacit_bin,
                output_dir=output_dir,
                retain_outputs=retain_outputs,
                max_tokens=max_tokens,
                temperature=temperature,
                timeout_seconds=timeout_seconds,
                max_retries=max_retries,
                repair_mode=repair_mode,
            )
        )

    primary = turns[0]
    repair = build_repair_metric(turns) if repair_mode else None
    duration_ms = int((time.monotonic() - started) * 1000)
    return TaskMetric(
        task_id=task.task_id,
        stdlib_dominated=task.stdlib_dominated,
        compile_pass=primary.compile_pass,
        typecheck_pass=primary.typecheck_pass,
        tests_pass=primary.tests_pass,
        tests_total=primary.tests_total,
        generation_tokens=primary.generation_tokens,
        python_baseline_tokens=task.python_baseline_tokens,
        diagnostics=primary.diagnostics,
        duration_ms=duration_ms,
        retries=sum(turn.retries for turn in turns),
        structural_equivalent=primary.structural_equivalent,
        repair=repair,
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


def repair_aggregates(
    tasks: list[TaskMetric],
    *,
    dry_run: bool,
    primer_tokens: int,
) -> dict[str, Any]:
    repair_tasks = [task for task in tasks if task.repair is not None]
    task_count = len(repair_tasks)
    initially_failed = [task for task in repair_tasks if not task.repair.first_pass_tests_pass]
    initially_failed_count = len(initially_failed)
    repair_successes = [task for task in initially_failed if task.repair.repair_success]
    final_passes = [task for task in repair_tasks if task.repair.final_tests_pass]
    final_compile_passes = [task for task in repair_tasks if task.repair.final_compile_pass]
    final_typecheck_passes = [task for task in repair_tasks if task.repair.final_typecheck_pass]

    def first_stage(task: TaskMetric) -> str | None:
        assert task.repair is not None
        return task.repair.failure_stage_by_turn.get("turn-0")

    compile_typecheck_initial = [
        task for task in initially_failed if first_stage(task) in {"typecheck", "compile"}
    ]
    compile_typecheck_recovered = [
        task
        for task in compile_typecheck_initial
        if task.repair is not None
        and task.repair.final_compile_pass
        and task.repair.final_typecheck_pass
    ]
    invalid_initial = [
        task
        for task in initially_failed
        if first_stage(task) in {"extract", "typecheck", "compile"}
    ]
    invalid_repaired = [
        task for task in invalid_initial if task.repair is not None and task.repair.final_tests_pass
    ]
    behavioral_initial = [task for task in initially_failed if first_stage(task) == "test"]
    behavioral_repaired = [
        task
        for task in behavioral_initial
        if task.repair is not None and task.repair.final_tests_pass
    ]

    model_calls = sum(
        len(task.repair.generation_tokens_by_turn)
        for task in repair_tasks
        if task.repair is not None
    )
    total_retries = sum(task.retries for task in repair_tasks)
    total_generation_tokens = sum(
        sum(task.repair.generation_tokens_by_turn.values())
        for task in repair_tasks
        if task.repair is not None
    )
    python_total = sum(task.python_baseline_tokens for task in repair_tasks)
    repair_primer_tokens_total = primer_tokens * model_calls
    repair_tacit_tokens_total = repair_primer_tokens_total + total_generation_tokens

    return {
        "task_count": task_count,
        "one_shot_task_pass_rate": _rate(task_count - initially_failed_count, task_count),
        "final_task_pass_rate": _rate(len(final_passes), task_count),
        "final_compile_pass_rate": _rate(len(final_compile_passes), task_count),
        "final_typecheck_pass_rate": _rate(len(final_typecheck_passes), task_count),
        "initially_failed_count": initially_failed_count,
        "repair_success_count": len(repair_successes),
        "repair_recovery_rate": _rate(len(repair_successes), initially_failed_count),
        "compile_typecheck_initial_failure_count": len(compile_typecheck_initial),
        "compile_typecheck_recovered_count": len(compile_typecheck_recovered),
        "compile_typecheck_recovery_rate": _rate(
            len(compile_typecheck_recovered), len(compile_typecheck_initial)
        ),
        "invalid_initial_failure_count": len(invalid_initial),
        "invalid_repaired_count": len(invalid_repaired),
        "invalid_recovery_rate": _rate(len(invalid_repaired), len(invalid_initial)),
        "behavioral_initial_failure_count": len(behavioral_initial),
        "behavioral_repaired_count": len(behavioral_repaired),
        "behavioral_recovery_rate": _rate(len(behavioral_repaired), len(behavioral_initial)),
        "average_model_calls_per_task": _rate(model_calls, task_count),
        "total_model_calls": model_calls,
        "total_generation_tokens": total_generation_tokens,
        "repair_primer_tokens_total": repair_primer_tokens_total,
        "repair_tacit_tokens_total": repair_tacit_tokens_total,
        "python_tokens_total": python_total,
        "repair_token_delta": (
            (repair_tacit_tokens_total - python_total) / python_total
            if python_total
            else 0.0
        ),
        "repair_primer_amortized_total": (
            primer_tokens + total_generation_tokens if task_count else 0
        ),
        "total_api_calls": 0 if dry_run else model_calls + total_retries,
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
    gates["passed_overall"] = all(gate["passed"] for gate in gates.values())

    repair = aggregates.get("repair")
    if repair is not None:
        repair_gates = {
            "repair_final_pass_rate_gate": {
                "applies": True,
                "metric": "final_task_pass_rate",
                "scope": "repair",
                "threshold": 0.70,
                "value": repair["final_task_pass_rate"],
                "passed": repair["final_task_pass_rate"] >= 0.70,
            },
            "repair_invalid_recovery_gate": {
                "applies": True,
                "metric": "invalid_recovery_rate",
                "scope": "repair",
                "threshold": 0.50,
                "value": repair["invalid_recovery_rate"],
                "passed": repair["invalid_recovery_rate"] >= 0.50,
            },
            "repair_behavioral_recovery_gate": {
                "applies": True,
                "metric": "behavioral_recovery_rate",
                "scope": "repair",
                "threshold": 1.0 / 3.0,
                "value": repair["behavioral_recovery_rate"],
                "passed": repair["behavioral_recovery_rate"] >= 1.0 / 3.0,
            },
        }
        gates.update(repair_gates)
        gates["repair_promising_overall"] = all(
            gate["passed"] for gate in repair_gates.values()
        )

    return gates


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
        "passed_overall": False,
    }


def build_metrics(
    *,
    run_id: str,
    track: Track,
    scope: Scope,
    result_label: ResultLabel,
    provider: Provider,
    model_id: str,
    primer_hash: str,
    primer_tokens: int,
    tasks: list[TaskMetric],
    repair_turns: int = 0,
    dry_run: bool = False,
) -> dict[str, Any]:
    aggregates = primary_aggregates(tasks, primer_tokens)
    if repair_turns > 0:
        aggregates["repair"] = repair_aggregates(
            tasks,
            dry_run=dry_run,
            primer_tokens=primer_tokens,
        )
    gates = (
        primary_gates(aggregates)
        if track == "primary" and result_label == "core-language"
        else reporting_only_gates()
    )
    return {
        "schema_version": "p3.0",
        "run_id": run_id,
        "track": track,
        "scope": scope,
        "result_label": result_label,
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
    if metrics.get("result_label") not in {None, "core-language", "library-mediated"}:
        raise EvalError("metrics result_label is invalid")
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
    result_label: ResultLabel,
    task_ids: list[str],
    primer_hash: str,
    primer_tokens: int,
    temperature: float,
    max_tokens: int,
    started_at: datetime,
    completed_at: datetime,
    dry_run: bool,
    tacit_bin: Path,
    repair_turns: int = 0,
) -> dict[str, Any]:
    sha = _git_sha()
    data = {
        "run_id": run_id,
        "harness_git_sha": sha,
        "corpus_git_sha": sha,
        "primer_blake3": primer_hash,
        "primer_token_count": primer_tokens,
        "model_id": model_id,
        "provider": provider,
        "result_label": result_label,
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
        "tacit_binary": _binary_metadata(tacit_bin),
    }
    if repair_turns > 0:
        data["repair_turns"] = repair_turns
    return data


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
        "--result-label",
        choices=["core-language", "library-mediated"],
        default="core-language",
        help=(
            "result interpretation label; library-mediated runs are reported "
            "separately and do not apply primary Phase 3 gates"
        ),
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
    parser.add_argument(
        "--repair-turns",
        type=int,
        default=0,
        help="number of repair turns after the initial generation (0-2; default: 0)",
    )
    parser.add_argument("--max-tokens", type=int, default=DEFAULT_MAX_TOKENS)
    parser.add_argument("--temperature", type=float, default=DEFAULT_TEMPERATURE)
    parser.add_argument("--timeout-seconds", type=int, default=DEFAULT_TIMEOUT_SECONDS)
    parser.add_argument("--max-retries", type=int, default=DEFAULT_MAX_RETRIES)
    return parser.parse_args(argv)


def run(argv: list[str] | None = None) -> int:
    args = parse_args(argv)

    if args.repair_turns < 0 or args.repair_turns > 2:
        raise EvalError("--repair-turns must be between 0 and 2")
    if args.repair_turns > 0 and args.include_sealed:
        raise EvalError("--repair-turns is open-only; sealed repair feedback policy is not settled")

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

    repair_label = f" repair_turns={args.repair_turns}" if args.repair_turns > 0 else ""
    print(
        f"corpus-eval run_id={run_id} scope={scope} track={track} "
        f"result_label={args.result_label} provider={provider} model={model} "
        f"tasks={len(tasks)}{repair_label}"
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
            repair_turns=args.repair_turns,
        )
        task_metrics.append(metric)
        ok = (
            metric.repair.final_tests_pass
            if metric.repair is not None
            else metric.tests_pass == metric.tests_total and metric.tests_total > 0
        )
        print("." if ok else "F", end="", flush=True)
    print()

    completed_at = _utc_now()
    metrics = build_metrics(
        run_id=run_id,
        track=track,
        scope=scope,
        result_label=args.result_label,
        provider=provider,
        model_id=model,
        primer_hash=primer_hash,
        primer_tokens=primer_tokens,
        tasks=task_metrics,
        repair_turns=args.repair_turns,
        dry_run=args.dry_run,
    )
    validate_metrics_shape(metrics)
    run_metadata = build_run_metadata(
        run_id=run_id,
        provider=provider,
        model_id=model,
        scope=scope,
        result_label=args.result_label,
        task_ids=[task.task_id for task in tasks],
        primer_hash=primer_hash,
        primer_tokens=primer_tokens,
        temperature=args.temperature,
        max_tokens=args.max_tokens,
        started_at=started_at,
        completed_at=completed_at,
        dry_run=args.dry_run,
        tacit_bin=args.tacit_bin,
        repair_turns=args.repair_turns,
    )

    run_path = args.output_dir / f"{run_id}.run.json"
    metrics_path = args.output_dir / f"{run_id}.metrics.json"
    write_json(run_path, run_metadata)
    write_json(metrics_path, metrics)

    full = metrics["aggregates"]["full"]
    print(f"wrote {display_path(run_path)} and {display_path(metrics_path)}")
    if args.repair_turns > 0:
        repair = metrics["aggregates"]["repair"]
        print(
            f"one_shot_tasks_passed={repair['one_shot_task_pass_rate']:.3f} "
            f"final_tasks_passed={repair['final_task_pass_rate']:.3f} "
            f"repair_recovery={repair['repair_recovery_rate']:.3f} "
            f"avg_model_calls={repair['average_model_calls_per_task']:.2f}"
        )
    else:
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
