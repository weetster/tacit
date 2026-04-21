# 0003 — Authoring view format: BPE-compact

**Status:** Accepted
**Date:** 2026-04-21
**Phase:** 0, Stage 1
**Closes:** Q1 (authoring view format) from [tacit-plan.md](../plans/tacit-plan.md) § Open Questions.

## Context

Q1 asks which surface syntax the authoring view should use. Five candidates were scored against two reference ASTs (21 nodes; 100 nodes with mutual recursion) under two tokenizers (tiktoken `cl100k_base` and Claude `claude-opus-4-7`). Full methodology and data in [plans/candidates/reference-ast.md](../plans/candidates/reference-ast.md).

At 100 nodes the standings are:

| Candidate        | tiktoken | Claude  |
|------------------|---------:|--------:|
| sexpr-int-ids    |    1.78× |   1.62× |
| glyph-prefix     |    1.40× |   1.42× |
| bpe-optimized    |    1.05× |   1.07× |
| **bpe-compact**  | **1.00×**| **1.00×** |
| bpe-hybrid       |    1.02× |   1.03× |

Two findings drive the decision:

1. **BPE-family vs. non-BPE is decisive.** The three BPE variants cluster within 1.00–1.07× on both tokenizers; sexpr-int-ids and glyph-prefix land at 1.40–1.78×. The gap widened from 21 to 100 nodes (keyword amortization hypothesis validated).
2. **Within BPE, the margin is small.** bpe-compact's 2–7% lead over bpe-hybrid and bpe-optimized is below the ≥10% "comfortable margin" threshold set in [ADR 0001](0001-target-tokenizer.md). Choosing among them is closer to a design judgment than a measured win.

## Decision

**The authoring view uses the bpe-compact grammar**: BPE-friendly keywords (`let`, `lambda`, `if`, `then`, `else`, `match`, `with`, `rec`, `in`), display names at binders and var refs, no spaces around `.` and `:`, tight braces for records and rec groups. The canonical string form is documented in [reference-ast.md § Sample 2 — bpe-compact](../plans/candidates/reference-ast.md#bpe-compact-no-spaces-around---tight-braces).

The authoring view is a lossless projection of the canonical AST. Display names in the authoring view map to DeBruijn indices in canonical form via the display-name sidecar; the mapping is the canonicalizer's responsibility.

## Alternatives considered

- **bpe-hybrid (keyword skeleton + DeBruijn integer leaves).** Within 1–3% of bpe-compact on both tokenizers. Rejected because (a) the win is within noise under ADR 0001's decision rule, (b) stripping pattern-var names (as Sample 2's hybrid rendering did) requires ctor arity to be reader-visible, adding a sidecar dependency the other variants don't have, and (c) it surfaces DeBruijn ints in the *authoring* view, partially collapsing the two-views distinction.
- **bpe-optimized (display names, spaces around `.` and `:`).** 5–7% worse than bpe-compact. Rejected — same grammar family, strictly worse density on both tokenizers, no compensating readability win large enough to matter.
- **glyph-prefix.** Won Sample 1 on tiktoken (1.00×) but collapsed at 100 nodes to 1.40–1.42× on both tokenizers. Variadic constructs (`rec`, `match`, `ctor`) inflate its paren-delimiter cost exactly where BPE amortizes keywords. Rejected: the small-program win does not generalize.
- **sexpr-int-ids.** Worst on every sample and every tokenizer; gap widens with size. The "bare ` <digit>` fuses to 1 token" assumption in the candidate doc was empirically wrong in the positions this encoding actually uses. Rejected.

## Consequences

- **Authoring grammar is now frozen for Phase 0.** The bpe-compact grammar in [authoring-bpe-compact.md](../plans/candidates/authoring-bpe-compact.md) is the source of truth; Sample 2's `rec { ... } in body` and `let name : Type = value in body` extensions have been folded in, and the doc renamed from `authoring-bpe-optimized.md` to match the chosen variant.
- **Display names become load-bearing for the authoring view.** The canonicalizer must maintain a bidirectional mapping between authoring-view identifiers and canonical DeBruijn indices. This pushes complexity into the sidecar (Q5) and the canonicalizer, which was flagged as a tradeoff in the bpe-optimized candidate doc.
- **Hard coupling to BPE-family tokenizers.** A future Anthropic tokenizer that diverges materially from cl100k could swing token counts. This is the cost of the ~40% win over structural encodings; re-measurement is cheap (one script re-run) but re-design would be expensive. Acceptable because (a) the plan targets Claude-class models specifically and (b) all three BPE variants would degrade together, so the choice among BPE variants is likely stable across BPE families even if absolute counts shift.
- **Operator precedence and left-associativity are now real grammar concerns** — juxtaposition means app, and `f x y` must parse as `(app (app f x) y)`. The canonicalizer needs precedence/associativity rules; these land with the Stage 2 canonical format spec.
- **The decision is robust across the two tokenizers measured**, but both are BPE. A third data point on a non-BPE tokenizer (e.g. a character-level or SentencePiece model) is not planned — Tacit targets BPE-tokenized LLMs per Q7.
- **One open item deferred to Stage 4:** whether the bpe-compact lead holds on a differently-shaped program (long linear `let` chains, heavy string-literal content). The Phase 3 corpus freeze is the natural place to re-check; if the lead reverses on corpus-shaped programs, this ADR is superseded.

## Related decisions

- [ADR 0001](0001-target-tokenizer.md) — target tokenizer (tiktoken primary, Claude as validation).
- [ADR 0002](0002-license.md) — license.
- Next ADR will address Q2/Q4/Q5 or the `rec` arity question surfaced by Sample 2 (Stage 2 prerequisite).
