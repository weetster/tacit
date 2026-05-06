# Excerpt for Phase 3 Freeze ADR — Stdlib Expansion Analysis

**To be incorporated into the freeze ADR decision document.** This section documents the empirical findings from stdlib expansion (Rounds 1–2) and their implications for the post-Phase-3 strategy choice.

## Background

Phase 3 planned two rounds of stdlib expansion to address the identified token-density gap between Tacit and Python baselines on the corpus:

- **Round 1** (completed 2026-05-04): Text indexing, vector operations, sorting, and counting primitives (Bundles A–D). Result: **68% one-shot pass rate**, **97.9% repair pass rate** on 47 open tasks; **3.27× token ratio** on stdlib-covered tasks (down from 4.25× baseline).

- **Round 2** (completed 2026-05-06): Stream I/O, ASCII classification, and UTF-8 codepoint primitives (Bundles E–G). Result: **11/12 one-shot pass rate**, **100% repair pass rate** on 12-task canary; **6.09× token ratio** on strings subset—above the 3.5× / 4.0× target.

## Empirical Findings

### Round 1: Primitives Enable Compositional Correctness

Verdict: **Successful within scope.** Indexed storage, text-indexing, ordering, and deduplication primitives are well-designed and well-composed by the model. Repair-loop discipline recovered failures at minimal cost (2.35 calls per task on 47-task full open run). The per-task token ratio on stdlib-covered tasks (3.27×) demonstrates that primitives reduce token cost for tasks that fit the covered patterns.

### Round 2: Surface Expansion Hits a Ceiling

Verdict: **Unsuccessful.** Byte-level I/O, ASCII classification, and UTF-8 primitives (Bundles E–G) improved **correctness** (11/12 one-shot, 100% final repair) but not **density**. Per-task token counts **rose** on the heaviest strings tasks despite the primitives being available:

- `strings/020-is-anagram`: 550 → 773 tokens (+40%)
- `strings/016-longest-word`: 294 → 596 tokens (+103%)

**Root cause:** The remaining token cost is not driven by byte-level surface gaps but by language-shape constraints. The expert-authored `reference.stdlib.tac` files reveal why:

```tac
{* is-anagram: still requires three separate rec-lambda functions
  to build and compare character frequency tables because Tacit
  lacks tuple types to pack the state. *}

let counts = @i64-alloc 256 in
rec {
  init = lambda i. if @ge i 256 then 0 else (let _ = @i64-set counts i 0 in init (@add i 1))
  fill1 = lambda off. if @ge off len1 then 0 else
    let byte = @buf-get input1 off in
    let prev = @i64-get counts byte in
    let _ = @i64-set counts byte (@add prev 1) in
    fill1 (@add off 1)
  fill2 = lambda off. if @ge off len2 then 0 else
    let byte = @buf-get input2 off in
    let prev = @i64-get counts byte in
    let _ = @i64-set counts byte (@sub prev 1) in
    fill2 (@add off 1)
  cmp = lambda i. if @ge i 256 then 1 else
    if @ne (@i64-get counts i) 0 then 0 else cmp (@add i 1)
} in cmp 0

{* Compare to Python: *}
sorted(a) == sorted(b)  {* 62 tokens *}
```

Similarly, `strings/016-longest-word` requires a 6-parameter lambda threading `(pos, wstart, wcps, bstart, bblen, bcps)` because record/tuple types are unavailable to pack multi-value returns.

### The Token-Cost Breakdown

Of the plan's identified five failure-source patterns (§"Failure-source patterns"), Round 2 solved patterns 1–4:

| Pattern | Issue | Primitive Solution | Status |
|---------|-------|-------------------|--------|
| 1. One-byte-at-a-time stdin loop | `let buf = @buf-alloc 1; ... @read 0 buf 1 ...` (25–35 tokens) | `@stdin-slurp` | ✅ Solved |
| 2. Manual UTF-8 decode | Branch on 128/224/240 (80–150 tokens) | `@utf8-decode` / `@utf8-encode` | ✅ Solved |
| 3. Manual ASCII case-shift | `if @ge byte 97 then if @le byte 122 then @sub byte 32` | `@ascii-tolower` / `@ascii-toupper` | ✅ Solved |
| 4. Character classification | `@eq byte 97 ... @eq byte 122` (10 branches) | `@ascii-is-alpha` / `@ascii-is-digit` / etc. | ✅ Solved |
| 5. Packed-integer multi-return | `vv * 40000 + g` arithmetic (lambda threading) | *None* (language-shape problem) | ❌ Unsolved |

**Pattern 5 dominates.** The token cost of manual lambda threading and arithmetic state-packing far exceeds the savings from byte-level primitives. Solving it requires language-design work (tuples, closures, higher-order combinators), not stdlib expansion.

## Strategic Implication: Abandon Density Parity, Embrace Structural Advantage

The empirical evidence from Rounds 1–2 refutes the hypothesis that Tacit can approach Python token parity on per-task coding problems through stdlib expansion. The remaining gap is structural, not surface.

**This outcome supports option 3 from the strategic choice:**

> **Option 3: Acknowledge Tacit is heavier. Lead positioning with structure, not density.** Tacit's advantage is not token count per task, but reasoning support: content-addressed code, effect-tracked correctness, and language-shaped program structure. Invest Phase 4 in features that make program reasoning *easier* (tuples, effect signatures, closures), not in features that make programs *shorter* (more stdlib). Market Tacit as a research and exploration tool where correctness and auditability matter more than density.

### Evidence Supporting Option 3

1. **Repair-loop discipline works.** Round 1 and Round 2 both achieve 97%+ final pass rates with <2.5 API calls per task. The model can reason about Tacit's explicit structure (effects, types, content-addressable semantics) effectively enough to self-correct when given feedback.

2. **Expert-authored code confirms the ceiling.** The `reference.stdlib.tac` files show that even humans writing with full knowledge of the primitives still pay the lambda-threading and state-packing tax. It is not a model limitation but a language-shape limitation.

3. **Primer investment hits diminishing returns.** The stdlib appendix grew from 1,699 tokens (round 1) to 2,405 tokens (round 2) for an 11-primitive addition. Further expansions will hit the 3,000–5,000 token budget ceiling of typical coding prompts. The stdlib is not the bottleneck.

4. **Correctness > density for the corpus tasks.** The corpus is designed to test compositional reasoning, not terseness. Tacit's explicit effect system and type annotations serve that goal. The token count on a per-task basis is secondary to whether the model can *reason correctly* about what it is building.

## Recommendation for Freeze ADR

**Cite Rounds 1–2 as empirical validation that stdlib expansion alone cannot close the token-density gap.** Document the specific failure modes (lambda threading, state packing) and note that they require Phase 4 language-design work. Use this finding to justify option 3 as the strategic direction: invest in language features that amplify Tacit's structural advantages (effect clarity, content-addressability, auditability) rather than features that chase Python's token parity.

**Do not plan Round 3 stdlib expansion.** The corpus analysis and Round 2 stop-rule verdict make clear that further primitives will not yield proportional token savings. Phase 4 roadmap should focus on language-design features.
