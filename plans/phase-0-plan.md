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

### Stage 2: Canonical format spec (2–3 weeks) — **Frozen 2026-04-22** ([ADR 0013](../decisions/0013-canonical-text-format-frozen.md))

Started 2026-04-21; frozen 2026-04-22 after two independent canonicalizers (`impls/py-canonicalizer/`, `impls/rs-canonicalizer/`) produced byte-identical BLAKE3 hashes on all 38 `*.canonical` fixtures and agreed on every `*.forbidden` / `*.reject` rejection. Further changes to the canonical text format now require a new ADR and are treated as spec bugs per [CLAUDE.md](../CLAUDE.md) ground rules.

Spec: [canonical-text-format.md](canonical-text-format.md). Backing ADRs:

- **[ADR 0005](../decisions/0005-canonical-surface-form.md)** — surface form is s-expressions with short ASCII keyword tags (`(lam (var 0))`), tag set frozen at Stage 2 exit, additive evolution only.
- **[ADR 0006](../decisions/0006-canonical-lexical-rules.md)** — integers in decimal ASCII, single-space token separation, no whitespace inside parens, no comments, JSON-style string escapes, no Unicode normalization.
- **[ADR 0007](../decisions/0007-debruijn-rec-indexing.md)** — for `rec` and `module`, position K in the binding list = DeBruijn index K (first binding is `(var 0)`); does not match the let-cascade analogy, chosen for simplicity.
- **[ADR 0008](../decisions/0008-record-field-ordering.md)** — record fields sorted ascending by field-symbol bytes; the only canonical-form override of user-supplied order, required for hash-equality of semantic-equality.
- **[ADR 0009](../decisions/0009-hashing-rule.md)** — `hash(node) = BLAKE3(canonical_text(node))` with children fully inlined; no hash-reference syntax inside canonical text. `rec`/`module` "hash as single atom" commitment satisfied trivially.
- **[ADR 0012](../decisions/0012-unicode-scalar-value-restriction.md)** — `\u{HEX}` escapes must denote Unicode scalar values (U+0000–U+D7FF or U+E000–U+10FFFF); surrogates and out-of-range values are hard parse errors. Tightens ADR 0006's string-escape clause; surfaced by Vector 24 during second-round drafting.

Stage 2 exit criteria (all met):

- ~~~30 round-trip test vectors with expected canonical bytes.~~ 45 files across 28 vectors under [test-vectors/](test-vectors/); narrative in [test-vectors.md](test-vectors.md). V29 remains blocked on type-subset ADR — out of scope for Stage 2 freeze.
- ~~Verification that two independent canonicalizer implementations produce byte-identical output on those vectors.~~ Python ([`impls/py-canonicalizer/`](../impls/py-canonicalizer/)) and Rust ([`impls/rs-canonicalizer/`](../impls/rs-canonicalizer/)) canonicalizers agreed on all 38 `*.canonical` fixture hashes on 2026-04-22. Both also agree on every `*.forbidden` and `*.reject` rejection.
- Open items in [canonical-text-format.md § 11](canonical-text-format.md#11-open-items) (hole diag-id set, `ann` type subset, bpe-compact corpus-shape recheck) are non-blocking and carried forward to later stages per ADR 0013.

### Stage 3: View grammars + AST enum (1–2 weeks, parallelizable with Stage 4) — **Frozen 2026-04-22** ([ADR 0017](../decisions/0017-stage-3-frozen.md))

Drafted and frozen on 2026-04-22 directly after Stage 2 freeze. The four deliverables landed as spec artifacts; a skeptical review surfaced and fixed four blocking inconsistencies before the freeze (record-sym notation in sidecar § 8, `rec` stack-discipline wording in the authoring projection, sidecar `children`-length rules across § 3.4 and § 4, and a structural rewrite of inspection-view § 3 so the § 6 fixtures reproduce deterministically). See [ADR 0017 § Review findings](../decisions/0017-stage-3-frozen.md) for the fix trail.

- **Sidecar format (`.tacd`)** — resolves Q5. Decided in [ADR 0014](../decisions/0014-sidecar-format.md): JSON parallel tree, `.tacd` extension, stale-tolerant via `targets_hash_blake3`, synthetic-name fallback when missing. Full schema + worked examples: [sidecar-format.md](sidecar-format.md).
- **Inspection view scope** — decided in [ADR 0015](../decisions/0015-inspection-view-scope.md): display-only pseudo-code (explicitly *not* round-trippable to canonical bytes), progressive annotation layers (L0 default, L1 `--debruijn`, L2 `--hashes`), Phase 1+ flags reserved for `--types` / `--effects` / `--tree` / `--table`. Full grammar with per-kind rules and L0/L1/L2 worked examples: [inspection-view.md](inspection-view.md). The structural break rules (always-break vs. "inline iff children inline") are the Stage 3 contract; § 6 renderings are the regression fixtures.
- **Authoring view projection rules** — appended to the existing grammar doc at [candidates/authoring-bpe-compact.md § Projection rules](candidates/authoring-bpe-compact.md), now grounded in ADR 0014's sidecar. Specifies both directions (authoring → canonical + sidecar; canonical + sidecar → authoring), round-trip guarantees, and missing/stale-sidecar behavior.
- **Rust AST enum** — decided in [ADR 0016](../decisions/0016-rust-ast-enum-location.md): the existing enum at [impls/rs-canonicalizer/src/ast.rs](../impls/rs-canonicalizer/src/ast.rs) is the Stage 3 conforming transcription. No Cargo workspace or shared crate in Phase 0; promotion deferred to Phase 1 with the rest of the compiler scaffolding.

Stage 3 exit criterion (per [ADR 0015](../decisions/0015-inspection-view-scope.md)): spec docs are internally consistent and an independent implementer could reproduce the worked examples byte-for-byte. Met on 2026-04-22. Unlike Stage 2, Stage 3 has no byte-equivalence gate against a built artifact — the inspection-view renderer is Phase 1+ work. Further changes to Stage 3 artifacts require a new ADR per [CLAUDE.md § Ground rules](../CLAUDE.md), matching the Stage 2 freeze discipline.

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
