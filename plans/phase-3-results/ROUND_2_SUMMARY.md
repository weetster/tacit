# Phase 3 Standard-Library Round 2 — Conclusion

**Status:** Unsuccessful per exit criteria. Byte-level surface expansion insufficient to close strings/IO token gap.

**Date:** 2026-05-06

**Canary Results (12 tasks, Sonnet 4.6 with repair-loop):**

| Metric | Gate | Result | Status |
|--------|------|--------|--------|
| Final repair-loop pass | ≥11/12 | 12/12 | ✅ PASS |
| Invalid recovery | ≥50% | 100% | ✅ PASS |
| Avg model calls | <2.0 | 1.08 | ✅ PASS |
| One-shot improvement | +2 tasks | +1 task | ❌ FAIL |
| Token reduction | ≥25% | -9.6% | ❌ FAIL |
| Strings family ratio | ≤4.0× | 6.09× | ❌ FAIL |

**Canary Run IDs:**
- Pre-correction one-shot: `019dfd48-9ea9-7158-bc09-7f39f8c2150e` (10/12)
- Post-correction repair-loop: `019dfd4d-a1e7-7197-b54b-5b3ee6d9fcfe` (12/12 final)

## Analysis

### What Round 2 Succeeded At

**Correctness.** Repair-loop discipline recovered both failures with minimal API overhead:
- One-shot: 11/12 (91.7%)
- Final: 12/12 (100%)
- Repair recovery: 100%
- Average API calls: 1.08 per task

This is the strongest correctness result across all canary runs. The 11 new primitives (Bundles E, F, G) are well-designed and well-composed by the model when given repair opportunities.

**Expert-authored token reduction.** The hand-written `reference.stdlib.tac` files show 37.3% word-count reduction on primary-target tasks vs baseline Tacit references. The primitives are genuinely useful for human programmers.

### Why Round 2 Failed the Gates

**Per-task tokens rose, not fell.** Comparing one-shot generation tokens on the heaviest strings tasks:
- `strings/020-is-anagram`: 550 → 773 tokens (+40%)
- `strings/016-longest-word`: 294 → 596 tokens (+103%)

The model is using the new primitives but composing more carefully (more correct) rather than more compactly (more dense). The expert-authored references show why:

**`strings/020-is-anagram` (`reference.stdlib.tac`):**
```tac
let counts = @i64-alloc 256 in
rec {
  init = lambda i. if @ge i 256 then 0 else ...
  fill1 = lambda off. ...
  fill2 = lambda off. ...
  cmp = lambda i. ...
} in ...
```

This still requires three separate recursive functions threading through the character counts because Tacit lacks tuples and higher-order combinators. Versus Python's `sorted(a) == sorted(b)` (62 tokens).

**`strings/016-longest-word` (`reference.stdlib.tac`):**
```tac
let scan = rec {
  scan = lambda off. lambda k. ... lambda pos/wstart/wcps/bstart/bblen/bcps ...
} in ...
```

A 6-parameter lambda function threading state manually because there is no record/tuple type to pack state. This is not a byte-level surface issue—primitives cannot substitute for language-shape features.

### Root Cause: Language-Shape, Not Surface Gaps

The plan's own ¶"Failure-source patterns" identified five patterns:

1. **One-byte-at-a-time stdin loop.** — **Solved by Bundle E** (`@stdin-slurp`)
2. **Manual UTF-8 decode.** — **Solved by Bundle G** (`@utf8-decode`, `@utf8-encode`)
3. **Manual ASCII case shift.** — **Solved by Bundle F** (`@ascii-tolower`, `@ascii-toupper`)
4. **Vowel/digit/space classification.** — **Solved by Bundle F** (`@ascii-is-*`)
5. **Packed-integer multi-return.** — **Deferred (language-shape problem)**

Patterns 1–4 were solved. But pattern 5—and the deeper issue of lambda-threaded state management—dominate the remaining token cost. The expert references prove this: even with all 11 primitives available, the bottleneck is not the byte-level surface but the absence of:

- **Tuples / records** (for multi-value returns without arithmetic packing)
- **Higher-order combinators** (`for-each`, `map`, `fold` over sequences)
- **Closures** (to capture loop state more idiomatically than lambda threading)

These are Phase 4 / language-design issues, not stdlib issues.

### Stop Rule Verdict

From [phase-3-stdlib-round-2.md § Stop rule](../phase-3-stdlib-round-2.md):

Failure condition #5: **"Dominant failures are generic Tacit composition rather than missing bundle-E/F/G operations."** ✅ Confirmed.

The failures (low one-shot, high per-task tokens) are rooted in lambda threading and state packing, not in unavailable byte-level primitives. Round 2's one allowed correction cycle did not reveal a fixable surface gap.

## Reported Finding for Phase 3 Freeze ADR

Byte-level surface expansion (Bundles E, F, G) does not, on top of Round 1, close the per-task token gap to within striking distance of Python parity for the strings/IO families under one-shot authorship. The remaining gap (6.09× on strings canary subset vs 3.5× target) is dominated by generic Tacit composition shape (manual recursion, lambda-threaded state, no tuples / no higher-order combinators), which primitives cannot substitute for.

This strengthens the **option 3** framing in the freeze ADR ("Tacit is heavier; lead positioning with structure, not density") and points subsequent work at Phase 4 language-shape changes rather than additional stdlib bundles.

### Implications for Positioning

The working hypothesis entering Phase 3 was "AI-first density"—can language features borrowed from libc and sequence utilities get the token ratio down to Python parity on a per-task basis? Round 2's concrete answer is no: the shape of the language (not the width of the stdlib) is the load-bearing constraint.

This opens a strategic pivot: instead of "can we make Tacit lean?", the better question is "what is Tacit's structural advantage?" The planned answer is content-addressed code, effect-tracked correctness, and language-shaped reasoning support—not token density. Phase 4 should invest in language-design features that make reasoning *about* correctness and program structure easier, not features that make programs shorter.

## Conclusion

Round 2 is unsuccessful per the documented exit criteria. No full open run is warranted. The canary data is sufficient to conclude that further stdlib expansion is not a productive direction for closing the identified token gap. The Phase 3 freeze ADR should record this finding and cite the option-3 recommendation for the long term.
