# Canonical Text Format

**Status:** Frozen 2026-04-22 ([ADR 0013](../decisions/0013-canonical-text-format-frozen.md))
**Parent:** [phase-0-plan.md](phase-0-plan.md)
**Authoring view source:** [candidates/authoring-bpe-compact.md](candidates/authoring-bpe-compact.md)

The canonical text format is the byte-exact, deterministic textual representation of a Tacit-Lite AST. It is what BLAKE3 hashes, what content-addresses identify, and what two independent implementations must produce identically for the same AST.

It is **not** what humans or AI write. The authoring view ([ADR 0003](../decisions/0003-authoring-view-bpe-compact.md)) handles writing; the inspection view (Stage 3) handles reading. Both are projections from this canonical form via a sidecar of display metadata. See [phase-0-plan.md](phase-0-plan.md) and the chat in `decisions/0005-canonical-surface-form.md` for why the three views are separate.

## Glossary

- **DeBruijn index** — a way of referring to variables by counting binders out from the use site. `lambda x. lambda y. x` becomes `(lam (lam (var 1)))` because `x` is one binder out. Two programs that differ only in variable names produce identical canonical bytes.
- **S-expression** — `(kind child child ...)`. Lisp-style; trivially parseable.
- **Content address** — a hash that identifies a value by its content, not its location. Identical content → identical hash.
- **Merkle structure** — the property that any change to a node bubbles up through every parent's hash; sibling subtrees keep their addresses.

## 1. Surface form

S-expressions with short ASCII keyword tags ([ADR 0005](../decisions/0005-canonical-surface-form.md)):

```
(kind child1 child2 ...)
```

`kind` is a short keyword (`lam`, `app`, etc.) drawn from a fixed table (§ 2). `child1 …` are either nested s-expressions or atomic tokens (decimal integers, double-quoted strings, bare symbols).

## 2. Node kinds

Frozen at Stage 2 exit. Additive evolution only — never re-purpose a tag.

| Tag        | Arity       | Children                                      | Notes                                              |
|------------|-------------|-----------------------------------------------|----------------------------------------------------|
| `lam`      | 1           | body                                          | Parameter is implicit; body sees `(var 0)`.        |
| `app`      | 2           | function, argument                            | Left-associative in authoring view; explicit here. |
| `let`      | 2           | rhs, body                                     | Binder implicit; body sees `(var 0)`.              |
| `rec`      | 1+N, N≥1    | binding₀, binding₁, …, body                   | Per [ADR 0004](../decisions/0004-rec-arity.md). DeBruijn convention in § 5. N=0 forbidden per [ADR 0011](../decisions/0011-variable-arity-minimums.md). |
| `module`   | N, N≥1      | binding₀, binding₁, …                         | Per ADR 0004. No body. Same DeBruijn rule. N=0 forbidden per ADR 0011. |
| `if`       | 3           | cond, then, else                              |                                                    |
| `match`    | 1+N, N≥1    | scrutinee, arm₀, …, armₙ                      | Arm order preserved (§ 7). N=0 forbidden per ADR 0011. |
| `arm`      | 2           | pattern, body                                 |                                                    |
| `record`   | 2N, N≥0     | sym₀, val₀, sym₁, val₁, …                     | Field-sym/value pairs sorted by sym (§ 6, [ADR 0008](../decisions/0008-record-field-ordering.md)). Empty record `(record)` permitted per ADR 0011. |
| `proj`     | 2           | record-expr, field-sym                        |                                                    |
| `ctor`     | 1+N, N≥0    | name-sym, arg₀, …                             | Constructor application. Nullary `(ctor Nil)` permitted per ADR 0011. |
| `ann`      | 2           | expression, type-as-expression                | Type syntax reuses expression kinds.               |
| `var`      | 1           | decimal int                                   | DeBruijn index, ≥ 0.                               |
| `int`      | 1           | decimal int                                   | Integer literal; may be negative. Emission rules in § 3 / [ADR 0010](../decisions/0010-canonical-emission-rules.md). |
| `str`      | 1           | double-quoted string                          | Emission rules in § 3 / ADR 0010.                  |
| `sym`      | 1           | bare symbol                                   | Used for `@foo`-style symbols.                     |
| `hole`     | 2           | diag-id-sym, payload-str                      | Parser-error placeholder (§ 8).                    |
| `pat-wild` | 0           | —                                             | `_` pattern.                                       |
| `pat-var`  | 0           | —                                             | Variable-binding pattern; binder implicit.         |
| `pat-ctor` | 1+N, N≥0    | name-sym, sub-pattern₀, …                     | Constructor pattern. Nullary `(pat-ctor Zero)` permitted per ADR 0011. |

Tags are 3–8 ASCII bytes; size is dominated by structure, not tag length.

## 3. Lexical rules

Per [ADR 0006](../decisions/0006-canonical-lexical-rules.md):

- Tokens are separated by exactly one ASCII space (`0x20`).
- No whitespace immediately inside `(` or `)`.
- No leading or trailing whitespace at any nesting level. The whole canonical text has no trailing newline.
- No comments. Comments live in the sidecar.
- Atomic tokens: decimal integers, double-quoted strings, bare symbols, `(`, `)`.

### Integers

Decimal ASCII. No leading zeros except the single digit `0`. Negative integers have a leading `-`. No `+`, no thousands separators, no underscores.

Valid: `0`, `1`, `42`, `-7`.
Invalid: `01`, `+5`, `1_000`, `0x10`.

**Emission rules** (per [ADR 0010](../decisions/0010-canonical-emission-rules.md)):

- **`-0` normalizes to `0`.** The canonicalizer never emits the two-byte sequence `-0` as a complete integer token. Any AST-level "negative zero" is treated as identical to `0`.
- **Arbitrary precision.** Canonical text accepts integer literals of any magnitude. Canonicalizer implementations must use arbitrary-precision integers at the canonical layer — no bounded integer type may truncate, wrap, or panic. Runtime integer representation is a Phase 1+ concern, separate from canonical emission.

### Strings

Double-quoted. UTF-8 source bytes accepted; the canonicalizer emits a normalized byte sequence per the rules below.

**Parser accepts** (per [ADR 0006](../decisions/0006-canonical-lexical-rules.md), tightened by [ADR 0012](../decisions/0012-unicode-scalar-value-restriction.md)): `\"`, `\\`, `\n`, `\t`, `\r`, and `\u{HEX}` for 1–6 lowercase hex digits. The hex value must be a **Unicode scalar value**: in U+0000–U+D7FF or U+E000–U+10FFFF. Surrogate code points (U+D800–U+DFFF) and out-of-range values (> U+10FFFF) are hard parse errors, not `(hole ...)` nodes. Raw newlines, raw tabs, and raw control characters are forbidden inside string literals; raw non-ASCII UTF-8 is accepted by the parser but re-emitted canonically per S2 below.

**Canonicalizer emission** (per [ADR 0010](../decisions/0010-canonical-emission-rules.md)):

- **S1. Named-escape preference.** Bytes with a named escape emit as: `"` → `\"`, `\` → `\\`, TAB (0x09) → `\t`, LF (0x0a) → `\n`, CR (0x0d) → `\r`.
- **S2. Escape all non-ASCII and unnamed controls.** Any byte outside 0x20–0x7e that does not match S1 emits as `\u{HEX}`. Covers 0x00–0x08, 0x0b, 0x0c, 0x0e–0x1f, 0x7f, and all code points ≥ 0x80 (including raw-UTF-8-accepted non-ASCII).
- **S3. `\u{HEX}` form.** Lowercase hex, minimum digits, no leading zeros (the single literal `0` for U+0000 is the one exception — `\u{0}` is canonical for NUL). Examples: `\u{a0}`, `\u{1f600}`, `\u{10ffff}`.
- **S4. Direct emission otherwise.** Bytes in 0x20–0x7e that are not `"` or `\` emit as themselves.

**Consequence:** Canonical string bytes between the surrounding `"…"` are always 7-bit ASCII-printable. No Unicode normalization is performed; decoded byte sequences remain opaque for hashing.

### Symbols

Bare ASCII identifiers used in positions that take a name (record field, ctor name, hole diag-id). Match `[A-Za-z_][A-Za-z0-9_-]*`. Field and ctor naming is otherwise a parser concern; the canonical form just requires the byte sequence.

## 4. Variables and DeBruijn

`(var N)` where N is a non-negative decimal integer. Every variable reference uses this form; display names live entirely in the sidecar.

### Binders

- `lam` introduces 1 name; body sees `(var 0)` for the parameter.
- `let` introduces 1 name; body sees `(var 0)` for the bound value.
- `rec` and `module` introduce N names simultaneously; § 6 defines the indexing.
- `pat-var` introduces 1 name into the arm body; the arm body sees `(var 0)` for the pattern's binding.
- `pat-ctor` with K nested `pat-var` sub-patterns introduces K names; the binding order is the textual order of `pat-var`s in the pattern, with the first encountered at the highest index (so the *last* encountered `pat-var` is at index 0 in the arm body).

References inside any binder skip past intervening binders the standard way. `(lam (lam (var 1)))` is the K combinator's identity-on-outer; `(lam (lam (var 0)))` is identity-on-inner.

## 5. Rec and module

```
(rec   binding₀ binding₁ … binding_{N-1} body)
(module binding₀ binding₁ … binding_{N-1})
```

The N bindings introduce N names simultaneously, all in scope in every binding RHS *and* in the body (for `rec`). DeBruijn assignment ([ADR 0007](../decisions/0007-debruijn-rec-indexing.md)):

> **Position K in the binding list = DeBruijn index K.** Binding 0 is `(var 0)`, binding 1 is `(var 1)`, etc.

This is the same numbering inside any binding RHS as inside the body. Note that this *differs* from the let-cascade intuition where the most recently declared name is `(var 0)` — `rec` is a single simultaneous frame, not nested binders, so the convention is set by simplicity rather than by analogy.

Binding order is preserved as the user wrote it. `rec` and `module` do **not** sort or reorder. Two `rec` groups with the same bindings in different orders produce different canonical text and different hashes; if a normalizer wants to canonicalize binding order, that is a separate pass and not part of Stage 2.

## 6. Records and field ordering

```
(record  sym₀ val₀  sym₁ val₁  …  sym_{N-1} val_{N-1})
```

Fields are sorted in canonical form by `symₖ` bytes (UTF-8 lexicographic, ascending) per [ADR 0008](../decisions/0008-record-field-ordering.md). This is the only place canonical reorders user input. The motivation: record literals are semantically unordered, and content-addressing requires hash-equality of semantic-equality.

`match` arms, `rec` bindings, `module` bindings, and `ctor` arguments all preserve user order because they are semantically order-sensitive (first-match-wins for arms; positional for ctor args; the rec/module case is documented in § 5).

## 7. Holes

```
(hole DIAG-ID (str "..."))
```

`DIAG-ID` is a bare symbol drawn from a small frozen set. Initial set (Phase 0; Phase 1 may add more, additive only):

| Diag id              | Meaning                                          |
|----------------------|--------------------------------------------------|
| `unexpected-token`   | Lexer or parser hit a token it could not place.  |
| `unclosed-paren`     | Group opened but EOF reached before close.       |
| `expected-expr`      | A position required an expression; none found.   |
| `expected-pattern`   | A position required a pattern; none found.      |
| `unbound-name`       | Authoring-view identifier had no binder in scope.|
| `arity-mismatch`     | Construct's child count did not match its kind.  |

The payload string is human-readable; it is *not* hashed for semantic equivalence (two holes with the same diag-id but different payload strings are still distinct, because canonical text differs). Holes hash like any other node, so a parse-failed program still has a stable content address.

## 8. Patterns

Patterns get their own kind set (`pat-wild`, `pat-var`, `pat-ctor`) rather than reusing expression kinds, because pattern syntax is restricted (no lambdas, no applications, etc.). `pat-var` is nullary in canonical form because the binder it introduces is referenced by DeBruijn from the arm body (per § 4); display names for pattern variables live in the sidecar.

## 9. Hashing

Per [ADR 0009](../decisions/0009-hashing-rule.md):

```
hash(node) = BLAKE3(canonical_text(node))
```

The canonical text of any subtree is self-contained: children are inlined, not replaced by hash references. Identical subtrees produce identical bytes and therefore identical hashes; the Merkle property is automatic.

A storage layer that wants to dedupe identical subtrees by hash is free to do so — it stores both the canonical text and its hash. Canonical text format does not specify a hash-reference syntax.

The hash domain is the BLAKE3 output of the UTF-8 bytes of canonical text. Hash output is treated as an opaque 32-byte value; representation (hex, base32, etc.) is a sidecar/storage concern.

## 10. Worked example

Authoring view:
```
let id = lambda x. x in id 5
```

Canonical:
```
(let (lam (var 0)) (app (var 0) (int 5)))
```

Hash: `BLAKE3("(let (lam (var 0)) (app (var 0) (int 5)))")`.

Mutual recursion:
```
rec {even = lambda n. if n then odd (n - 1) else 1; odd = lambda n. if n then even (n - 1) else 0} in even 10
```

Canonical (using simplified subtraction as a `ctor` for illustration; real subtraction wiring is Phase 1):
```
(rec (lam (if (var 0) (app (var 2) (ctor sub (var 0) (int 1))) (int 1))) (lam (if (var 0) (app (var 1) (ctor sub (var 0) (int 1))) (int 0))) (app (var 0) (int 10)))
```

DeBruijn trace, per [ADR 0007](../decisions/0007-debruijn-rec-indexing.md):

- In the rec's binding list, `even` is binding 0 and `odd` is binding 1.
- In the rec body (`(app (var 0) (int 10))`), `(var 0)` = `even`.
- Inside `even`'s `lam`, `n` is `(var 0)`; the rec bindings shift up by 1, so `even` = `(var 1)` and `odd` = `(var 2)`. The call `odd (n - 1)` is `(app (var 2) (ctor sub (var 0) (int 1)))`.
- Inside `odd`'s `lam`, symmetrically: `n` = `(var 0)`, `even` = `(var 1)`, `odd` = `(var 2)`. The call `even (n - 1)` is `(app (var 1) (ctor sub (var 0) (int 1)))`.

This example is a canonical Stage 2 test vector candidate.

## 11. Open items

- ~~**Type syntax inside `ann`.**~~ Resolved 2026-04-27 ([ADR 0034](../decisions/0034-p2-type-subset-ann.md)).
  New canonical tags: `fn-ty` (arity 3: arg-type, ret-type, eff-node), `ty-var` (arity 1: DeBruijn int),
  `forall` (arity 3: ty-count int, eff-count int, body). Effect set represented by `eff-set` and `eff-var`
  tags ([ADR 0035](../decisions/0035-p2-effect-set-canonical.md), [ADR 0036](../decisions/0036-p2-effect-polymorphism-syntax.md)).
  Valid type positions: `sym`, `ty-var`, `record`, `fn-ty`, `app` (type application), `forall`, `eff-set`, `eff-var`.
  Test vector V29 committed.
- **Exact set of hole diag-ids.** The § 7 table is initial. Phase 2 adds three new ids:
  `type-parse-error`, `effect-parse-error`, `module-binding-error` ([ADR 0040](../decisions/0040-p2-hole-recovery.md)).
- ~~**Test vector format and location.**~~ Resolved 2026-04-22. Bytes live in [`plans/test-vectors/`](test-vectors/); narrative in [`test-vectors.md`](test-vectors.md); file-naming convention and test-role semantics documented in [`test-vectors/README.md`](test-vectors/README.md).
- **Module composition.** ADR 0004 reserves `module` but defers cross-module name resolution to Phase 1+. Canonical form for cross-module references is not in this spec.
- **bpe-compact lead on corpus-shaped programs.** Open item from [ADR 0003](../decisions/0003-authoring-view-bpe-compact.md). Re-checked at Stage 4 corpus freeze; if the lead reverses, ADR 0003 is superseded but this canonical-format spec is unaffected (it does not depend on the authoring view choice).

## 12. Exit criteria for Stage 2

Per [phase-0-plan.md § Stage 2](phase-0-plan.md):

1. This document is precise enough that two independent implementations produce byte-identical canonical text on the same AST.
2. ~30 round-trip test vectors exist and pass under both implementations.
3. `rec`/`module` hashing-as-single-atom is verified by test vector.
4. A Hole-bearing program produces a stable hash matching expectation.

When all four are met, this document moves from Draft to Frozen and the authoring grammar (ADR 0003) plus this format become the source of truth for everything downstream.
