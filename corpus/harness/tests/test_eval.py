from __future__ import annotations

import re
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


def test_load_tasks_accepts_numeric_selector() -> None:
    enc = tiktoken.get_encoding("o200k_base")

    tasks = corpus_eval.load_tasks(
        scope="open",
        selectors={"001"},
        enc=enc,
    )

    assert [task.task_id for task in tasks] == ["arithmetic/001-sum-to-n"]


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
