# Phase 3 Standard-Library Round 2 Plan

**Status:** Draft (post-round-1 design pass)
**Date:** 2026-05-04
**Predecessor:** [phase-3-stdlib-next-steps.md](phase-3-stdlib-next-steps.md) (round 1, completed 2026-05-04)

## Summary

Round 1 settled the central library-mediated authoring question for the tasks
it covered: with vector, text-indexing, ordering, and counting primitives, a
frontier model can compose stdlib operations one-shot at 68% and recover via
repair at 98% across 47 open tasks. Per-task generation tokens for stdlib-
covered tasks fell to ~3.27× Python.

Round 1 did not address the bulk of the remaining token cost. The full open
run (`019df533-fc2a-7511-ad6f-ebdc653878ae`) still shows generation tokens at
**~3.86× Python** (17,705 vs 4,584 across 47 tasks). The gap is concentrated
in tasks **without** stdlib references, especially the strings family.

Round 2 narrows the question:

> Can a small set of byte-level I/O, character-class, and codepoint
> primitives bring the strings/IO families' generation token ratio under 3.5×
> without a primer-budget regression and without recipe-shaped guidance?

This is a different hypothesis than round 1's "library-mediated authorship is
learnable." Round 1 measured one-shot composition under a stdlib surface;
round 2 measures whether one more layer of byte-level surface can close the
last big per-task token gap. The same reported-not-gating posture applies:
results inform the freeze ADR and post-Phase-3 work, not the Phase 3
go/no-go.

## Where round 1 landed

From [phase-3-stdlib-next-steps.md § Conclusion](phase-3-stdlib-next-steps.md):

- One-shot pass rate: 68% (32/47), final pass rate 97.9% with repair.
- 14 tasks have `reference.stdlib.tac` (collections, sorting algorithms,
  matrix, line-oriented IO).
- Stdlib token reduction: 51.7% on the 12-task canary subset.
- The four implemented bundles cover indexed storage (A), text indexing
  (B/B2), ordering (C), and counting/dedup (D).

What round 1 explicitly *did not* attempt:

- Byte-level stream input ergonomics. Tasks read stdin one byte at a time
  unless the model figures out to allocate a large buffer and `@read`
  the whole thing. The latter is not idiomatic in the current primer.
- ASCII character classes and case shifting.
- UTF-8 codepoint decode/encode. Multi-byte sequences are open-coded with
  manual `@lt b 128`/`@lt b 224`/`@lt b 240` branches.
- Multi-value return / state packing. Tasks needing tuple-shaped state
  pack with `* 40000 + g` arithmetic.

## Token analysis (data driving round 2)

From `019df533-fc2a-7511-ad6f-ebdc653878ae.metrics.json`, partitioning the
47 open tasks by whether they have a `reference.stdlib.tac`:

| Group                      | n  | tac tokens | py tokens | ratio |
|----------------------------|----|-----------:|----------:|------:|
| With stdlib ref            | 14 |       5,868 |     1,796 | 3.27× |
| Without stdlib ref         | 33 |      11,837 |     2,788 | 4.25× |
| ↳ no-stdlib **strings**    | 7  |       3,443 |       554 | **6.21×** |
| ↳ no-stdlib **io**         | 4  |         845 |       177 | 4.77× |
| ↳ no-stdlib collections    | 4  |       1,443 |       364 | 3.96× |
| ↳ no-stdlib algorithms     | 10 |       3,816 |     1,005 | 3.80× |
| ↳ no-stdlib arithmetic     | 8  |       2,290 |       688 | 3.33× |

Highest per-task ratios (Sonnet generation, all compile_pass=True unless
noted):

| ratio  | tac | py  | task                            |
|-------:|----:|----:|---------------------------------|
| 10.47× | 932 |  89 | strings/016-longest-word        |
|  8.06× | 524 |  65 | strings/013-is-palindrome       |
|  8.03× |1164 | 145 | arithmetic/009-divisors (1/7)   |
|  7.35× | 272 |  37 | strings/011-reverse-string      |
|  7.27× | 458 |  63 | strings/015-title-case          |
|  7.24× | 601 |  83 | algorithms/047-stable-sort-pairs|
|  6.12× | 563 |  92 | algorithms/042-valid-sudoku-row |
|  5.91× | 609 | 103 | strings/020-is-anagram          |

### Failure-source patterns (from reading the heavy references)

1. **One-byte-at-a-time stdin loop.** `let buf = @buf-alloc 1; let n = @read
   0 buf 1; if @eq n 0 ... loop` is repeated boilerplate. Adds ~25–35 tokens
   per task vs a one-shot slurp.
2. **Manual UTF-8 decode.** `is-palindrome` (lines 12–42 of its
   `reference.tac`) branches on `@lt b 128`, `@lt b 224`, `@lt b 240` and
   reconstructs codepoints by hand. Same pattern in `reverse-string`.
   ~80–150 tokens of ceremony per task that needs codepoint awareness.
3. **Manual ASCII case shift.** `title-case` and others compute
   `if @ge byte 97 then if @le byte 122 then @sub byte 32` inline.
4. **Vowel/digit/space classification by byte literal.** `count-vowels`
   spells out `@eq byte 97`, `@eq byte 101`, ... five lowercase + five
   uppercase = 10 branches. Generic across many tasks.
5. **Packed-integer multi-return.** `divisors` packs Newton-iterate state
   as `vv * 40000 + g`. Same pattern in `longest-word` (word + length
   packed), and elsewhere when a recursive helper carries >1 numeric
   accumulator without a tuple type.

Patterns 1–4 are byte-level surface gaps. Pattern 5 is a language-shape
gap — left to round 3 / Phase 4 because primitives can't cleanly substitute
for tuple/record types.

## Round 2 candidate bundles

Each bundle obeys the round-1 design rules ([phase-3-stdlib-next-steps.md §
Design Rules](phase-3-stdlib-next-steps.md)): general across ≥2 task families,
compact authoring shape, typed effect signature, codegen+typecheck tests,
shippable model-facing example, no corpus-shaped naming.

### Bundle E — Stream input and output sugar

Goal: stop encoding byte-stream loops as `let buf = @buf-alloc 1` plus
recursion. Make the slurp-then-process pattern primer-default, not a
tribal-knowledge alternative.

Candidate primitives:

- `@stdin-slurp buf cap` — read up to `cap` bytes from fd 0 until EOF;
  return total bytes read. Equivalent to a tail-rec `@read` loop, packaged.
  Effect: `IO`. Saves the 25-token boilerplate per IO/string task.
- `@write-range fd buf off len` — write `buf[off..off+len)` to `fd`. Today
  models write a slice by computing `buf+off` arithmetic or copy-then-write;
  this is a one-call form. Effect: `IO`.
- `@buf-rev buf off len` — reverse the byte range `buf[off..off+len)` in
  place. Effect: `Mut`. Direct enabler for `reverse-string` and reverse
  variants.

Expected impact: per-task savings of 20–60 tokens on `reverse-string`,
`title-case`, `count-vowels`, `is-palindrome` (the slurp portion only),
`is-anagram`, `longest-line`, `line-count`, `word-count`, `echo-reverse`.

### Bundle F — ASCII character class and case

Goal: collapse the `@eq byte 97 ... @eq byte 117` and
`@ge byte 97 if @le byte 122` patterns to one call.

Candidate primitives:

- `@ascii-tolower b` / `@ascii-toupper b` — return the case-shifted byte
  for ASCII letters, identity otherwise. Pure (no effect).
- `@ascii-is-alpha b` / `@ascii-is-digit b` / `@ascii-is-space b` — return
  0/1. Pure.

`@ascii-is-vowel` was considered and rejected as too task-specific: the
operation does not appear in mainstream language standard libraries and
shows up in essentially one corpus task (`count-vowels`). That task uses
`@ascii-tolower` + a 5-branch equality check instead.

Expected impact: 5–25 tokens per affected task. Affected tasks:
`title-case`, `count-vowels`, `is-anagram`, `valid-sudoku-row` (digit
checks), `parse-i64` callers (already covered), `word-count` token
discrimination.

### Bundle G — UTF-8 codepoint decode/encode

Goal: collapse the manual 1/2/3/4-byte UTF-8 ceremony to one call each way.

Candidate primitives:

- `@utf8-decode buf off` — read one codepoint starting at `buf[off]`;
  return packed `(cp, byte_len)` where `byte_len` is 1–4 (or 0 on
  malformed). Packing convention: `cp * 8 + byte_len`. Pure read; effect
  `{}`.
- `@utf8-encode buf off cp` — write codepoint `cp` as 1–4 UTF-8 bytes
  starting at `buf[off]`; return number of bytes written. Effect: `Mut`.
- `@utf8-len cp` — return how many UTF-8 bytes a codepoint encodes to
  (1–4). Pure. Lets a model pre-size output buffers without writing.

Expected impact: 50–120 tokens per UTF-8-aware task. Affected tasks:
`reverse-string` (write codepoints back in reverse order), `is-palindrome`
(decode forward and reverse; cleaner equality), `longest-word` (encode
captured codepoints to output instead of base-27 packing). Indirect: any
future text task that needs to copy a slice as codepoints.

The packed return convention follows the existing range-table style
(absolute offset + length packed via i64 arithmetic). The threshold
between "primitive returns a pair via packing" and "primitive writes to
an out-buffer" stays consistent with round 1: small packed pair → packed
i64; >2 outputs → write to caller-supplied buffer.

### Bundles deferred from round 2

- **Tuple/record support.** Multi-value packing (pattern 5) is a
  language-shape problem, not a primitive gap. Adding `@pair-fst`/
  `@pair-snd` style helpers without a real product type would just rename
  the existing arithmetic packing. Defer to Phase 4 / language-surface
  redesign.
- **Closures or higher-order combinators** (`for-each`, `map`, `fold`).
  Round 1's conclusion already noted these as Phase 4+ for the >80%
  one-shot target.
- **Module/import system.** Out of scope per parent plan.

## Design rules

Inherit from [phase-3-stdlib-next-steps.md § Design Rules](phase-3-stdlib-next-steps.md).
Two round-2-specific tightenings:

- **Primer-budget cap.** The current stdlib appendix is roughly under the
  1,500-token round-1 budget. Round 2 must not push the appendix past
  **2,000 tokens total**. Each new primitive earns its tokens or it
  doesn't ship.
- **Net-token rule.** A bundle ships only if its expected per-task savings
  on the canary, multiplied by the count of canary-affected tasks,
  exceeds the primer-token cost of its appendix entry. This is the
  round-1 implicit rule made explicit; round 2 has more bundles
  competing for less primer headroom.

## Canary

Twelve tasks. String/IO heavy, with regression coverage for round-1 bundles
to catch any cross-bundle regressions.

Primary target tasks (round-2 surface):

- `strings/011-reverse-string` (Bundle E + G)
- `strings/012-count-vowels` (Bundle F)
- `strings/013-is-palindrome` (Bundle E + G)
- `strings/015-title-case` (Bundle E + F)
- `strings/016-longest-word` (Bundle E + G)
- `strings/020-is-anagram` (Bundle F)
- `io/051-line-count` (Bundle E)
- `io/057-word-count` (Bundle E + F)
- `io/059-echo-reverse` (Bundle E)
- `io/060-longest-line` (Bundle E; existing stdlib ref serves as regression)

Regression / coverage tasks (round-1 surface unchanged):

- `collections/021-unique-in-order` (round-1 stdlib ref)
- `algorithms/036-quicksort` (round-1 stdlib ref)

This keeps the canary at 12 tasks. Round-1 references must continue to
pass tests with the round-2 stdlib loaded.

## Metrics

Same shape as round 1 ([phase-3-stdlib-next-steps.md § Metrics](phase-3-stdlib-next-steps.md)),
plus two round-2-specific reads:

- **Per-family ratio.** Strings family token ratio before vs after.
  Target: ≤ 3.5×.
- **Primer headroom.** Stdlib appendix token count before vs after.
  Target: ≤ 2,000 tokens after round 2.

Carry forward the result label: round-2 paid runs use
`--result-label library-mediated` (round-1 convention; gates remain
reporting-only).

## Exit criteria

Mirrors round 1's discipline.

Proceed from design to implementation only if a narrow ADR pins each bundle's
exact signatures and effect rows.

Proceed from implementation to paid canary only if:

- new primitive tests pass (codegen + typecheck);
- existing 47 open references still pass under the round-2 build (no
  semantic regression);
- round-2 canary `reference.stdlib.tac` files pass all tests;
- round-2 canary reference token count drops by **at least 30%** against
  the current Tacit references on the same 10 primary-target tasks
  (the two regression tasks already have round-1 stdlib refs and are not
  rewritten);
- the stdlib primer appendix is **under 2,000 tokens** after the round-2
  additions.

Proceed from paid canary to full open round-2 run only if:

- one-shot canary pass count improves by at least **+2 tasks** over the
  Sonnet `019df533` one-shot result on the same 12-task subset;
- final repair-loop canary pass count is at least **11/12**;
- generated tokens fall by at least **25%** on the canary;
- invalid recovery is at least 50%;
- average model calls stay below 2.0 per task;
- strings family token ratio on the canary subset comes in at ≤ 4.0×
  (full-corpus 3.5× target is for the post-canary full open run).

### Stop rule

Inherit round 1's stop rule verbatim: at most one non-task-specific
correction cycle between the canary and the decision. Failure conditions
that should declare round 2 unsuccessful:

- Expert-authored round-2 references aren't ≥ 30% shorter than current
  Tacit references on the canary primary-target tasks.
- One-shot canary pass count remains below the round-1 baseline (regression).
- Final repair-loop pass count drops below 11/12.
- Generated token reduction does not reach 25% on the canary.
- Dominant failures are generic Tacit composition rather than missing
  bundle-E/F/G operations.
- Acceptable canary results require task-shaped primer recipes.

If round 2 fails, the documented conclusion is narrow: byte-level surface
expansion does not, on top of round 1, close the per-task token gap to
within striking distance of Python parity for the strings/IO families
under one-shot authorship. That outcome strengthens the Phase-3 freeze ADR
case for option 3 ("Tacit is heavier; lead positioning with structure, not
density") rather than option 1 ("more stdlib") for the long term.

## Resolved design questions

The four design questions raised during round-2 plan drafting are resolved
in this section. The ADR pinning bundle signatures restates these as
decisions; the rationale lives here.

- **`@stdin-slurp` cap is caller-passed.** Signature is
  `@stdin-slurp buf cap` (returns bytes-read). Keeps the model honest
  about bounds, matches the existing `@read` shape, and avoids baking a
  magic byte budget into the language.
- **`@ascii-is-vowel` is dropped.** The operation isn't part of any
  mainstream language standard library and shows up in one corpus task.
  Bundle F is `@ascii-tolower`, `@ascii-toupper`, `@ascii-is-alpha`,
  `@ascii-is-digit`, `@ascii-is-space`. `count-vowels` uses
  `@ascii-tolower` + a 5-branch equality check.
- **`@utf8-decode` packs as `cp * 8 + byte_len`.** Tightest meaningful
  packing (3-bit length field, values 0–4). The wider round-1 packing
  factors (`* 40000`, `* 80000`) exist where the secondary value can
  exceed three bits; UTF-8 byte length cannot.
- **E, F, and G ship together.** Round 1's precedent (four bundles, one
  canary) holds; the round-2 canary cost is tractable, and per-bundle
  keep/drop decisions are deferred to the single allowed correction
  cycle if the first canary misses gates.

## Work plan

1. Write the ADR pinning Bundle E / F / G signatures and effect rows.
   ([decisions/0067-…] not yet allocated; next free number depends on
   merge order.)
2. Implement primitives in the codegen layer with codegen+typecheck
   tests. No model-facing primer changes yet.
3. Author 10 round-2 `reference.stdlib.tac` files for the primary-target
   canary tasks. Two regression tasks reuse their round-1 references.
4. Run the local preflight from [phase-3-stdlib-next-steps.md § Local
   Preflight](phase-3-stdlib-next-steps.md): build CLI with LLVM, check
   each new primitive on a tiny program, run the 12 canary references
   against their tests, report token totals.
5. Update the primer stdlib appendix with bundle-E/F/G semantics and one
   tiny generic example each. Stay under the 2,000-token cap. No
   task-shaped recipes.
6. Run the open-only canary one-shot and repair-loop modes.
7. Apply at most one non-task-specific correction cycle if needed.
8. If the proceed gates clear, run the full open library-mediated round-2
   evaluation. Report under `plans/phase-3-results/` with the
   `library-mediated` label and a `round-2` annotation in the run notes.

## Reporting

Add a follow-up section to [phase-3-stdlib-next-steps.md](phase-3-stdlib-next-steps.md)
or a new `phase-3-results/ROUND_2_SUMMARY.md` once the full run lands.
The Stage 11 freeze ADR cites round-2 numbers alongside round-1 in the
"reported, not gating" section.
