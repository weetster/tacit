# 0001 — Target tokenizer for authoring view optimization

**Status:** Accepted
**Date:** 2026-04-20
**Phase:** 0, Stage 1

## Context

Tacit's authoring view is optimized for token density under a specific BPE tokenizer. The choice of tokenizer shapes Q1 (authoring view format) — every candidate format must be measured against some tokenizer to decide which wins.

The parent plan (tacit-plan.md § Open Questions, Q7) framed the choice as: optimize for a specific model family (Claude's tokenizer, GPT's tiktoken) or aim for tokenizer-agnostic density. A specific target yields sharper wins but creates a dependency.

Practical constraint: current development uses a Claude Code subscription only — no Anthropic API access. Claude's tokenizer is not distributed as a local library; batch measurement requires the API's `count_tokens` endpoint, which is unavailable.

## Decision

**Use tiktoken (`cl100k_base` or `o200k_base`) as the primary measurement tool for authoring-view token density. Treat Claude's tokenizer as a post-hoc validation target if API access materializes later.**

This is a deliberate inversion of the parent plan's original framing ("Claude tokenizer primary, tiktoken secondary"). The rationale is pragmatic: tiktoken is open-source, local, and scriptable; Claude's tokenizer currently is not accessible in this setup.

**Decision rule for Q1 candidates:** a candidate wins only if it beats rivals by a comfortable margin under tiktoken (roughly ≥10%). Margins of 2–5% are within plausible noise between modern BPE tokenizers and do not justify commitment.

**Cross-check:** optionally validate close calls against a second BPE tokenizer with a different vocabulary composition (e.g. HuggingFace's Llama 3 tokenizer). Agreement across two tokenizers raises confidence that the winner generalizes.

## Alternatives considered

- **Claude tokenizer as primary, via API `count_tokens`.** Ideal but blocked on API access. Revisit if access is added.
- **Tokenizer-agnostic design.** Loses an estimated 10–20% density in exchange for portability we don't currently need. The plan is explicit that Tacit targets Claude-class models; portability is not a v0 goal.
- **Byte count as a proxy.** Correlated with token count but too coarse; BPE merges make character efficiency and token efficiency diverge (a rare character sequence is byte-cheap but token-expensive).
- **Ask Claude Code to estimate token counts.** Unreliable — the model estimates rather than counts. Not suitable for comparative measurement.
- **Use Claude Code's `/context` command to measure snippets.** Too manual and noisy to scale to the 20-node reference AST comparison Q1 requires.

## Consequences

- Q1 prototyping work uses tiktoken; no API dependency blocks progress.
- The authoring view may be marginally suboptimal under Claude's actual tokenizer. This is acceptable given (a) relative comparisons transfer well between modern BPE tokenizers, and (b) the ≥10% margin rule avoids committing to noise-level wins.
- If API access is added later, the authoring view should be re-measured under Claude's tokenizer. A large gap would trigger a new ADR; a small gap is confirmation.
- The Q1 prototyping harness should be written so swapping tokenizers is a one-line change, not a rewrite. This keeps the re-measurement cost low.
- The plan's Phase 3 token-savings metric ("end-to-end token usage at least 30% lower than equivalent Python") will be measured under whatever tokenizer is available at Phase 3 time. If that is tiktoken, report it as such; do not claim Claude-specific numbers without Claude-specific measurement.
