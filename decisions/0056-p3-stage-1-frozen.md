# 0056 — Phase 3 Stage 1 frozen

**Status:** Accepted
**Date:** 2026-04-28
**Phase:** 3, Stage 1 (exit)
**Closes:** [phase-3-plan.md § Stage 1](../plans/phase-3-plan.md)
**Artifacts frozen by this ADR:**
- ADRs [0047](0047-p3-stdlib-expansion-surface.md), [0048](0048-p3-tacit-idiom-rules.md), [0049](0049-p3-examples-layout-contamination.md), [0050](0050-p3-primer-scope.md), [0051](0051-p3-tacit-token-rule.md), [0052](0052-p3-eval-model-contract.md), [0053](0053-p3-maintenance-subtrack.md), [0054](0054-p3-cross-family.md), [0055](0055-p3-metrics-schema.md) — the Phase 3 Stage 1 spec surface.
- The Q-P3-N → ADR mapping at [`plans/phase-3-plan.md` § Stage 1](../plans/phase-3-plan.md), recording the resolution of every Stage 1 open question.

## Context

Stage 1 of [phase-3-plan.md](../plans/phase-3-plan.md) was scoped as "ADRs only. No production code, no primer prose, no Tacit reference solutions." Its purpose was to close every spec question that Stages 2–11 would otherwise have to bikeshed mid-implementation. Nine open questions (Q-P3-1 through Q-P3-9) covered the stdlib expansion surface, Tacit-Lite reference idiom rules, the `examples/phase-3/` layout and primer-contamination boundary, primer scope and structure, the Tacit-Lite token-count rule, the eval-harness model invocation contract, the maintenance/edit/repair sub-track, the cross-family sub-track, and the Phase 3 metrics JSON schema.

All nine landed as Accepted ADRs on 2026-04-28. Unlike Phase 2 Stage 1, no canonical-format amendments were required: every Phase 3 question is settled within the existing Phase 1–2 surface (typed `@name` primitives per [ADR 0028](0028-phase-1-libc-call-surface.md), the four-atom effect lattice per [ADR 0035](0035-p2-effect-set-canonical.md), the structured diagnostic envelope per [ADR 0041](0041-p2-structured-error-format.md)). No new canonical node kinds, no new diag-ids, no new test vectors land with Stage 1.

## Decision

**Phase 3 Stage 1 is frozen.** The nine ADRs above are the Phase 3 spec surface that Stages 2–11 build against. Further amendments require new ADRs and are treated as spec bugs against this surface, not scope renegotiation — the same discipline imposed on Phase 0 frozen artifacts by [ADR 0013](0013-canonical-text-format-frozen.md), Phase 1 by [ADR 0033](0033-phase-1-frozen.md), Phase 2 Stage 1 by [ADR 0044](0044-p2-stage-1-frozen.md), and Phase 2 as a whole by [ADR 0046](0046-p2-stage-5-frozen.md).

Concretely:

1. **The Q-P3-N → ADR mapping is final.**

   | Q-P3-N | Subject                                                    | ADR  |
   |--------|------------------------------------------------------------|------|
   | Q-P3-1 | Stdlib expansion surface for corpus coverage               | 0047 |
   | Q-P3-2 | Tacit-Lite reference-solution idiom rules                  | 0048 |
   | Q-P3-3 | `examples/phase-3/` layout and primer-contamination boundary | 0049 |
   | Q-P3-4 | Primer scope, structure, and budget                        | 0050 |
   | Q-P3-5 | Tacit-Lite token-count rule                                | 0051 |
   | Q-P3-6 | Eval-harness model invocation contract                     | 0052 |
   | Q-P3-7 | Maintenance / edit / repair sub-track scope                | 0053 |
   | Q-P3-8 | Cross-family sub-track scope                               | 0054 |
   | Q-P3-9 | Phase 3 metrics JSON schema                                | 0055 |

2. **Eight new `@name` primitives are admitted to the codegen surface** per ADR 0047, across three new categories (PARSE, FORMAT, MEM) and one extension (STACK-ALLOC):
   - `@parse-i64` (PARSE, `{}`)
   - `@fmt-i64` (FORMAT, `{Mut}`)
   - `@buf-get`, `@buf-set`, `@buf-copy`, `@buf-eq`, `@scan-byte` (MEM)
   - `@buf-alloc-dyn` (STACK-ALLOC extension)

   The categories extend [ADR 0028](0028-phase-1-libc-call-surface.md) additively. No existing primitive name or category is repurposed. The signatures and effect sets are pinned by ADR 0047's table.

3. **One new monomorphic type is admitted: `Buf` (dynamic, no size index).** Per ADR 0047, `Buf N <: Buf` (a fixed-size buffer is a subtype of the dynamic-size handle). The type is represented in canonical form as `(sym Buf)` — no new tag kind. The relationship to the [ADR 0038](0038-p2-writable-buffer.md) `Buf N` is by construction: every primitive that accepts `Buf` also accepts `Buf N`.

4. **No canonical-format amendments are required.** ADR 0047 explicitly notes "no new canonical node kinds, no changes to the canonical lexical rules"; ADRs 0048–0055 are scoped to corpus authoring rules, primer authorship, harness behavior, and metrics output and do not touch the canonical surface. Consequently Phase 3 Stage 1 lands **no new test vectors** under [`plans/test-vectors/`](../plans/test-vectors/) — the Stage 1 exit gate's "or note in the ADR if no new canonical syntax is required" branch applies.

5. **One new diagnostic kind is admitted to the metrics envelope: `test-failure`** (ADR 0055). It exists only inside Phase 3 metrics files when a compiled program failed a test case; it is **not** emitted by `tacit-typecheck` or `tacit-cli`. The Phase 2 diagnostic-kind set (ADR 0041) is unchanged.

6. **Stdlib expansion is bounded by ADR 0047.** Stage 4–6 references that need a primitive not listed in ADR 0047 are a Q-P3-1 follow-up ADR and a Stage 2 patch, not an in-line addition. The known gap (general hash map for task 056-unique-lines per ADR 0047 § Acknowledged gaps) is documented and **deferred to Phase 7** rather than added to the surface.

7. **Tacit-Full features remain explicitly out of scope.** Phase 3 introduces no language features beyond the typed `Buf` extension: no row polymorphism, no user-defined effects, no effect handlers, no type classes, no heap-allocated buffers, no general hash maps, no whole-module inference, no dynamic dispatch beyond `@name`. Design pressure for any of these during Stage 2+ is a Phase 7 signal — defer, do not extend Phase 3.

8. **Eval-only specs do not change Phase 1–2 frozen artifacts.** ADRs 0048–0055 govern primer authorship, corpus reference rules, harness invocation, maintenance/cross-family runs, and metrics output. They consume the Phase 1–2 surface but amend nothing in it. The `tacit compile` and `tacit check` CLIs, the canonical text format, the sidecar format, the diagnostic envelope, and the smoke corpus all remain frozen as of [ADR 0046](0046-p2-stage-5-frozen.md).

9. **The 30%-reduction gate is fully specified.** Combined with [ADR 0019](0019-corpus-idiom-rules.md) (Python rule), [ADR 0021](0021-corpus-stdlib-dominance-reporting.md) (three-way split), ADR 0050 (primer 12K cap), and ADR 0051 (Tacit-Lite token rule, primer-inclusive per-task), every term in the Phase 3 gate calculation is pinned. Stage 9's measurement is mechanical.

10. **Changes to the Stage 1 surface after this freeze require a new ADR.** Spec ambiguities discovered during Stage 2+ implementation are bugs against ADRs 0047–0055 and resolved via new ADRs that supersede or amend them, not by relitigating the relevant ADR text.

## What is NOT frozen

- The stdlib expansion implementation (Stage 2 work — codegen, signatures, smoke tests for the eight new primitives per ADR 0047).
- The Phase 2 carry-over programs under `examples/phase-3/` (Stage 3 work).
- The Tacit-Lite reference solutions for the open 47 corpus tasks (Stages 4–6 work, governed by ADR 0048's idiom rules).
- The primer document at `plans/primer/tacit-lite-primer.md` (Stage 7 work, budgeted by ADR 0050).
- The `corpus-eval` harness extension (Stage 8 work, contracted by ADRs 0052 and 0055).
- The `corpus-eval --track maintenance` mode and the `corpus/maintenance/` task set (Stage 10 work, scoped by ADR 0053).
- The Stage 9 baseline run results and the Stage 10 maintenance / cross-family run results under `plans/phase-3-results/`.
- The formal `docs/phase-3-metrics.schema.json` JSON Schema file (Stage 7 deliverable per ADR 0055; it validates `corpus-eval` outputs but is not itself produced in Stage 1).
- The Phase 3 freeze ADR (Stage 11 deliverable).
- The CLAUDE.md current-phase annotation (still "Phase 3" as a whole; Stage 11 will update to "Phase 3 complete" or "Phase 3 in primer-revision cycle" per the Stage 9 outcome).

## Exit-gate evidence

Per [phase-3-plan.md § Stage 1](../plans/phase-3-plan.md):

> Exit gate: every Q-P3-N has an Accepted ADR; the canonical-format / stdlib amendments from Q-P3-1 ship with conformance test vectors landed under [`plans/test-vectors/`](test-vectors/) (or note in the ADR if no new canonical syntax is required). A **Stage 1 freeze ADR** closes the stage, mirroring [ADR 0044](../decisions/0044-p2-stage-1-frozen.md).

- Every Q-P3-N has an Accepted ADR. ✓ (See § Decision item 1.)
- Q-P3-1 (ADR 0047) introduces no new canonical node kinds and no new lexical rules; it operates inside the existing `@name` primitive surface and the existing canonical type-form surface. The "no new canonical syntax" branch of the exit gate applies, and ADR 0047 § "Conformance and tests" records this explicitly. ✓
- Stage 1 freeze ADR (this document) lands, mirroring ADR 0044 in shape. ✓

## Related decisions

- [ADR 0013](0013-canonical-text-format-frozen.md) — canonical text format; not amended by Phase 3 Stage 1.
- [ADR 0019](0019-corpus-idiom-rules.md) — Python / Rust corpus idiom rules; ADR 0048 is the Tacit-Lite analogue.
- [ADR 0020](0020-sealing-held-out-in-repo.md) — sealing mechanism; load-bearing for ADRs 0048, 0049, 0050.
- [ADR 0021](0021-corpus-stdlib-dominance-reporting.md) — three-way aggregate split; consumed by ADRs 0051 and 0055.
- [ADR 0025](0025-phase-1-libc-surface.md) — `libc-effects.toml` schema; **not** amended by ADR 0047 (the new primitives are intrinsics, not libc wrappers).
- [ADR 0028](0028-phase-1-libc-call-surface.md) — `@name` primitive surface; extended additively by ADR 0047 (PARSE, FORMAT, MEM categories).
- [ADR 0033](0033-phase-1-frozen.md) — Phase 1 baseline.
- [ADR 0035](0035-p2-effect-set-canonical.md) — four-atom effect lattice; consumed unchanged by ADR 0047 effect signatures.
- [ADR 0038](0038-p2-writable-buffer.md) — `Buf N` type; ADR 0047 admits `Buf` (dynamic) as a supertype.
- [ADR 0041](0041-p2-structured-error-format.md) — diagnostic envelope; embedded in ADR 0055's metrics schema.
- [ADR 0044](0044-p2-stage-1-frozen.md) — Phase 2 Stage 1 freeze ADR; this ADR mirrors its shape.
- [ADR 0046](0046-p2-stage-5-frozen.md) — Phase 2 frozen; the foundation Phase 3 builds on.
- [phase-3-plan.md § Stage 1](../plans/phase-3-plan.md) — closed by this ADR. Stages 2–11 are now implementation-blocked only on their own deliverables, not on spec questions.
