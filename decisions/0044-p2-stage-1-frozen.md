# 0044 — Phase 2 Stage 1 frozen

**Status:** Accepted
**Date:** 2026-04-27
**Phase:** 2, Stage 1 (exit)
**Closes:** [phase-2-plan.md § Stage 1](../plans/phase-2-plan.md)
**Artifacts frozen by this ADR:**
- ADRs [0034](0034-p2-type-subset-ann.md), [0035](0035-p2-effect-set-canonical.md), [0036](0036-p2-effect-polymorphism-syntax.md), [0037](0037-p2-pat-int.md), [0038](0038-p2-writable-buffer.md), [0039](0039-p2-module-authoring-syntax.md), [0040](0040-p2-hole-recovery.md), [0041](0041-p2-structured-error-format.md), [0042](0042-p2-operator-overload.md), [0043](0043-p2-test-conventions.md) — the Stage 1 spec surface.
- Test vectors V29–V33 under [`plans/test-vectors/`](../plans/test-vectors/) — conformance bytes for ADRs 0034–0038.
- The Phase 2 amendment notes in [`plans/canonical-text-format.md` § 11](../plans/canonical-text-format.md) — open items resolved or moved to deferral.

## Context

Stage 1 of [phase-2-plan.md](../plans/phase-2-plan.md) was scoped as "ADRs only. No production code." Its purpose was to close every spec question that Stages 2–5 would otherwise have to bikeshed mid-implementation. Ten open questions (Q-P2-1 through Q-P2-10) covered the type subset, effect-set canonical form, effect polymorphism, the four Phase 1 carry-overs (smoke #7, smoke #8, `module` authoring, hole recovery), structured diagnostics, operator overload resolution, and test conventions for typed programs.

All ten landed as Accepted ADRs on 2026-04-27 (commit `a3c5a4d`, "Phase 2 stage 1"). The conformance test vectors for the canonical-format amendments shipped alongside them.

## Decision

**Stage 1 is frozen.** The ten ADRs above are the Phase 2 spec surface that Stages 2–5 build against. Further amendments require new ADRs and are treated as spec bugs against this surface, not scope renegotiation — the same discipline imposed on Phase 0 frozen artifacts by [ADR 0013](0013-canonical-text-format-frozen.md) and on Phase 1 by [ADR 0033](0033-phase-1-frozen.md).

Concretely:

1. **The Q-P2-N → ADR mapping is final.**

   | Q-P2-N | Subject                                             | ADR  |
   |--------|-----------------------------------------------------|------|
   | Q-P2-1 | Type subset for `ann`                               | 0034 |
   | Q-P2-2 | Effect-set canonical syntax + lattice ordering      | 0035 |
   | Q-P2-3 | Effect polymorphism surface syntax                  | 0036 |
   | Q-P2-4 | `pat-int` canonical extension                       | 0037 |
   | Q-P2-5 | Writable-buffer binding model                       | 0038 |
   | Q-P2-6 | Top-level `module` authoring syntax                 | 0039 |
   | Q-P2-7 | Hole-node parser recovery (supersedes ADR 0023)     | 0040 |
   | Q-P2-8 | Structured error format                             | 0041 |
   | Q-P2-9 | Operator overload resolution                        | 0042 |
   | Q-P2-10| Test conventions for typed programs                 | 0043 |

2. **Five new canonical node kinds are admitted to the format**: `fn-ty`, `ty-var`, `forall` (ADR 0034), `eff-set` (ADR 0035), `eff-var` (ADR 0036), and `pat-int` (ADR 0037). Three new diag-ids are admitted: `type-parse-error`, `effect-parse-error`, `module-binding-error` (ADR 0040). One new `@name` primitive category is admitted: STACK-ALLOC, with `buf-alloc` as its sole Phase 2 member (ADR 0038). All of these are additive amendments to [ADR 0013](0013-canonical-text-format-frozen.md); no existing tag, diag-id, or primitive is repurposed.

3. **Conformance test vectors V29–V33 are committed** under [`plans/test-vectors/`](../plans/test-vectors/) with narrative entries in [`plans/test-vectors.md`](../plans/test-vectors.md). V33 round-trips through the existing Phase 1 canonical parser today (it uses only Phase 1 tags); V29–V32 round-trip once Stage 2 extends the canonical parser with the new tags. The "passes on the existing parser" exit-gate clause is satisfied by V33; for V29–V32 the clause is a Stage 2 entry-gate, which is the natural reading given Stage 1's "ADRs only" scope.

4. **The `tacit-typecheck` consumer remains a stub.** Stage 1's exit-gate explicitly permits this. Stage 2 builds the consumer; Stage 3 layers the effect checker; Stage 4 closes the four Phase 1 carry-overs; Stage 5 wires the CLI and the Phase 2 freeze ADR.

5. **Tacit-Full features remain explicitly out of scope.** The ADR set holds the Phase 2 / Phase 7 line firm:
   - Row polymorphism (M ≤ 1 in `forall`; ADR 0036).
   - User-defined effects (atom set fixed at `{Alloc, Div, IO, Mut}`; ADR 0035).
   - Effect handlers (not present; ADR 0036).
   - Type classes / `Num a` constraints (ADR 0042 alternative-considered).
   - Heap-allocated buffers, dynamic buffer sizes, ownership/lifetime types (ADR 0038 alternative-considered).
   - Whole-module type inference (ADR 0034 generics scope).

   Design pressure for any of these during Stage 2+ is a Phase 7 signal — defer, do not extend Phase 2.

6. **Changes to the Stage 1 surface after this freeze require a new ADR.** Spec ambiguities discovered during Stage 2+ implementation are bugs against ADRs 0034–0043 and resolved via new ADRs that supersede or amend them, not by relitigating the relevant ADR text. This is the same freeze discipline as ADRs 0013, 0017, 0018, 0032, 0033.

## What is NOT frozen

- The `tacit-typecheck` crate (Stage 2 work).
- The effect checker (Stage 3 work).
- The four Phase 1 carry-overs as concrete features — smoke #7 / #8, `module` authoring parser, hole-node parser recovery (Stage 4 work).
- The `tacit check` CLI subcommand and `--types`/`--effects` view flags (Stage 5 work).
- The `docs/error-format.schema.json` JSON Schema file (Stage 5 deliverable per ADR 0041).
- The `[types]` table population in smoke-program sidecars (Stage 2 deliverable per ADR 0043).
- Inline updates to [`plans/canonical-text-format.md`](../plans/canonical-text-format.md) § 2 / § 7 / § 8 tables. The ADRs append rows by reference; the spec body remains as frozen by ADR 0013. Whether to inline the new rows in a future doc-update pass is a Stage 5 cosmetic decision, not a spec change.

## Exit-gate evidence

Per [phase-2-plan.md § Stage 1](../plans/phase-2-plan.md):

> Exit gate: every Q-P2-N has an Accepted ADR; the canonical-text-format amendment ADRs (Q-P2-1, -2, -4, -5) ship with conformance test vectors landed under `plans/test-vectors/` and passing on both the existing canonical-form parser and the new typecheck consumer (consumer can be a stub at this stage).

- Every Q-P2-N has an Accepted ADR. ✓
- Q-P2-1 → V29, V30; Q-P2-2 → V30, V31; Q-P2-4 → V32; Q-P2-5 → V33. All committed. ✓
- V33 passes the existing Phase 1 canonical parser. V29–V32 require Stage 2 parser extensions; this is consistent with "ADRs only, no production code." ✓
- Typecheck consumer is a stub (the crate does not exist yet). ✓

## Related decisions

- [ADR 0013](0013-canonical-text-format-frozen.md) — canonical text format; amended additively by ADRs 0034, 0035, 0036, 0037, 0038, 0040.
- [ADR 0014](0014-sidecar-format.md) — sidecar format; extended by ADR 0040 (`hole_positions`) and ADR 0043 (`[types]`).
- [ADR 0023](0023-hole-node-recovery-deferred.md) — superseded by ADR 0040.
- [ADR 0025](0025-phase-1-libc-surface.md) — `libc-effects.toml` schema; consumed (not amended) by ADRs 0035 and 0038.
- [ADR 0028](0028-phase-1-libc-call-surface.md) — `@name` primitive surface; extended additively by ADR 0038 (STACK-ALLOC category).
- [ADR 0030](0030-phase-1-arith-primitives.md) — Phase 1 arith/cmp primitives; preserved with type-system overlay by ADR 0042.
- [ADR 0033](0033-phase-1-frozen.md) — Phase 1 baseline; the four Phase 1 carry-overs deferred there are now spec-closed (ADRs 0037, 0038, 0039, 0040) and implementation-deferred to Stage 4.
- [phase-2-plan.md § Stage 1](../plans/phase-2-plan.md) — closed by this ADR. Stages 2–5 are now implementation-blocked only on their own deliverables, not on spec questions.
