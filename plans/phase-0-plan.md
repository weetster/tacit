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

### Stage 1: Resolve blocking open questions (1–2 weeks)

Must be decided before writing any spec.

- **Q1 Authoring view format** — the single most consequential decision. Candidates: S-exprs over integer IDs, single-glyph prefix operators, BPE-optimized encoding. Prototype 2–3 candidates on a 20-node reference AST and measure tokens under the target tokenizer.
- **Q7 Target tokenizer** — must precede Q1's measurement. Recommend Claude's tokenizer as primary, tiktoken as secondary sanity check.
- **Q6 License** — MIT/Apache-2.0 dual-license is conventional; decide early so the repo is clean.
- Q2 (effect polymorphism surface syntax), Q4 (testing conventions), Q5 (metadata sidecar format) can slip to Stage 2.
- Q3 (scope of libc wrappers) is a Phase 1 concern.

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
