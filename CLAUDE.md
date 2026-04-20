# Tacit — Development Guide

Tacit is an AI-first programming language. See [plans/tacit-plan.md](plans/tacit-plan.md) for the full vision and [plans/phase-0-plan.md](plans/phase-0-plan.md) for current work.

**Current phase: Phase 0 — specification and scaffolding.** No compiler code yet. Work is spec writing, decision resolution, and corpus curation.

## What Phase 0 produces

- A byte-exact **canonical text format** spec for the Tacit-Lite AST (the blocking prerequisite — everything downstream depends on it)
- **Authoring** and **inspection** view grammars, both lossless projections of the canonical form
- Content-addressing scheme (BLAKE3 over canonical text)
- A frozen **Phase 3 evaluation corpus** (~50–100 tasks, with a sealed held-out subset)
- Repo scaffolding, CI, decision log

Exit is gated on: two independent canonicalizers producing identical bytes on shared test vectors, and the corpus being frozen with the held-out subset sealed.

## Ground rules for this phase

- **The canonical text format is frozen once Stage 2 ends.** Changes after that require a decision-log entry and are treated as spec bugs, not scope negotiation.
- **No Phase 1 work.** Don't write a parser, AST walker, or LLVM emitter until Phase 0's exit criteria are met. Rust AST enum definitions that derive from the spec are in scope; anything that consumes or produces them is not.
- **Two views from day one.** Authoring and inspection grammars land together. If only one ships, the view abstraction rots into a single canonical form.
- **Decision log is load-bearing.** Every non-trivial design choice gets an ADR-style entry in `decisions/NNNN-title.md`. This is how the spec stays coherent across sessions.

## Key design commitments (from the parent plan — do not relitigate in Phase 0)

- Variable references use **DeBruijn indices** in canonical text; no variable IDs. Names are display metadata only.
- Mutual recursion uses explicit `rec { ... }` groupings that hash as a single atom.
- Parser errors produce **typed `Hole` nodes** with structured diagnostics, not failed parses.
- **BLAKE3** is the hash (unless Phase 0 explicitly substitutes it).
- Display names, comments, and file layout are all sidecar / advisory. The AST is the source of truth.
- Tacit-Lite is the default focus. Tacit-Full features (refinement types, capabilities, handlers) are out of scope here and in Phase 1–6.

## Repository layout

```
plans/        — phase plans, starting with tacit-plan.md and phase-0-plan.md
docs/         — design docs (e.g. effect-system.md)
decisions/    — ADR-style decision log (to be created in Stage 5)
```

Cargo workspace, evaluation corpus directory, and CI are Stage 5 deliverables — not yet present.

## Open questions still to resolve in Phase 0

Tracked in `plans/tacit-plan.md § Open Questions`. Stage 1 resolves Q1 (authoring view format), Q7 (target tokenizer), Q6 (license). Q2/Q4/Q5 resolve in Stage 2–3. Q3 is deferred to Phase 1.

## Working style

- Prefer editing existing plan/spec files over creating new ones.
- When a design choice is made, write the ADR before writing the spec text that depends on it.
- Don't add compiler scaffolding "to save time later" — Phase 1 will do it with the benefit of a frozen spec.
