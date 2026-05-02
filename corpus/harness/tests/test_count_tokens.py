from __future__ import annotations

from tacit_corpus import count_tokens


def test_render_report_compares_current_and_stdlib_tacit_refs() -> None:
    rows = [
        count_tokens.TokenRow(
            task="tasks/collections/025-partition-eo",
            stdlib_dominated=False,
            python=100,
            tacit=200,
            stdlib_tacit=125,
            rust=150,
        ),
        count_tokens.TokenRow(
            task="tasks/io/055-sort-lines",
            stdlib_dominated=True,
            python=40,
            tacit=80,
            stdlib_tacit=None,
            rust=70,
        ),
    ]

    report = count_tokens.render_report(rows, "open")

    assert "stdlib" in report
    assert "std/tac" in report
    assert "tasks/collections/025-partition-eo" in report
    assert "    100    200     125" in report
    assert "+100%" in report
    assert "+25%" in report
    assert "-38%" in report
    assert "TOTAL (open, Stdlib Tacit refs all; n=1)" in report
    assert "TOTAL (open, Stdlib vs Tacit paired all; n=1)" in report
