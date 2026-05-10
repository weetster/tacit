from __future__ import annotations

import json
from pathlib import Path

from tacit_corpus import phase5_eval


def test_load_manifest_selects_subset(tmp_path: Path) -> None:
    manifest = tmp_path / "manifest.json"
    task_dir = tmp_path / "r1"
    task_dir.mkdir()
    (task_dir / "main.tac").write_text("(int 0)", encoding="utf-8")
    (task_dir / "main.taca").write_text("0", encoding="utf-8")
    manifest.write_text(
        json.dumps(
            {
                "version": 1,
                "tasks": [
                    {
                        "id": "r1",
                        "class": "repair",
                        "evaluation_kind": "program",
                        "canonical_source": "r1/main.tac",
                        "authoring_input": "r1/main.taca",
                        "prompt": "fix it",
                        "expected_exit": 0,
                        "expected_stdout": "",
                    }
                ],
            }
        ),
        encoding="utf-8",
    )

    tasks = phase5_eval.load_manifest(manifest, {"r1"})

    assert [task.task_id for task in tasks] == ["r1"]
    assert tasks[0].expected_exit == 0


def test_grade_explanation_requires_manifest_terms() -> None:
    task = phase5_eval.Phase5Task(
        task_id="x1",
        task_class="explanation",
        evaluation_kind="explanation",
        canonical_source=Path("/tmp/x1.tac"),
        authoring_input=Path("/tmp/x1.taca"),
        prompt="Explain the failure.",
        required_substrings=("record", "field", "total"),
    )

    failing = phase5_eval.grade_explanation(task, "This mentions only the record.")
    passing = phase5_eval.grade_explanation(
        task, "The record does not contain the field total."
    )

    assert not failing.passed
    assert failing.missing_substrings == ("field", "total")
    assert passing.passed


def test_build_program_repair_prompt_includes_feedback() -> None:
    task = phase5_eval.Phase5Task(
        task_id="r1",
        task_class="repair",
        evaluation_kind="program",
        canonical_source=Path("/tmp/r1.tac"),
        authoring_input=Path("/tmp/r1.taca"),
        prompt="Fix the program.",
        expected_exit=9,
        expected_stdout="",
    )
    turn = phase5_eval.TurnRecord(
        turn_index=0,
        prompt="first prompt",
        raw_output="```tacit\n0\n```",
        retries=0,
        generation_tokens=1,
        failure_stage="run",
        diagnostics={"schema_version": "p2.0", "errors": [{"message": "bad exit"}]},
        source="0\n",
        inspection_text="let x = 0 in x",
    )

    prompt = phase5_eval.build_program_repair_prompt(task, turn)

    assert "Failure stage: run" in prompt
    assert "bad exit" in prompt
    assert "Inspection view:" in prompt
    assert "```tacit" in prompt
