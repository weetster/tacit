# Canonical Format — Stage 2 Pressure-Test Vectors (Draft)

**Status:** Draft, pre-freeze review
**Parent:** [canonical-text-format.md](canonical-text-format.md)
**Purpose:** Ten vectors chosen to pressure-test the spec before Stage 2 exit. Each targets a specific rule, an anti-intuitive convention, or a suspected ambiguity. Several vectors are expected to **surface spec gaps** — those are flagged explicitly in the Notes and summarized in § 11.

The file-per-vector format (§ 11 of the spec, open item 3) is deferred; this is one doc so the set can be reviewed as a whole. When the set converges, split into one AST per file.

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

## Vector 9 — String canonicalization edges **(spec gap)**

**Pressure-tests:** § 5 Strings / [ADR 0006](../decisions/0006-canonical-lexical-rules.md). Drafting this vector surfaced two spec ambiguities that must be resolved before freeze.

**Authoring intent:** Three strings all denoting the same Unicode text, with different source encodings.

**Candidate canonicalizations** (all three are claims the spec does *not* currently disambiguate between):

9a. `\u{...}` escape, shortest hex form:
```
(str "A\u{1f600}")
```

9b. `\u{...}` escape, padded hex form:
```
(str "A\u{0001f600}")
```

9c. Raw UTF-8 non-ASCII (since § 5 only forbids raw newlines, tabs, and control bytes — not raw non-ASCII):
```
(str "A😀")
```

**Spec gaps:**

1. **`\u{HEX}` emission form.** ADR 0006 says "1–6 lower-case hex digits, no leading-zero requirement." "No leading-zero requirement" reads as parser permissiveness ("leading zeros are not required"). The canonicalizer's *emission* form is not pinned. 9a and 9b differ in bytes, match in decoded semantics. **Must pin to one form** (recommend: shortest, minimum digits, lowercase).
2. **Raw vs escaped non-ASCII.** § 5 forbids only raw newlines/tabs/controls in string literals, which implicitly permits raw non-ASCII. The canonicalizer must decide whether to emit 9a or 9c. **Must pin to one form** (recommend: all non-ASCII emitted as `\u{...}` with the shortest-hex rule from (1), so canonical text is always ASCII-printable, matching the "simplifies transport" consequence already claimed in ADR 0006).

Recommend an additional ADR 0010 to close both — they are entangled and one ADR covers both neatly.

---

## Vector 10 — Arity edges: zero-children constructs **(spec gap)**

**Pressure-tests:** § 2 node-kinds table, specifically the arity expressions `2N`, `1+N`, and `N`. None of them explicitly state the minimum N.

**Candidate canonicalizations:**

10a. Empty record (2N where N=0):
```
(record)
```

10b. Nullary constructor (1+N where N=0):
```
(ctor Nil)
```

10c. Empty module (N where N=0):
```
(module)
```

10d. `rec` with body only, no bindings (1+N where N=0):
```
(rec (int 1))
```

10e. `match` with no arms (1+N where N=0):
```
(match (var 0))
```

**Spec gaps:**

1. **Empty `record`.** Semantically unambiguous (the unit record). The spec should either explicitly permit `(record)` or forbid it. Currently silent.
2. **Nullary `ctor`.** Required for constructors like `Nil`, `None`, `Unit`. § 2 says `1+N` — if N=0 is permitted, `(ctor Nil)` is canonical. Almost certainly intended but not explicitly stated.
3. **Zero-binding `rec` and empty `module`.** Likely should be rejected at the parser layer (nothing useful to express), but the canonical form's job is to render any well-formed AST. Need a rule: "the parser produces `Hole` rather than empty `rec`/`module`," or "empty forms are permitted and canonical." Either is fine; silence is not.
4. **Zero-arm `match`.** Semantically ill-formed (non-exhaustive, never matches). Same question as (3).

Recommend a one-line addition to § 2 per-row specifying the minimum for each `N`, or a § 2 footer stating the general rule. Lowest-friction: add a "Min N" column next to "Arity" for every row where N appears.

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

## Vector 13 — Signed zero **(spec gap)**

**Pressure-tests:** § 3 Integers. Drafting this vector surfaced a third spec ambiguity.

**Candidate canonicalizations:**

13a. Signed zero preserved:
```
(int -0)
```

13b. Signed zero normalized to unsigned:
```
(int 0)
```

**Spec gap:** § 3's integer grammar reads "No leading zeros except the single digit `0`. Negative integers have a leading `-`." Strictly parsed, the rule applies to the digit sequence after the sign — `-0` has a digit sequence of `0`, which is allowed. So `-0` is syntactically valid. But it denotes the same value as `0`, which means two canonical forms for one semantic integer — a direct byte-equivalence violation.

**Must pin** one of:
- Reject `-0` at the canonical layer (emit `(hole arity-mismatch …)` or similar — but arity-mismatch is wrong fit; may want a new diag-id).
- Normalize `-0` to `0` at the canonicalizer input, so canonical text never contains `-0`.
- Document `-0` as distinct from `0` (only defensible if Tacit-Lite has IEEE-754-style signed zero semantics, which it does not for integers).

Recommend normalization to `0`, documented as a one-line canonicalizer rule in ADR 0006.

---

## Vector 14 — Unbounded integer **(spec gap)**

**Pressure-tests:** § 3 Integers. Spec places no upper or lower bound on integer literals.

**Candidate canonicalization:**
```
(int 99999999999999999999999999999)
```

**Spec gap:** Canonical text happily carries arbitrary decimal. But two canonicalizers written against the spec may internally use different int types (one u64, one i128, one bignum) and diverge: one succeeds, one truncates, one panics. The canonical text layer is not where this must be resolved — the hash domain doesn't care — but Stage 2's exit criterion ("two independent implementations produce byte-identical canonical text on the same AST") *does* care if "the same AST" is interpretable differently.

**Must pin** one of:
- Declare canonical text accepts arbitrarily-large decimal integers; implementations must use arbitrary-precision internally.
- Pin a range (e.g. i64 / two's-complement 64-bit) and forbid out-of-range integer literals at the canonical layer.

Recommend the first, with a note that runtime integer size is a Phase 1+ concern (separate from canonical representation).

---

## Vector 15 — Escape-sequence coverage in `str` **(spec gap, minor)**

**Pressure-tests:** § 5 / ADR 0006 string escape table, for the non-`\u{...}` escapes.

**Candidate canonicalization** (decoded content: `hello "world"<LF><TAB>back\slash<CR>`):
```
(str "hello \"world\"\n\tback\\slash\r")
```

**Notes & spec gap:** ADR 0006's table lists `\"`, `\\`, `\n`, `\t`, `\r` as "allowed escape sequences" — naturally read as *parser* permissiveness. The canonicalizer's *emission* rule for a byte like `"` (0x22) is unspecified — it could emit `\"` or `\u{22}`, both legal per the parser's rules. Same for the other four. Closely related to Vector 9's spec gap (1); the fix likely covers both.

**Must pin**: canonicalizer emits the named escape (`\"`, `\\`, `\n`, `\t`, `\r`) wherever it applies; `\u{...}` only for code points without a named escape. Combined with Vector 9's recommendation, this fully pins string canonicalization.

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

## 21. Spec gaps surfaced by this draft

Consolidated for the Stage 2 freeze checklist:

| Gap | Source | Action |
|-----|--------|--------|
| `\u{HEX}` emission form not pinned (shortest vs padded) | Vector 9 (1) | New ADR or amendment to ADR 0006: "canonicalizer emits shortest-hex lowercase form." |
| Raw vs escaped non-ASCII in `str` not pinned | Vector 9 (2) | Same ADR: "canonicalizer escapes all non-ASCII code points as `\u{...}`." |
| Named-escape vs `\u{...}` emission for ASCII specials not pinned | Vector 15 | Same ADR: "canonicalizer emits named escape when one exists, `\u{...}` only otherwise." |
| Minimum `N` for variable-arity kinds not stated | Vector 10 | Add Min-N column to § 2 node-kinds table, or equivalent prose. |
| Signed zero `-0` syntactically valid but semantically redundant | Vector 13 | Canonicalizer rule in ADR 0006: "normalize `-0` to `0`; canonical text never contains `-0`." |
| Integer range unbounded — two-impl divergence risk | Vector 14 | Canonical-level note: "canonical accepts arbitrary-precision decimal integers; implementations must use bignum. Runtime size is Phase 1+." |
| Symbol regex permits trailing `-` (probable oversight) | Vector 16 | Either tighten the regex in § 5 or add a one-line intent statement. Low urgency. |

Vectors 1–8, 11, 12, 16, 17, 18, 19, 20 confirm existing spec rules and should pass under any spec-conformant canonicalizer. Vectors 9, 10, 13, 14, 15 will *change shape* once the gaps close; their exact canonical bytes above are illustrative of the ambiguity, not final. The three string-related gaps (9, 15) are entangled and should be closed together in one ADR.

## 22. Remaining coverage for the ~30-vector set

Stage 2 exit criterion 2 asks for ~30 vectors. With 20 drafted here, ~10 remain to cover:

- **Int boundaries, positive side:** a single very-large positive (covered by Vector 14 once range pins), `i64::MAX` boundary if the spec adopts a range.
- **String Unicode edge cases:** `\u{0}` (embedded null — permitted by spec, worth an explicit vector), surrogate-range code points (U+D800–U+DFFF — spec should reject these but doesn't say so), max 6-hex-digit `\u{10ffff}`.
- **Nested binders crossing pattern and lambda:** a `pat-var` inside an `arm` body that itself contains a `lam` — catches a canonicalizer that fails to combine the shifts correctly.
- **`ctor` with mixed-type arguments:** e.g. a constructor taking a `var`, an `int`, and a nested `ctor`. Low risk but closes out the `ctor` arity surface.
- **`rec` with a single binding:** degenerate but legal N=1 case; distinguishes "no intervening binder" from "one binder".
- **`module` with real bindings (N>0):** currently uncovered except by the 10c gap. A `(module (lam (var 0)))` vector fixes that.
- **Type-expression subset exploration:** as § 11 of the spec notes, the subset of expressions valid inside `ann` is an open item. A vector with function-type annotation (`A -> B` form, whatever it ends up as) pressure-tests the subset decision when it lands.

These are mechanical to write once Stage 2's three open ADRs (string emission, arity minimum, `-0` normalization) land.
