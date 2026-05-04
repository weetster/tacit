from __future__ import annotations

import re
import subprocess
from datetime import UTC, datetime, timedelta

import tiktoken
import pytest

from tacit_corpus import eval as corpus_eval


def test_extract_tacit_source_accepts_single_block() -> None:
    response = corpus_eval.ModelResponse(
        text="Here is the program:\n```tacit\n0\n```\n",
        stop_reason="end_turn",
    )

    out = corpus_eval.extract_tacit_source(response)

    assert out.diagnostics is None
    assert out.source == "0\n"


def test_extract_tacit_source_rejects_missing_block() -> None:
    response = corpus_eval.ModelResponse(text="0", stop_reason="end_turn")

    out = corpus_eval.extract_tacit_source(response)

    assert out.source is None
    assert out.diagnostics is not None
    assert out.diagnostics["errors"][0]["kind"] == "extraction-error"


def test_extract_tacit_source_rejects_truncation() -> None:
    response = corpus_eval.ModelResponse(
        text="```tacit\n0\n```",
        stop_reason="max_tokens",
    )

    out = corpus_eval.extract_tacit_source(response)

    assert out.source is None
    assert out.diagnostics is not None
    assert "max_tokens" in out.diagnostics["errors"][0]["message"]


def test_compare_test_output_handles_non_utf8_stdout() -> None:
    proc = subprocess.CompletedProcess(
        args=["ref"],
        returncode=0,
        stdout=b"\xa9",
        stderr=b"",
    )

    ok, message = corpus_eval._compare_test_output(
        proc,
        {"name": "sample", "stdout": ""},
        sealed=False,
    )

    assert not ok
    assert "b'\\xa9'" in message


def test_repair_prompt_limits_open_test_feedback(tmp_path) -> None:
    (tmp_path / "task.md").write_text("# Task\n\nDo the thing.\n", encoding="utf-8")
    previous = corpus_eval.TurnResult(
        turn_index=0,
        compile_pass=True,
        typecheck_pass=True,
        tests_pass=0,
        tests_total=3,
        generation_tokens=12,
        diagnostics=corpus_eval.diagnostic_envelope("test-failure", "failed"),
        retries=0,
        raw_output="```tacit\n0\n```",
        source="0\n",
        failure_stage="test",
        test_failures=(
            corpus_eval.TestFailureDetail("case-1", "a\n", "A\n", "x\n"),
            corpus_eval.TestFailureDetail("case-2", "b\n", "B\n", "y\n"),
            corpus_eval.TestFailureDetail("case-3", "c\n", "C\n", "z\n"),
        ),
    )

    prompt = corpus_eval.build_repair_prompt(tmp_path, previous)

    assert "Failure stage: test" in prompt
    assert "case-1" in prompt
    assert "case-2" in prompt
    assert "case-3" not in prompt
    assert "1 additional failing case(s) omitted" in prompt
    assert "Do not include the sidecar" in prompt


def test_repair_prompt_classifies_runtime_and_format_failures(tmp_path) -> None:
    (tmp_path / "task.md").write_text("# Task\n\nDo the thing.\n", encoding="utf-8")
    previous = corpus_eval.TurnResult(
        turn_index=0,
        compile_pass=True,
        typecheck_pass=True,
        tests_pass=0,
        tests_total=5,
        generation_tokens=12,
        diagnostics=corpus_eval.diagnostic_envelope("test-failure", "failed"),
        retries=0,
        raw_output="```tacit\n0\n```",
        source="0\n",
        failure_stage="test",
        test_failures=(
            corpus_eval.TestFailureDetail(
                "empty",
                "",
                "\n",
                "0\n",
            ),
            corpus_eval.TestFailureDetail(
                "spaced-1",
                "a\n",
                "a b\n",
                "ab\n",
            ),
            corpus_eval.TestFailureDetail(
                "spaced-2",
                "c\n",
                "c: d\n",
                "cd\n",
            ),
            corpus_eval.TestFailureDetail(
                "crash",
                "x\n",
                "x\n",
                "",
                "nonzero exit -11",
            ),
            corpus_eval.TestFailureDetail(
                "bad-status",
                "y\n",
                "y\n",
                "",
                "nonzero exit 1",
            ),
        ),
    )

    prompt = corpus_eval.build_repair_prompt(tmp_path, previous)

    assert "Generic failure classification:" in prompt
    assert "segmentation fault" in prompt
    assert "exit 1 with empty stderr" in prompt
    assert "empty-input formatting bug" in prompt
    assert "output formatting bug" in prompt


def test_repair_prompt_classifies_typecheck_and_parse_failures(tmp_path) -> None:
    (tmp_path / "task.md").write_text("# Task\n\nDo the thing.\n", encoding="utf-8")
    previous = corpus_eval.TurnResult(
        turn_index=0,
        compile_pass=False,
        typecheck_pass=False,
        tests_pass=0,
        tests_total=0,
        generation_tokens=12,
        diagnostics=corpus_eval.diagnostic_envelope(
            "unresolved-type",
            "unknown type 'token-index-any'; expected 'else' but got RBrace",
        ),
        retries=0,
        raw_output="```tacit\n0\n```",
        source="0\n",
        failure_stage="typecheck",
    )

    prompt = corpus_eval.build_repair_prompt(tmp_path, previous)

    assert "unknown primitive or primitive spelling issue" in prompt
    assert "keep the leading @" in prompt
    assert "Tacit if syntax issue" in prompt
    assert "requires both then and else branches" in prompt


def test_build_repair_metric_records_turns() -> None:
    first = corpus_eval.TurnResult(
        turn_index=0,
        compile_pass=False,
        typecheck_pass=False,
        tests_pass=0,
        tests_total=0,
        generation_tokens=0,
        diagnostics=corpus_eval.diagnostic_envelope("extraction-error", "missing block"),
        retries=0,
        raw_output="no block",
        source=None,
        failure_stage="extract",
    )
    second = corpus_eval.TurnResult(
        turn_index=1,
        compile_pass=True,
        typecheck_pass=True,
        tests_pass=2,
        tests_total=2,
        generation_tokens=4,
        diagnostics=None,
        retries=0,
        raw_output="```tacit\n0\n```",
        source="0\n",
        failure_stage=None,
    )

    metric = corpus_eval.build_repair_metric([first, second])

    assert metric.turns_used == 1
    assert metric.repair_success
    assert metric.failure_stage_by_turn == {"turn-0": "extract", "turn-1": None}
    assert metric.generation_tokens_by_turn == {"turn-0": 0, "turn-1": 4}
    assert metric.diagnostics_by_turn["turn-1"] is None


def test_evaluate_turn_captures_repair_failure_in_turn_directory(
    tmp_path, monkeypatch: pytest.MonkeyPatch
) -> None:
    task = corpus_eval.EvalTask(
        task_id="arithmetic/001-sum-to-n",
        task_dir=tmp_path,
        sealed=False,
        stdlib_dominated=False,
        python_baseline_tokens=0,
    )

    def fake_call_model_with_retries(
        *,
        provider: corpus_eval.Provider,
        api_key: str,
        model: str,
        primer: str,
        prompt: str,
        max_tokens: int,
        temperature: float,
        timeout_seconds: int,
        max_retries: int,
    ) -> corpus_eval.ModelOutcome:
        return corpus_eval.ModelOutcome(
            response=corpus_eval.ModelResponse(text="no fenced block", stop_reason="end_turn"),
            retries=0,
        )

    monkeypatch.setattr(corpus_eval, "call_model_with_retries", fake_call_model_with_retries)

    result = corpus_eval.evaluate_turn(
        task=task,
        run_id="run",
        turn_index=0,
        prompt="prompt",
        dry_run=False,
        provider="anthropic",
        api_key="key",
        model="claude-sonnet-4-6",
        primer="primer",
        enc=tiktoken.get_encoding("o200k_base"),
        tacit_bin=tmp_path / "tacit",
        output_dir=tmp_path,
        retain_outputs=False,
        max_tokens=10,
        temperature=0,
        timeout_seconds=1,
        max_retries=0,
        repair_mode=True,
    )

    assert result.failure_stage == "extract"
    assert (tmp_path / "failures/run/arithmetic/001-sum-to-n/turn-0/diagnostics.json").is_file()


def test_primary_aggregates_are_primer_inclusive() -> None:
    tasks = [
        corpus_eval.TaskMetric(
            task_id="arithmetic/001-sum-to-n",
            stdlib_dominated=False,
            compile_pass=True,
            typecheck_pass=True,
            tests_pass=2,
            tests_total=2,
            generation_tokens=4,
            python_baseline_tokens=20,
            diagnostics=None,
            duration_ms=1,
            retries=0,
        ),
        corpus_eval.TaskMetric(
            task_id="algorithms/031-binary-search",
            stdlib_dominated=True,
            compile_pass=False,
            typecheck_pass=False,
            tests_pass=0,
            tests_total=0,
            generation_tokens=6,
            python_baseline_tokens=30,
            diagnostics=corpus_eval.diagnostic_envelope("compile-error", "nope"),
            duration_ms=1,
            retries=1,
        ),
    ]

    aggregates = corpus_eval.primary_aggregates(tasks, primer_tokens=10)

    assert aggregates["full"]["task_count"] == 2
    assert aggregates["full"]["tacit_tokens_total"] == 30
    assert aggregates["full"]["python_tokens_total"] == 50
    assert aggregates["full"]["token_delta"] == -0.4
    assert aggregates["full"]["primer_amortized_total"] == 20
    assert aggregates["non_stdlib_dominated"]["tests_pass_rate"] == 1.0


def test_library_mediated_metrics_are_reporting_only() -> None:
    task = corpus_eval.TaskMetric(
        task_id="collections/025-partition-eo",
        stdlib_dominated=False,
        compile_pass=True,
        typecheck_pass=True,
        tests_pass=2,
        tests_total=2,
        generation_tokens=4,
        python_baseline_tokens=20,
        diagnostics=None,
        duration_ms=1,
        retries=0,
    )

    metrics = corpus_eval.build_metrics(
        run_id="run",
        track="primary",
        scope="open",
        result_label="library-mediated",
        provider="anthropic",
        model_id="claude-sonnet-4-6",
        primer_hash="0" * 64,
        primer_tokens=10,
        tasks=[task],
    )

    assert metrics["result_label"] == "library-mediated"
    assert metrics["gates"]["primary_pass_rate_gate"]["applies"] is False
    assert metrics["gates"]["passed_overall"] is False
    corpus_eval.validate_metrics_shape(metrics)


def test_repair_aggregates_report_final_recovery() -> None:
    recovered = corpus_eval.TaskMetric(
        task_id="algorithms/035-bubble-sort",
        stdlib_dominated=False,
        compile_pass=False,
        typecheck_pass=True,
        tests_pass=0,
        tests_total=0,
        generation_tokens=3,
        python_baseline_tokens=20,
        diagnostics=corpus_eval.diagnostic_envelope("compile-error", "failed"),
        duration_ms=1,
        retries=0,
        repair=corpus_eval.RepairMetric(
            turns_used=1,
            first_pass_compile_pass=False,
            first_pass_typecheck_pass=True,
            first_pass_tests_pass=False,
            final_compile_pass=True,
            final_typecheck_pass=True,
            final_tests_pass=True,
            repair_success=True,
            failure_stage_by_turn={"turn-0": "compile", "turn-1": None},
            generation_tokens_by_turn={"turn-0": 3, "turn-1": 5},
            diagnostics_by_turn={
                "turn-0": corpus_eval.diagnostic_envelope("compile-error", "failed"),
                "turn-1": None,
            },
        ),
    )
    first_passed = corpus_eval.TaskMetric(
        task_id="arithmetic/001-sum-to-n",
        stdlib_dominated=False,
        compile_pass=True,
        typecheck_pass=True,
        tests_pass=2,
        tests_total=2,
        generation_tokens=4,
        python_baseline_tokens=20,
        diagnostics=None,
        duration_ms=1,
        retries=0,
        repair=corpus_eval.RepairMetric(
            turns_used=0,
            first_pass_compile_pass=True,
            first_pass_typecheck_pass=True,
            first_pass_tests_pass=True,
            final_compile_pass=True,
            final_typecheck_pass=True,
            final_tests_pass=True,
            repair_success=False,
            failure_stage_by_turn={"turn-0": None},
            generation_tokens_by_turn={"turn-0": 4},
            diagnostics_by_turn={"turn-0": None},
        ),
    )

    aggregates = corpus_eval.repair_aggregates(
        [recovered, first_passed],
        dry_run=False,
        primer_tokens=10,
    )

    assert aggregates["one_shot_task_pass_rate"] == 0.5
    assert aggregates["final_task_pass_rate"] == 1.0
    assert aggregates["repair_recovery_rate"] == 1.0
    assert aggregates["compile_typecheck_recovery_rate"] == 1.0
    assert aggregates["average_model_calls_per_task"] == 1.5
    assert aggregates["total_model_calls"] == 3
    assert aggregates["total_generation_tokens"] == 12
    assert aggregates["repair_primer_tokens_total"] == 30
    assert aggregates["repair_tacit_tokens_total"] == 42
    assert aggregates["python_tokens_total"] == 40
    assert aggregates["repair_token_delta"] == 0.05
    assert aggregates["repair_primer_amortized_total"] == 22
    assert aggregates["total_api_calls"] == 3

    gates = corpus_eval.primary_gates(
        {
            "full": {"tests_pass_rate": 0.5, "token_delta": 0.0},
            "non_stdlib_dominated": {"token_delta": 0.0},
            "repair": aggregates,
        }
    )

    assert not gates["passed_overall"]
    assert gates["repair_final_pass_rate_gate"]["passed"]
    assert gates["repair_invalid_recovery_gate"]["passed"]
    assert not gates["repair_behavioral_recovery_gate"]["passed"]
    assert not gates["repair_promising_overall"]


def test_load_tasks_accepts_numeric_selector() -> None:
    enc = tiktoken.get_encoding("o200k_base")

    tasks = corpus_eval.load_tasks(
        scope="open",
        selectors={"001"},
        enc=enc,
    )

    assert [task.task_id for task in tasks] == ["arithmetic/001-sum-to-n"]


def test_require_api_key_loads_cwd_dotenv(tmp_path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.chdir(tmp_path)
    (tmp_path / ".env").write_text("ANTHROPIC_API_KEY=from-dotenv\n", encoding="utf-8")
    monkeypatch.delenv("ANTHROPIC_API_KEY", raising=False)

    assert corpus_eval._require_api_key("anthropic") == "from-dotenv"


def test_uuid7_shape() -> None:
    run_id = corpus_eval._uuid7()

    assert re.fullmatch(
        r"[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}",
        run_id,
    )


def test_parse_retry_after_accepts_seconds() -> None:
    assert corpus_eval._parse_retry_after("3.5") == 3.5


def test_parse_retry_after_accepts_http_date() -> None:
    future = datetime.now(UTC) + timedelta(seconds=10)
    value = future.strftime("%a, %d %b %Y %H:%M:%S GMT")

    delay = corpus_eval._parse_retry_after(value)

    assert delay is not None
    assert delay == pytest.approx(10, abs=2)


def test_call_model_with_retries_honors_retry_after(monkeypatch: pytest.MonkeyPatch) -> None:
    calls: list[str] = []
    sleeps: list[float] = []

    def fake_anthropic_call(**_: object) -> corpus_eval.ModelResponse:
        calls.append("call")
        if len(calls) == 1:
            raise corpus_eval.ApiStatusError(
                429,
                {"error": "rate limited"},
                headers={"Retry-After": "7"},
            )
        return corpus_eval.ModelResponse(text="```tacit\n0\n```", stop_reason="end_turn")

    monkeypatch.setattr(corpus_eval, "_anthropic_call", fake_anthropic_call)
    monkeypatch.setattr(corpus_eval.time, "sleep", lambda seconds: sleeps.append(seconds))

    outcome = corpus_eval.call_model_with_retries(
        provider="anthropic",
        api_key="key",
        model="claude-sonnet-4-6",
        primer="primer",
        prompt="prompt",
        max_tokens=10,
        temperature=0,
        timeout_seconds=1,
        max_retries=1,
    )

    assert outcome.response is not None
    assert outcome.response.text == "```tacit\n0\n```"
    assert outcome.retries == 1
    assert sleeps == [7]
    assert len(calls) == 2
