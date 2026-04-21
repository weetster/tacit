# Phase 0 Implementation Plan

**Status:** Draft
**Parent:** [tacit-plan.md](tacit-plan.md)

Phase 0 is spec + scaffolding — no compiler code yet. The critical-path item is the canonical text format; everything else branches from it. Phase 1 must not begin until every deliverable here is frozen.

## Deliverables (from parent plan)

- Canonical text format specification (byte-exact)
- Formal grammar for Tacit-Lite AST as a Rust enum hierarchy
- Authoring view grammar with token-efficiency analysis
- Inspection view grammar
- Content-addressing scheme specification (BLAKE3)
- Phase 3 evaluation task corpus (~50–100 tasks, held-out subset sealed)
- Project repository structure, CI, issue tracking
- Decision log document

## Sequencing

### Stage 1: Resolve blocking open questions (1–2 weeks) — **Complete (2026-04-21)**

Decisions closed as ADRs in `decisions/`:

- **Q7 Target tokenizer** — [ADR 0001](../decisions/0001-target-tokenizer.md): tiktoken primary, Claude as validation. Inverts the plan's original framing (pragmatic, given API access at the time).
- **Q6 License** — [ADR 0002](../decisions/0002-license.md): MIT OR Apache-2.0 dual-license.
- **Q1 Authoring view format** — [ADR 0003](../decisions/0003-authoring-view-bpe-compact.md): bpe-compact. Five candidates scored on two reference ASTs (21 and 100 nodes) under two tokenizers. BPE-family beat non-BPE by 40%+ at 100 nodes; within BPE, bpe-compact won 2–7% (noise-band per ADR 0001's ≥10% rule, picked on design grounds: no DeBruijn in the authoring view, no pattern-var-name stripping). Grammar doc: [authoring-bpe-compact.md](candidates/authoring-bpe-compact.md).
- **`rec` arity** (new Stage 1 question surfaced during scoring) — [ADR 0004](../decisions/0004-rec-arity.md): inner `rec` is 1+N; separate `module` kind at arity N for top-level.
- Q2 (effect polymorphism surface syntax), Q4 (testing conventions), Q5 (metadata sidecar format) remain deferred to Stage 2.
- Q3 (scope of libc wrappers) remains a Phase 1 concern.

One open item carried into Stage 4: whether the bpe-compact lead holds on non-lambda-calc-shaped programs (corpus-shaped). If the lead reverses at corpus freeze, ADR 0003 is superseded.

### Stage 2: Canonical format spec (2–3 weeks)

Must be frozen before any other Phase 0 deliverable. Two independent implementations must produce identical bytes for the same AST.

- Grammar down to exact bytes: node kinds, integer encoding (LEB128 vs decimal), field ordering in records, DeBruijn index encoding, `rec { }` grouping rule, `Hole` node structure
- BLAKE3 hashing rule over canonical text of each subtree, including the mutual-recursion grouping rule
- Round-trip test vectors: ~30 ASTs with expected canonical bytes, for cross-impl verification

### Stage 3: View grammars + AST enum (1–2 weeks, parallelizable with Stage 4)

- Rust AST enum hierarchy deriving from the canonical spec
- Authoring view grammar + bidirectional projection rules
- Inspection view grammar + projection rules (indented, type-annotated, effect-annotated)
- Display metadata (`.tacd`) sidecar format — JSON is the cheap default (resolves Q5)

### Stage 4: Evaluation corpus (2–3 weeks, parallelizable with Stage 3)

Frozen upfront so Phase 3 does not grade its own homework.

- 50–100 tasks spanning arithmetic, strings, collections, I/O, small algorithms
- Reference solutions in Python + Rust
- Executable test cases (stdin/stdout contracts)
- Seal ~20% as held-out; store hashes of held-out set in a separate repo to enforce

### Stage 5: Repo scaffolding (parallel, low-effort)

- Cargo workspace layout, CI (fmt / clippy / test), issue templates
- Decision log (`decisions/NNNN-title.md` ADR format)

## Exit criteria

1. Canonical text format is frozen and specified precisely enough that two independent implementations produce byte-identical output on all test vectors.
2. Another engineer can read the spec and build a round-tripping parser for the storage view.
3. Phase 3 task corpus is finalized and the held-out subset is sealed.

## Risks

- **Authoring-view bikeshedding** — timebox Q1 to 2 weeks; pick "good enough" over "optimal".
- **Corpus curation drift** — freeze hard at end of Stage 4; changes require a decision-log entry.
- **Spec ambiguity discovered in Phase 1** — expected; treat as a bug against Phase 0, not scope creep.
- **View system deprioritized** — both authoring and inspection grammars must land together. If inspection slips, the authoring view effectively becomes the canonical form and the view abstraction rots.
