# Canonical Format — Stage 2 Pressure-Test Vectors

**Status:** Narrative authoritative; bytes split into [test-vectors/](test-vectors/) (2026-04-22). Phase 2 Stage 1 added V29–V33 (2026-04-27, ADRs 0034–0038).
**Parent:** [canonical-text-format.md](canonical-text-format.md)
**Purpose:** 33 vectors chosen to pressure-test the spec. V1–V28 backed Stage 2 exit (Phase 0); V29–V33 back Phase 2 Stage 1 amendments. Each targets a specific rule, an anti-intuitive convention, or a suspected ambiguity. Several vectors surfaced spec gaps; those are flagged in their Notes and summarized in § 30. Remaining open items are listed in § 31.

The machine-consumable bytes live in [test-vectors/](test-vectors/), one file per vector (or sub-vector), keyed by `NN-slug.{canonical,forbidden,reject}`. That directory's [README](test-vectors/README.md) documents the file-naming convention and the minimum tests an implementation must run. This doc remains the narrative reference for pressure-test descriptions, DeBruijn traces, and ADR cross-references.

---

## Vector 1 — Identity lambda (baseline)

**Pressure-tests:** `lam` arity 1, `(var 0)` baseline.

**Authoring intent:** `lambda x. x`

**Canonical:**
```
(lam (var 0))
```

**Notes:** The simplest non-atomic AST. If two canonicalizers disagree here, everything else is noise.

---

## Vector 2 — Nested `let` cascade

**Pressure-tests:** `let` body sees `(var 0)` for the bound name; outer bindings shift up by 1 under each inner binder.

**Authoring intent:** `let x = 1 in let y = 2 in x`

**Canonical:**
```
(let (int 1) (let (int 2) (var 1)))
```

**Notes:** In the inner let's body, `y` is `(var 0)`, `x` is `(var 1)`. Contrasts with Vector 4, where `rec`'s convention deliberately breaks this analogy.

---

## Vector 3 — `rec` with no intervening binders

**Pressure-tests:** [ADR 0007](../decisions/0007-debruijn-rec-indexing.md) position-K = index K, *without* a `lam` to mask the convention.

**Authoring intent:** `rec { a = b; b = a } in a` (a degenerate cycle; semantically ill-typed but syntactically well-formed)

**Canonical:**
```
(rec (var 1) (var 0) (var 0))
```

**Notes:** This is the purest ADR 0007 test. Inside binding 0's RHS, `a` is `(var 0)` and `b` is `(var 1)`; `a` is assigned `b`, so the RHS is `(var 1)`. Inside binding 1's RHS, `b` is `(var 1)` and it's assigned `a` = `(var 0)`. Body references `a` = `(var 0)`. The spec's § 10 worked example wraps each RHS in a `lam`, which obscures whether the rule applies directly in RHS position or only after a shift. This vector pins it.

---

## Vector 4 — Mutual recursion under `lam`

**Pressure-tests:** ADR 0007 combined with shift-by-1 under the `lam` introduced by each binding's RHS.

**Authoring intent:** `rec { even = \n. odd n; odd = \n. even n } in even 10` (arithmetic elided — this exercises the DeBruijn convention, not the numerics).

**Canonical:**
```
(rec (lam (app (var 2) (var 0))) (lam (app (var 1) (var 0))) (app (var 0) (int 10)))
```

**Notes:** Inside `even`'s `lam`, `n` = `(var 0)`, and the rec bindings shift up by 1: `even` = `(var 1)`, `odd` = `(var 2)`. The body `odd n` is `(app (var 2) (var 0))`. Symmetric for `odd`. Body `even 10` has no intervening binder so `even` = `(var 0)`. This is the § 10 example, stripped of the `if`/`ctor` filler, so it is purely a DeBruijn trace. Minor variant of the worked example — promoting from "illustration" to "test vector" guarantees byte-equivalence is checked on it.

---

## Vector 5 — Record field sorting: uppercase vs lowercase, length-mixed

**Pressure-tests:** [ADR 0008](../decisions/0008-record-field-ordering.md) byte-wise UTF-8 lexicographic ordering, specifically the non-obvious case where ASCII uppercase (`0x41–0x5A`) sorts *before* lowercase (`0x61–0x7A`).

**Authoring intent:** `{b: 1, aa: 2, ab: 3, A: 4}` (any order the author writes)

**Canonical:**
```
(record A (int 4) aa (int 2) ab (int 3) b (int 1))
```

**Notes:** Byte order is `"A"` (0x41) < `"aa"` (0x61…) < `"ab"` < `"b"`. A canonicalizer that uses a case-insensitive or locale-aware sort fails here. A canonicalizer that uses Python `sorted()` on `str` passes. A canonicalizer that sorts by length-then-lex fails. Good fuzz target.

---

## Vector 6 — Nested records sort independently

**Pressure-tests:** ADR 0008 applies per-`record` node, not globally. Each `record` sorts its own immediate fields.

**Authoring intent:** `{outer: {z: 1, a: 2}, inner: {b: 3, a: 4}}`

**Canonical:**
```
(record inner (record a (int 4) b (int 3)) outer (record a (int 2) z (int 1)))
```

**Notes:** Outer keys sort `inner` < `outer`. Inside `inner`'s value, `a` < `b`. Inside `outer`'s value, `a` < `z`. A canonicalizer that accidentally sorts all field-symbols globally (e.g. by flattening into a single list before sort) would fail here.

---

## Vector 7 — Pattern with multiple `pat-var`s plus outer binder reference

**Pressure-tests:** § 4 pattern-binding rule (last encountered `pat-var` = index 0) combined with outer-binder shift.

**Authoring intent:** `\x. match x with Pair a b -> x a` (arm body applies outer `x` to first pattern variable `a`)

**Canonical:**
```
(lam (match (var 0) (arm (pat-ctor Pair pat-var pat-var) (app (var 2) (var 1)))))
```

**Notes:** Breakdown of indices inside the arm body:
- Two `pat-var`s are in scope: textual-first is `a`, textual-last is `b`. Per § 4: `b` = `(var 0)`, `a` = `(var 1)`.
- Outer `lam`'s `x` shifts up by 2 (two pat-vars pushed): `x` = `(var 2)`.
- Body `x a` = `(app (var 2) (var 1))`.

The scrutinee `(var 0)` sits outside the arm, so it references the `lam`'s `x` directly (no pat-var shift there). This vector catches a canonicalizer that reverses the pattern-binding order (first = 0, last = highest) — a plausible error given that ADR 0007 uses position = index for `rec`, while § 4 uses reverse-position = index for patterns.

---

## Vector 8 — Hole with stable hash, standalone and embedded

**Pressure-tests:** § 7 Holes, § 9 Hashing. A `hole` is a valid AST node and its canonical text hashes like any other node.

**Authoring intent:** A parse-failed construct in two contexts — standalone, and embedded as the body of a `let`.

**Canonical (8a, standalone):**
```
(hole unbound-name (str "foo"))
```

**Canonical (8b, embedded):**
```
(let (int 1) (hole unbound-name (str "foo")))
```

**Notes:** Verifies that `(hole ...)` bytes are produced identically in both positions (so 8a's canonical text appears literally as a substring of 8b's). Verifies § 7's claim that holes hash like any other node. A secondary check: the diag-id set (§ 7 table) is frozen; a canonicalizer that accepts an unlisted diag-id should not be required to reject it at this layer (that is parser territory), but it must still render it verbatim.

---

## Vector 9 — String canonicalization: non-ASCII

**Pressure-tests:** § 3 Strings / [ADR 0010](../decisions/0010-canonical-emission-rules.md) S2 + S3. Non-ASCII code points always emit as `\u{HEX}` in shortest lowercase form.

**Authoring intent:** `"A😀"` (mixed ASCII + astral-plane emoji).

**Canonical:**
```
(str "A\u{1f600}")
```

**Notes:** Raw UTF-8 input `"A😀"` and escaped input `"A\u{0001f600}"` both canonicalize to the form above — ADR 0010 S2 forces escape, S3 forces shortest hex. Padded-hex (`\u{0001f600}`) and raw-UTF-8 forms are **not** valid canonical text for this content.

---

## Vector 10 — Arity edges: zero-children constructs

**Pressure-tests:** § 2 node-kinds table and [ADR 0011](../decisions/0011-variable-arity-minimums.md). Splits into valid canonical forms (permitted N = 0) and anti-tests (forbidden N = 0).

**Valid canonical forms:**

10a. Empty record (unit value, permitted per ADR 0011):
```
(record)
```

10b. Nullary constructor (permitted per ADR 0011):
```
(ctor Nil)
```

**Anti-tests** — the canonicalizer must *not* emit these; a compliant parser must not produce the ASTs that would serialize to them:

10c. Empty module — forbidden per ADR 0011:
```
(module)
```

10d. Body-only `rec` — forbidden per ADR 0011 (would hash-collide with the body alone):
```
(rec (int 1))
```

10e. Zero-arm `match` — forbidden per ADR 0011 (non-exhaustive by construction):
```
(match (var 0))
```

**Notes:** The rule is "permit N = 0 where the construct has a meaningful empty form." `record` and `ctor` (plus `pat-ctor`) qualify; `rec`, `module`, and `match` do not. A conformance test suite should include the anti-test cases with the expected behavior being canonicalizer refusal or parser-level rejection, not silent emission.

---

## Vector 11 — `ann` with record type

**Pressure-tests:** § 6 field-sort rule applies inside `ann` (§ 2 "Type syntax reuses expression kinds"). [ADR 0008](../decisions/0008-record-field-ordering.md) calls this out explicitly; needs a vector.

**Authoring intent:** `5 :: {y: Int, x: Int}` — a value annotated with a record type whose fields the author wrote in y-then-x order.

**Canonical:**
```
(ann (int 5) (record x (sym Int) y (sym Int)))
```

**Notes:** Fields sort `x` < `y` *inside* the type, confirming ADR 0008 reaches through `ann`. The representation of `Int` as `(sym Int)` is provisional — § 11 of the spec flags the exact type-expression subset as open. If `Int` ends up being some other kind (e.g. a nullary `ctor`), this vector's inner bytes change; the sorting rule it pressure-tests does not.

---

## Vector 12 — `proj` coverage

**Pressure-tests:** § 2 `proj` node (arity 2, children: record-expr, field-sym). `proj` is currently uncovered.

**Authoring intent:** `r.x.y` with `r` in scope as the innermost binder.

**Canonical:**
```
(proj (proj (var 0) x) y)
```

**Notes:** Field-sym is a **bare** symbol, not wrapped in `(sym ...)`. This matches `record`'s key treatment and contrasts with `str` / `int` / `var` atoms which *do* get a wrapping node. Worth an explicit vector because a canonicalizer writer who sees `(record sym₀ val₀ …)` and then reaches `proj` might wrap the field-sym by analogy with `ann`-type-expressions.

---

## Vector 13 — Signed zero normalized

**Pressure-tests:** § 3 Integers / [ADR 0010](../decisions/0010-canonical-emission-rules.md) I1. The canonicalizer normalizes `-0` to `0`.

**Authoring intent:** Any source construct that produces an integer AST node with value zero (signed or unsigned).

**Canonical:**
```
(int 0)
```

**Anti-test:** The following must *not* appear in canonical text:
```
(int -0)
```

**Notes:** ADR 0010 I1 treats any AST-level "negative zero" as identical to `0`. Two canonicalizers fed an AST `(int -0)` must both emit `(int 0)`. Standalone `0` and `-N` for N ≥ 1 are unaffected.

---

## Vector 14 — Arbitrary-precision integer

**Pressure-tests:** § 3 Integers / [ADR 0010](../decisions/0010-canonical-emission-rules.md) I2. Canonical accepts any magnitude; canonicalizers must use bignum internally.

**Canonical:**
```
(int 99999999999999999999999999999)
```

**Notes:** Well beyond u64 (≈ 1.8 × 10¹⁹) and beyond i128 (≈ 1.7 × 10³⁸ for positive range, but 29 nines = 10²⁹ − 1 fits i128). For a stronger test, use a ~50-digit value. A canonicalizer that internally parses into u64, i64, or i128 will fail here; one using `num-bigint` (Rust) or `int` (Python) passes. Runtime integer size (what the Phase 1+ evaluator uses) is a separate concern — canonical carries arbitrary precision regardless.

---

## Vector 15 — Named-escape coverage in `str`

**Pressure-tests:** § 3 Strings / [ADR 0010](../decisions/0010-canonical-emission-rules.md) S1. All five named escapes emit in their short form, not as `\u{HEX}`.

**Authoring intent:** Decoded content `hello "world"<LF><TAB>back\slash<CR>`.

**Canonical:**
```
(str "hello \"world\"\n\tback\\slash\r")
```

**Notes:** ADR 0010 S1 forces named-escape preference: byte 0x22 emits as `\"` (not `\u{22}`), byte 0x5c as `\\` (not `\u{5c}`), TAB as `\t`, LF as `\n`, CR as `\r`. S4 emits the remaining printable ASCII bytes directly. A canonicalizer that emits any named-escape byte as `\u{...}` produces non-canonical output even though it would parse to the same string.

---

## Vector 16 — Symbol-regex edges

**Pressure-tests:** § 5 symbol regex `[A-Za-z_][A-Za-z0-9_-]*` and byte-wise field-sym sorting with non-alphanumeric bytes.

**Authoring intent:** A record with four symbol field names exercising regex edges: bare underscore, leading-underscore, embedded hyphen, trailing hyphen.

**Canonical:**
```
(record _ (int 1) _foo (int 2) a- (int 3) a-b (int 4))
```

**Notes:** Byte order for the keys:
- `_` = `0x5f`
- `_foo` = `0x5f 0x66 0x6f 0x6f`
- `a-` = `0x61 0x2d`
- `a-b` = `0x61 0x2d 0x62`

Sort resolves by standard prefix-then-length rules: `_` < `_foo` (prefix match, shorter wins), `_foo` < `a-` (`0x5f` < `0x61`), `a-` < `a-b` (prefix match, shorter wins). The `a-` case also raises a minor question: the regex permits trailing hyphen, which is almost certainly an oversight — most language symbol regexes forbid it. Tacit is ASCII-restricted in Phase 0 and this is defensible to defer, but worth a sentence in § 5 confirming intent.

---

## Vector 17 — Permuted `rec` bindings produce distinct canonical

**Pressure-tests:** ADR 0007's explicit claim: "Two `rec` groups with the same bindings in different orders produce different canonical text and different hashes."

**Candidate canonicalizations** (two distinct ASTs):

17a:
```
(rec (int 1) (int 2) (var 0))
```

17b:
```
(rec (int 2) (int 1) (var 0))
```

**Notes:** This is an **anti-test**: it verifies the canonicalizer does *not* sort or reorder rec bindings. Under ADR 0007, 17a's body `(var 0)` refers to the binding at position 0 (`(int 1)`), while 17b's `(var 0)` refers to `(int 2)`. A canonicalizer that canonicalizes by sorting bindings would collapse these, incorrectly equating two semantically-distinct programs. The same principle applies to `module`, `match` arms, and `ctor` arguments — all must preserve user order. One vector covers the class.

---

## Vector 18 — Deep nesting

**Pressure-tests:** No accidental size or depth limits in the emit path. A 10-deep lambda stack with a reference to the outermost parameter.

**Authoring intent:** `\x1. \x2. \x3. \x4. \x5. \x6. \x7. \x8. \x9. \x10. x1`

**Canonical:**
```
(lam (lam (lam (lam (lam (lam (lam (lam (lam (lam (var 9)))))))))))
```

**Notes:** Inside the innermost body, `x10` = `(var 0)`, `x9` = `(var 1)`, …, `x1` = `(var 9)`. Counts the closing parens (11) to guard against off-by-one emit errors. Not deep enough to stress a real implementation, but deep enough that a canonicalizer with a too-eager whitespace-insertion bug or a stack-depth-aware pretty-printer will visibly misbehave.

---

## Vector 19 — `match` arm order preserved

**Pressure-tests:** § 6 explicit claim that `match` arms preserve user order (first-match-wins semantics).

**Candidate canonicalizations** (two distinct ASTs, the second is the correct program):

19a (wild-match first — always fires, `Zero` arm unreachable):
```
(match (var 0) (arm pat-wild (int 1)) (arm (pat-ctor Zero) (int 0)))
```

19b (`Zero` first, wild-match fallback):
```
(match (var 0) (arm (pat-ctor Zero) (int 0)) (arm pat-wild (int 1)))
```

**Notes:** Sibling anti-test to Vector 17. A canonicalizer that sorts arms by hash would equate these — a silent correctness bug for any first-match-wins program. Also exercises nullary `pat-ctor` (`(pat-ctor Zero)` with 0 sub-patterns), a subset of Vector 10b's gap.

---

## Vector 20 — `hole` diag-id sweep

**Pressure-tests:** § 7 diag-id table coverage. Six sub-vectors, one per listed diag-id, with minimal (empty) payload strings.

**Canonical (one per diag-id):**
```
(hole unexpected-token (str ""))
(hole unclosed-paren (str ""))
(hole expected-expr (str ""))
(hole expected-pattern (str ""))
(hole unbound-name (str ""))
(hole arity-mismatch (str ""))
```

**Notes:** Coverage sweep — confirms every listed diag-id round-trips. Pairs with Vector 8 (hole in context, stable hash). Empty payload string `""` is a zero-content `str`, also exercising the minimal-string case (confirms `""` is canonical, not `"\u{}"` or anything weirder). If § 11 of the spec grows the diag-id set during Phase 1, this vector expands additively.

---

## Vector 21 — 50-digit positive integer (bignum stress)

**Pressure-tests:** § 3 Integers / [ADR 0010](../decisions/0010-canonical-emission-rules.md) I2. Raises the bar on "use bignum internally" beyond Vector 14.

**Canonical:**
```
(int 12345678901234567890123456789012345678901234567890)
```

**Notes:** 50 decimal digits (~2¹⁶⁶). Well beyond i128 (max ≈ 1.7 × 10³⁸). A canonicalizer that parses into i128 truncates, wraps, or panics; one using `num-bigint` (Rust) or Python's native `int` round-trips. Pairs with V14 — same decision (ADR 0010 I2), stronger stress. V14 could arguably be retired in favor of this, but keeping both cheaply documents the graduated threshold.

---

## Vector 22 — Embedded NUL in string (`\u{0}` exception)

**Pressure-tests:** § 3 Strings / [ADR 0010](../decisions/0010-canonical-emission-rules.md) S3. The one documented exception to "shortest-hex, no leading zeros" is `\u{0}` for U+0000.

**Authoring intent:** Decoded content `a<NUL>b`.

**Canonical:**
```
(str "a\u{0}b")
```

**Notes:** NUL (0x00) is a control byte, not ASCII-printable, so S2 forces escape. Under S3, minimum-digit form is `\u{0}` — one hex digit even though the "no leading zeros" rule would otherwise forbid a bare zero. A canonicalizer that pads to two hex digits (`\u{00}`), pads to byte-width (`\u{00000000}`), or rejects embedded NUL outright fails. Pairs with V9 (shortest-hex for non-ASCII) and V15 (named escapes).

---

## Vector 23 — Maximum code point `\u{10ffff}`

**Pressure-tests:** § 3 Strings / [ADR 0010](../decisions/0010-canonical-emission-rules.md) S3. Max valid Unicode code point, 6 hex digits, no leading zero padding.

**Authoring intent:** A string consisting of the single character U+10FFFF.

**Canonical:**
```
(str "\u{10ffff}")
```

**Notes:** Exercises the upper bound of the spec's 1–6 hex-digit `\u{HEX}` range. Confirms lowercase and no padding. A canonicalizer using a fixed-width hex formatter (`\u{0010ffff}`) fails; one using `format!("{:x}", cp)` (Rust) or `f"{cp:x}"` (Python) passes. Triangulates with V9 (5 digits) and V22 (1 digit) to cover the full width range.

---

## Vector 24 — Non-scalar code points (anti-tests per ADR 0012)

**Pressure-tests:** § 3 Strings / [ADR 0012](../decisions/0012-unicode-scalar-value-restriction.md). `\u{HEX}` escapes must denote Unicode scalar values (U+0000–U+D7FF or U+E000–U+10FFFF). Surrogates and out-of-range values are hard parse errors.

**Anti-tests** — the parser must reject each of these inputs; a compliant canonicalizer must not emit them:

24a. Surrogate range (low):
```
(str "\u{d800}")
```

24b. Surrogate range (high):
```
(str "\u{dfff}")
```

24c. Out-of-range (just past the Unicode maximum):
```
(str "\u{110000}")
```

24d. Out-of-range (maximum 6-digit value):
```
(str "\u{ffffff}")
```

**Valid boundary cases** (must be accepted, to pin the boundary from both sides):

24e. Just below the low surrogate:
```
(str "\u{d7ff}")
```

24f. Just above the high surrogate:
```
(str "\u{e000}")
```

**Notes:** Pairs with Vector 23 (max valid `\u{10ffff}`). Together V23 and V24c pin the upper edge; V24e/V24a and V24b/V24f pin the surrogate gap from both sides. A canonicalizer that uses Rust `String::from_utf8` or `char::from_u32` gets the surrogate and out-of-range rejections for free; one using `u32`-indexed byte arrays must add the explicit range check described in ADR 0012. Failure mode is a lexer-level hard error inside the string literal, not a `(hole invalid-escape ...)` node — § 7 diag-ids are not expanded by this vector.

---

## Vector 25 — `pat-var` under inner `lam` (stacked DeBruijn shifts)

**Pressure-tests:** § 4 pattern binding + `lam` binder, combined. A canonicalizer that handles each shift in isolation but drops a count across the boundary fails.

**Authoring intent:** `lambda f. match f with Just x -> lambda y. x y` — a pattern-bound `x` crosses into an inner lambda's body.

**Canonical:**
```
(lam (match (var 0) (arm (pat-ctor Just pat-var) (lam (app (var 1) (var 0))))))
```

**Notes:** DeBruijn trace inside the innermost `app`:
- Innermost `lam` introduces `y` = `(var 0)`.
- `pat-var` in the arm bound `x`, which under the inner `lam` shifts by 1: `x` = `(var 1)`.
- Outer `lam`'s `f` shifts by 2 (one pat-var + one inner lam): `f` = `(var 2)`, unreferenced here.
- `x y` = `(app (var 1) (var 0))`.

Scrutinee `(var 0)` sits outside the arm, so no pat-var shift applies — it references the outer `lam`'s `f` directly. Sibling of V7, but stacks an extra `lam` past the `pat-var` instead of referencing it directly — catches a canonicalizer that tracks pattern-shift and binder-shift in separate counters without composing them.

---

## Vector 26 — `ctor` with mixed-type arguments

**Pressure-tests:** § 2 `ctor` arity surface with heterogeneous child kinds (`var`, `int`, nested `ctor`). Closes out ctor coverage.

**Authoring intent (context):** `Triple x 42 Nil` with `x` = `(var 0)` in scope.

**Canonical:**
```
(ctor Triple (var 0) (int 42) (ctor Nil))
```

**Notes:** Confirms `ctor`'s name-sym is a bare symbol (matching `record` keys and `proj` fields, unlike `sym` atoms which wrap) and that arg₀…arg_{N-1} accept any expression kind in any combination. Nested `(ctor Nil)` reuses V10b's permitted-N=0 form. Adds no new spec rules — pure surface coverage.

---

## Vector 27 — `rec` with single binding (N=1 edge)

**Pressure-tests:** [ADR 0011](../decisions/0011-variable-arity-minimums.md) — `rec` requires N≥1. The N=1 form is the minimum legal shape and distinguishes "no rec at all" (which would just be `let` or bare expression) from "rec with exactly one binding."

**Authoring intent:** `rec { x = x } in x` — degenerate self-referential binding.

**Canonical:**
```
(rec (var 0) (var 0))
```

**Notes:** Binding 0's RHS refers to itself as `(var 0)` per ADR 0007; body also references binding 0 as `(var 0)`. Two `(var 0)` tokens that denote the same name at the AST level (the single binding introduced by the `rec`) and must not be collapsed by any peephole optimization at the canonical layer. A canonicalizer that special-cases N=1 (rewriting to `let` or to the bare body) changes the hash — forbidden. Pairs with V17 (N=2 order preservation) and V10d (N=0 forbidden).

---

## Vector 28 — `module` with N=1 real binding

**Pressure-tests:** § 5 `module` with N≥1, otherwise uncovered (V10c's `(module)` is only the forbidden-form anti-test).

**Authoring intent:** A module exporting a single identity function.

**Canonical:**
```
(module (lam (var 0)))
```

**Notes:** Binding 0 is `(lam (var 0))` — an identity function. Inside that `lam`'s body the parameter is `(var 0)`; the module-level self-reference (if it were used) would be `(var 1)` shifted by the lam, but is unreferenced here. Confirms `module` has no body slot, contra `rec`. A canonicalizer that emits `(module (lam (var 0)) )` with trailing whitespace, or that treats `module` as `rec` with body = last binding, diverges on this vector.

---

## Vector 29 — Generic identity function (`forall` + `fn-ty` + `ty-var` + empty `eff-set`)

**Pressure-tests:** [ADR 0034](../decisions/0034-p2-type-subset-ann.md) `fn-ty`, `ty-var`, `forall` tags; [ADR 0035](../decisions/0035-p2-effect-set-canonical.md) empty `eff-set` as the "pure" bottom. Closes the § 11 "Type syntax inside `ann`" open item.

**Authoring intent:** `id :: ∀a. a -> a` — the identity lambda annotated with a generic, pure function type.

**Canonical:**
```
(ann (lam (var 0)) (forall 1 0 (fn-ty (ty-var 0) (ty-var 0) (eff-set))))
```

**Notes:** Encoding decomposition: `forall 1 0` introduces 1 type variable and 0 effect variables. Inside the body, `(ty-var 0)` is the sole type variable bound by this `forall`. The `fn-ty` carries arg, ret, and eff children — both arg and ret are `(ty-var 0)`; eff is `(eff-set)` (empty atom list, the pure bottom of the lattice per ADR 0035). The annotated value `(lam (var 0))` is independent of the type expression — `(var 0)` is a value-level DeBruijn index inside the `lam`, while `(ty-var 0)` is a type-level DeBruijn index inside the `forall`. Type-variable and value-variable index spaces are disjoint (ADR 0034 § DeBruijn detail).

Supersedes the originally-drafted V29 placeholder, which proposed three candidate function-type encodings (`(ctor Arrow …)`, `(ctor -> …)`, a new `fn-type` tag). ADR 0034 picked the new-tag option (renamed `fn-ty` for the 5-byte limit) and added the `forall` quantifier so generics could be expressed without an implicit-quantification convention.

---

## Vector 30 — Monomorphic IO-annotated function type (`fn-ty` with concrete operands and one effect atom)

**Pressure-tests:** [ADR 0034](../decisions/0034-p2-type-subset-ann.md) `fn-ty` with `(sym ...)` operand types (the simplest type expression); [ADR 0035](../decisions/0035-p2-effect-set-canonical.md) singleton `eff-set`.

**Authoring intent:** `f :: Int -> Int / IO` — a function annotated with concrete operand types and a single declared effect.

**Canonical:**
```
(ann (lam (var 0)) (fn-ty (sym Int) (sym Int) (eff-set IO)))
```

**Notes:** No `forall` wraps this signature — there are no free type or effect variables. `(eff-set IO)` exercises the singleton-atom case of ADR 0035; sort-order and lattice rules are trivially satisfied. The `(sym Int)` shape (rather than e.g. `(ctor Int)` or a built-in tag) is what ADR 0034 specifies for base type names; the typechecker — not the canonical parser — decides which symbols denote types.

Pairs with V29 (no effect, generic) and V31 (effect-polymorphic) to triangulate the three combinations of fixed/variable type and effect components in `fn-ty`.

---

## Vector 31 — Effect-polymorphic identity (`forall` with both type and effect bindings, `eff-var`)

**Pressure-tests:** [ADR 0034](../decisions/0034-p2-type-subset-ann.md) `forall N M body` with M ≥ 1; [ADR 0036](../decisions/0036-p2-effect-polymorphism-syntax.md) `eff-var` and the disjoint type/effect variable index spaces.

**Authoring intent:** `id :: ∀(a, e). a -> a / e` — an identity function that transparently propagates its caller's effects.

**Canonical:**
```
(ann (lam (var 0)) (forall 1 1 (fn-ty (ty-var 0) (ty-var 0) (eff-var 0))))
```

**Notes:** `forall 1 1` introduces one type variable and one effect variable simultaneously. Inside the body, `(ty-var 0)` and `(eff-var 0)` both denote position-0 of their respective binding lists; the tag distinguishes them, so a canonicalizer that conflated their index spaces would still produce different bytes here than for V29 (which uses `(eff-set)` instead).

ADR 0036 caps M ≤ 1 in Phase 2; this vector exercises that limit. M ≥ 2 would model row polymorphism, deferred to Tacit-Full. Pairs with V29/V30 to cover the polymorphism axis.

---

## Vector 32 — `pat-int` in a `match` arm

**Pressure-tests:** [ADR 0037](../decisions/0037-p2-pat-int.md) integer-literal pattern; arm ordering (V19) preserved with a mixed `pat-int` / `pat-wild` arm list.

**Authoring intent:** `match 5 with | 5 -> 1 | _ -> 0`

**Canonical:**
```
(match (int 5) (arm (pat-int 5) (int 1)) (arm pat-wild (int 0)))
```

**Notes:** `pat-int` is arity 1 with a single decimal-int child following the same grammar as `(int N)` (negative integers permitted, `-0` normalizes to `0` per ADR 0010 I1). It introduces no binders — the arm body's DeBruijn scope is unchanged, exactly as for `pat-wild`. Arm order is preserved by the canonicalizer (per spec § 6, sibling rule to V17 and V19); a canonicalizer that sorted arms by hash would silently change which arm fires first.

Codegen (Stage 4 of Phase 2) lowers this as `icmp eq scrutinee, 5`; the wildcard fall-through is the existing pattern-dispatch behavior. Unblocks smoke #7 (`match-int.tac`).

---

## Vector 33 — Stack-allocated writable buffer via `let` + `@buf-alloc`

**Pressure-tests:** [ADR 0038](../decisions/0038-p2-writable-buffer.md) writable-buffer binding model. Notably uses **only existing** canonical tags (`let`, `app`, `sym`, `int`, `var`) — the design decision was to express buffer allocation through the established `@name` primitive surface ([ADR 0028](../decisions/0028-phase-1-libc-call-surface.md)) rather than introducing a new node kind.

**Authoring intent:** `let buf = @buf-alloc 256 in @read 0 buf 256` — allocate a 256-byte stack buffer, then fill it from stdin.

**Canonical:**
```
(let (app (sym buf-alloc) (int 256)) (app (app (app (sym read) (int 0)) (var 0)) (int 256)))
```

**Notes:** `(sym buf-alloc)` is the new STACK-ALLOC primitive added by ADR 0038 to the `@name` allowlist. `(var 0)` in the `let` body is the buffer handle; `read` is curried-applied with arguments fd=0, buf=`(var 0)`, count=256. Per ADR 0038, the buffer's lifetime is the `let` scope and the typechecker rejects any expression that would let the handle escape (e.g., capture in a closure or return as a value). The `Mut` augmentation of `read`'s effect when its second argument is a `Buf N` is also a typechecker concern — `libc-effects.toml` continues to record only `IO` for `read`, unchanged from ADR 0025.

Round-trips through the **Phase 1** canonical parser unchanged: the canonical-format extension for V33 is purely a typechecker-level interpretation of an existing canonical shape. Unblocks smoke #8 (`echo.tac`).

---

## 30. Spec gaps surfaced and resolved

Seven spec gaps surfaced across the two vector-drafting rounds. Six closed via ADRs 0010 and 0011 on 2026-04-21; one closed via ADR 0012 on 2026-04-22. One remains deferred as low-urgency.

| Gap | Source | Resolution |
|-----|--------|------------|
| `\u{HEX}` emission form not pinned (shortest vs padded) | Vector 9 | **ADR 0010 S3** — shortest lowercase hex, no leading zeros (except `\u{0}` for NUL). |
| Raw vs escaped non-ASCII in `str` not pinned | Vector 9 | **ADR 0010 S2** — all non-ASCII emitted as `\u{HEX}`; canonical text is strictly 7-bit ASCII. |
| Named-escape vs `\u{...}` emission for ASCII specials not pinned | Vector 15 | **ADR 0010 S1** — named escape wherever one exists; `\u{HEX}` only for code points without one. |
| Minimum `N` for variable-arity kinds not stated | Vector 10 | **ADR 0011** — `record`/`ctor`/`pat-ctor` permit N≥0; `rec`/`match`/`module` require N≥1. § 2 kind table updated inline. |
| Signed zero `-0` syntactically valid but semantically redundant | Vector 13 | **ADR 0010 I1** — `-0` normalized to `0` at canonicalizer input; canonical text never contains `-0`. |
| Integer range unbounded — two-impl divergence risk | Vector 14 | **ADR 0010 I2** — canonical accepts arbitrary-precision decimal; implementations must use bignum. Runtime size is Phase 1+. |
| `\u{HEX}` permits surrogates (U+D800–U+DFFF) and out-of-range (> U+10FFFF) values | Vector 24 | **ADR 0012** — hex value must be a Unicode scalar value; non-scalar values are hard parse errors. § 3 Strings updated inline. |
| Symbol regex permits trailing `-` (probable oversight) | Vector 16 | **Deferred** — low urgency; regex is ASCII-only in Phase 0 and no corpus programs hinge on it. Revisit if Phase 1 parser hits an issue. |

With those seven closed, vectors 9, 10, 13, 14, 15, 24 are all pinned — the "candidate canonicalization" framing is gone from each, and the listed bytes are final for Stage 2.

Vectors 1–8, 11, 12, 16, 17, 18, 19, 20, 21, 22, 23, 25, 26, 27, 28 pressure-test existing spec rules (ADRs 0005–0012) and should pass under any spec-conformant canonicalizer. They did not require new decisions.

## 31. Coverage status

### Phase 0 Stage 2 (frozen 2026-04-22 by [ADR 0013](../decisions/0013-canonical-text-format-frozen.md))

Stage 2 exit criterion 2 asked for ~30 vectors. The § 22 (previously-planned) coverage list closed as follows:

| Coverage item | Status | Vector(s) |
|---------------|--------|-----------|
| Int boundaries, positive side | Drafted | V21 (50-digit) strengthens V14 |
| String Unicode: embedded NUL | Drafted | V22 |
| String Unicode: max `\u{10ffff}` | Drafted | V23 |
| String Unicode: surrogate range / out-of-range | Drafted, pinned by [ADR 0012](../decisions/0012-unicode-scalar-value-restriction.md) | V24 (six sub-vectors) |
| Nested binders crossing pat + lam | Drafted | V25 |
| `ctor` with mixed-type arguments | Drafted | V26 |
| `rec` with single binding | Drafted | V27 |
| `module` with real bindings | Drafted | V28 |
| Type-expression subset | Deferred to Phase 2 | V29 placeholder reserved |

Stage 2 exited on V1–V28 (V29 excluded pending the type-subset ADR). Phase 1 was subsequently frozen by [ADR 0033](../decisions/0033-phase-1-frozen.md).

### Phase 2 Stage 1 additions (2026-04-27)

Five vectors added alongside ADRs 0034–0038. Each pressure-tests a specific Phase 2 spec amendment:

| Coverage item | Vector | ADR(s) |
|---------------|--------|--------|
| Generic function type with empty effect set | V29 | 0034, 0035 |
| Concrete function type with singleton effect | V30 | 0034, 0035 |
| Effect-polymorphic function (one type var + one effect var) | V31 | 0034, 0035, 0036 |
| Integer literal pattern in `match` | V32 | 0037 |
| Stack-allocated writable buffer (no new tags) | V33 | 0038 |

V29–V32 require canonical-parser extensions for the new tags (`fn-ty`, `ty-var`, `forall`, `eff-set`, `eff-var`, `pat-int`); landing those parser extensions is Phase 2 Stage 2 work. V33 round-trips through the Phase 1 canonical parser today since it uses only existing tags.

### Vector files

The file-per-vector split landed 2026-04-22 as [`test-vectors/`](test-vectors/) alongside this doc. Phase 0 contributed 45 files (28 primary vectors, with V8/V10/V17/V19/V20/V24 expanding into sub-vector files); Phase 2 Stage 1 added 5 files (V29–V33), bringing the total to 50. Extensions are `.canonical` (must emit), `.forbidden` (must not emit), `.reject` (parser hard error). The directory README enumerates the minimum test set implementations run to demonstrate byte-equivalence.
