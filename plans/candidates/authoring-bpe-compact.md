# Authoring View: BPE-Compact

**Status:** Accepted as the Tacit authoring view per [ADR 0003](../../decisions/0003-authoring-view-bpe-compact.md).
**Parent:** [../phase-0-plan.md](../phase-0-plan.md)
**Related:** [authoring-sexpr-int-ids.md](authoring-sexpr-int-ids.md) (rejected), [authoring-glyph-prefix.md](authoring-glyph-prefix.md) (rejected), [reference-ast.md](reference-ast.md).

This doc was originally one of five Q1 candidates ("BPE-optimized"). After scoring (see [reference-ast.md](reference-ast.md)), the **compact** sub-variant won on both tokenizers at 100 nodes and was accepted in ADR 0003. The doc has been rewritten around bpe-compact; the original bpe-optimized and bpe-hybrid sub-variants are preserved at the end as considered-but-rejected alternatives.

## Design thesis

The authoring view picks its surface syntax to align with the merge tables of modern BPE tokenizers. Structural encodings (s-exprs, glyph-prefix) optimize for *structure*; bpe-compact optimizes for *what the BPE has already merged for free*. A leading-space `let`, `if`, `lambda`, `match`, `rec`, `in`, `then`, `else` tokenizes to a single token under cl100k-class vocabularies, and common identifier fragments (`id`, `pair`, `xs`, `first`) likewise. The bet — validated by the 100-node scoring — is that mimicking familiar programming-language surface forms wins more tokens than abbreviating into glyphs or integers, once the program is large enough for keyword cost to amortize.

## Grammar

```
expr    ::= ident                                  ; display name (authoring layer);
                                                   ; canonical strips to DeBruijn int
          | "(" expr ")"
          | "lambda" ident "." expr
          | "let" ident (":" type)? "=" expr "in" expr
          | "rec" "{" binding (";" binding)* "}" "in" expr
          | "if" expr "then" expr "else" expr
          | "match" expr "with" arm+
          | expr expr                              ; juxtaposition = app, left-assoc
          | expr "." ident                         ; projection (no spaces)
          | "{" field ("," field)* "}"             ; record
          | literal | "@" sym | "_"

binding ::= ident (":" type)? "=" expr             ; in rec, one per mutually-recursive defn
arm     ::= "|" pattern "=>" expr
field   ::= ident ":" expr                         ; record field (no space around `:`)
type    ::= expr                                   ; types are first-class exprs (Phase 1);
                                                   ; canonical materializes as `ann` node
pattern ::= "_" | ident | ctor ident*              ; ctor = capitalized ident
literal ::= int | string                           ; string is double-quoted; int is decimal
```

**Spacing rules (compact variant):**

- No spaces around `.` (projection) or `:` (field/type annotation).
- Tight braces for records and `rec` groups: `{len: e, …}`, `rec {f = …; g = …}`.
- Single space around binary structural keywords (`=`, `in`, `then`, `else`, `=>`, `with`).
- Single space before opening delimiters that follow identifiers (`… in let`, `} in`).

The compact spacing is what decided bpe-compact's win over bpe-optimized (5–7% on both tokenizers). It costs identifier fragmentation (`pair.fst` can split into `pair`/`.f`/`st` under cl100k) but gains on the near-universal merges of space-adjacent keywords and operators.

**Canonical mapping (informative, pinned by Stage 2 spec):**

| Authoring form                              | Canonical kind | Arity |
|--------------------------------------------|----------------|------:|
| `lambda x. e`                              | `lam`          | 1     |
| `f x` (juxtaposition)                      | `app`          | 2     |
| `let x = v in b`                           | `let`          | 2     |
| `let x: T = v in b`                        | `let` over `ann(v, T)` | 2 (with `ann` = 2) |
| `rec {f = e₁; …; g = eₙ} in b`             | `rec`          | 1+N   (see [ADR 0004](../../decisions/0004-rec-arity.md)) |
| `if c then a else b`                       | `if`           | 3     |
| `match s with \| p₁ => e₁ \| … \| pₙ => eₙ`| `match`        | 1+N   |
| `\| p => e`                                | `arm`          | 2     |
| `{f₁: v₁, …, fₙ: vₙ}`                      | `record`       | 2N    |
| `e.f`                                      | `proj`         | 2     |
| `Ctor a₁ … aₙ`                             | `ctor`         | 1+N   |
| `_`                                         | `hole`         | 1     |
| (inferred from `let x: T`)                 | `ann`          | 2     |

Top-level files are not `rec`; they're `module` (arity N, no body, per [ADR 0004](../../decisions/0004-rec-arity.md)). The authoring-view surface syntax for `module` is deferred — Phase 0 exercises only inner `rec`, and module semantics are a Phase 1+ concern.

## Why BPE wins at scale

cl100k and modern BPE vocabularies were trained on English + mainstream source code, which yields near-universal single-token merges for:

- Keywords with leading space: ` let`, ` in`, ` if`, ` then`, ` else`, ` lambda`, ` match`, ` with`, ` rec`.
- Operators and punctuation with leading space: ` =>`, ` =`, ` .`, ` ,`, ` :`, ` |`, ` (`, ` )`, ` {`, ` }`.
- Short common identifiers: `x`, `y`, `f`, `n`, ` xs`, ` foo`, ` bar`.

Structural encodings pay `(` + digit + ` ` + digit + `)` per subtree (s-expr int-IDs) or paren-delimited glyph groups for every variadic node (glyph-prefix). BPE-compact pays for keywords, but each keyword is **one token**, the same cost as `(` — and keywords carry more structural information per token.

Empirical scores at 100 nodes ([reference-ast.md](reference-ast.md)): bpe-compact at **1.00×** on both tiktoken (129 tokens) and Claude (199 net tokens). Structural encodings land at 1.40–1.78×.

## Tradeoffs (accepted)

- **Hard coupling to BPE-family tokenizers.** A future Anthropic tokenizer that diverges materially from cl100k could swing absolute token counts. Acceptable because (a) the plan targets Claude-class models specifically ([ADR 0001](../../decisions/0001-target-tokenizer.md)), and (b) re-measurement is a one-script re-run.
- **Authoring view loses 1:1 visual correspondence with canonical structure.** `f x y` in the view is `(app (app f x) y)` in canonical; the user trusts the round-trip through the canonicalizer. Mitigated by the inspection view (Stage 3 deliverable).
- **Display names are load-bearing.** The canonicalizer must maintain a bidirectional mapping between authoring-view identifiers and DeBruijn indices. This pushes complexity into the sidecar (Q5) and the canonicalizer. Accepted in ADR 0003.
- **Operator precedence and associativity are real grammar concerns.** Juxtaposition means app and must parse left-associative; `.` binds tighter than juxtaposition. The canonicalizer's grammar writes these rules down explicitly in Stage 2.
- **Identifier fragmentation.** `pair.fst` can split across three tokens under cl100k. The compact no-space form accepts this in exchange for keyword/operator merges that net out in its favor at scale. If a future corpus shows identifier-heavy programs reversing the win, revisit (see `reference-ast.md` open items).

## Rejected sub-variants

Kept here for decision-log completeness; not part of the accepted grammar.

### bpe-optimized (spaced)

Same grammar, but all binary operators are space-separated: `pair . fst`, `len : length xs`, `lambda x . x`. Scored 1.05–1.07× at 100 nodes. Rejected — the spaces cost tokens without compensating readability win.

### bpe-hybrid (DeBruijn integer leaves)

Keyword skeleton with DeBruijn ints at var-ref positions and binder-name elision:

```
rec { = lambda match 0 with | Nil => Zero | Cons => Succ ( 3 0 ) ; ... } in ...
```

Scored 1.02–1.03× at 100 nodes (closest rival to bpe-compact). Rejected despite the small gap because:

- Stripping pattern-var names (`| Cons => …`) requires ctor arity to be reader-visible, creating a sidecar dependency that the display-name variants don't have.
- Surfacing DeBruijn ints in the *authoring* view partially collapses the two-views distinction — the abstraction loses value if authoring already carries the canonical representation.
- Binder-name elision (`let = …`, `rec { = lambda …`) is hard for humans to read and offers no LLM-legibility gain.

## Relationship to other Phase 0 artifacts

- **Stage 2 canonical text format** cites this grammar as the authoring-view source of truth for round-trip test vectors.
- **Stage 3 inspection view** ([inspection-view.md](../inspection-view.md)) is a separate, indented/annotated projection — not this grammar. The two views land together (ground rule in [CLAUDE.md](../../CLAUDE.md)).
- **Q5 sidecar format** — resolved by [ADR 0014](../../decisions/0014-sidecar-format.md) / [sidecar-format.md](../sidecar-format.md). Projection rules below depend on it.
- **Third data point** (non-lambda-calc-shaped program) is deferred to the Stage 4 corpus freeze; if bpe-compact's lead reverses there, ADR 0003 is superseded.

## Projection rules (authoring ↔ canonical)

Added at Stage 3. The grammar above defines what authoring-view source *looks like*; this section defines how it maps to and from the canonical form, using the sidecar to carry the information that canonical form discards.

### Direction 1: authoring view → canonical + sidecar

A canonicalizer that accepts authoring-view input produces two outputs: the canonical `.tac` bytes, and a `.tacd` sidecar per [sidecar-format.md](../sidecar-format.md).

**Walk state:** a **binding stack** (list of names currently in scope, innermost first) and an **emit buffer** (canonical bytes being built) and a **sidecar builder** (parallel JSON tree being built in lockstep).

**Per-construct rules:**

- `lambda X. E` → push `X` onto the binding stack, emit `(lam ` + recurse on `E` + `)`, sidecar entry `{binder: X, children: [<E-entry>]}`, pop.
- `let X = V in B` → emit `(let ` + recurse on `V` + ` `, push `X`, recurse on `B`, pop, emit `)`. Sidecar: `{binder: X, children: [<V-entry>, <B-entry>]}`. (`X` is **not** in scope while `V` is being emitted; scope is the body only. Matches canonical-text-format.md § 4.)
- `let X: T = V in B` → same as above but wrap `V` in `ann`: emit `(let (ann ` + recurse on `V` + ` ` + recurse on `T` + `) ` + ... . Sidecar's binder entry has children `[<ann-entry>, <B-entry>]` where `<ann-entry>.children = [<V-entry>, <T-entry>]`. The binder name still attaches to the `let` node, not the `ann`.
- `rec {X0 = E0; X1 = E1; ...; XN-1 = EN-1} in B` → extend the binding stack with a simultaneous N-frame in which position K maps to DeBruijn index K per [ADR 0007](../../decisions/0007-debruijn-rec-indexing.md) — i.e., inside every Ek and inside B, `(var 0)` resolves to `X0`, `(var 1)` to `X1`, …, `(var N-1)` to `X_{N-1}`. (If the binding stack is implemented as a list with innermost-at-head, this is equivalent to pushing `X_{N-1}, X_{N-2}, …, X_0` in that order so `X_0` ends up at the head; the key contract is that lookup of `(var K)` returns `X_K`.) Emit `(rec ` + recurse on each Ek in order + ` ` + recurse on B + `)`. Sidecar: `{binders: [X0, X1, ..., XN-1], children: [<E0>, ..., <EN-1>, <B>]}`. Pop the frame after emission.
- `if C then T else E` → emit `(if ` + recurse on C + ` ` + recurse on T + ` ` + recurse on E + `)`. Sidecar: `{children: [<C>, <T>, <E>]}`.
- `match S with | P0 => E0 | ... | PN-1 => EN-1` → emit `(match ` + recurse on S + N arms + `)`. Each arm: `(arm ` + pattern + body + `)`; the body's binding stack has any `pat-var`s pushed in the order specified by [canonical-text-format.md § 4](../canonical-text-format.md) (*last-encountered* `pat-var` ends up at index 0). Sidecar arm entry carries no metadata except children; `pat-var` sidecar entries carry their binder names.
- `{f0: V0, f1: V1, ..., fN-1: VN-1}` (record) → sort fields alphabetically by field-symbol bytes for canonical emission per [ADR 0008](../../decisions/0008-record-field-ordering.md). Record the permutation in the sidecar's `field_order`. Emit `(record sym0 val0 sym1 val1 ...)` in canonical (sorted) order.
- `E.f` → emit `(proj ` + recurse on E + ` ` + f + `)`. No sidecar metadata.
- `Ctor a0 a1 ... aN-1` → emit `(ctor Ctor ` + recurse on args + `)`. No sidecar binder metadata (ctor names are in canonical form already).
- `ident` (identifier in expression position) → look up in the binding stack; emit `(var N)` where N is the stack depth. Record no sidecar entry for the var — names on `var` are computed from the binding stack at render time (§ inspection-view.md § 2). If the identifier is not in scope, emit a hole: `(hole unbound-name (str "identifier 'X' not in scope"))`.
- `_` in expression position → emit `(hole expected-expr (str "missing expression"))` (this is the authoring-view explicit-hole syntax; distinct from `_` in pattern position which is `pat-wild`). If Phase 1 introduces a user-authored "fill-this-in" hole marker, this rule is revisited.
- `_` in pattern position → emit `(pat-wild)`. Sidecar: `{}`.
- `ident` in pattern position (non-ctor) → emit `(pat-var)`, push the name onto a pattern-binding list (flushed into the arm body's binding stack per [canonical-text-format.md § 4](../canonical-text-format.md)'s "last encountered = index 0" rule). Sidecar: `{binder: ident}`.
- `Ctor p0 p1 ... pN-1` in pattern position (capitalized identifier) → emit `(pat-ctor Ctor ` + sub-patterns + `)`. Sidecar: `{children: [null, <p0>, ..., <pN-1>]}` (the ctor-name sym has no metadata).
- Literals (`int`, string) → emit `(int N)` or `(str "...")` with canonical emission rules applied ([ADR 0010](../../decisions/0010-canonical-emission-rules.md)). No sidecar metadata.
- `@sym` → emit `(sym sym)`. No sidecar metadata.

**Comment handling:** the authoring view grammar has **no comment syntax** — it's optimized for token density. User-authored comments, if any, enter the pipeline through a separate channel (editor UI writing directly to the sidecar, or a tool that annotates a hash with a note). Phase 0 authoring view is comment-free; Phase 1+ may add a sugar form that flows comments into the sidecar at emission time.

**Sidecar hash:** after the canonical text is fully emitted, compute BLAKE3 of the bytes and record it in the sidecar's `targets_hash_blake3` field per [sidecar-format.md § 2](../sidecar-format.md).

### Direction 2: canonical + sidecar → authoring view

The reverse projection is a tree-walk producing authoring-view bytes. The sidecar provides names; where names are absent (missing or stale sidecar), the view synthesizes them per [sidecar-format.md § 5](../sidecar-format.md) — same scheme as inspection view, but rendered in bpe-compact surface syntax instead of indented pseudo-code.

**Per-kind emission:**

- `(lam E)` — look up the sidecar `binder` (or synthesize); emit `lambda X. ` + recurse on E with X pushed. Lambda chains *do not* collapse in the authoring view (unlike inspection view § 3.1) — authoring view prefers the denser `lambda x. lambda y. …` form because nested-lambda BPE merges are tokenizer-efficient.
- `(app F A)` — recurse on F inline, emit ` `, recurse on A. Left-associativity is the default; parenthesize A only if A is itself an app or other non-atomic form.
- `(let V B)` — emit `let X = ` + recurse on V + ` in ` + recurse on B with X pushed.
- `(let (ann V T) B)` — emit `let X: ` + recurse on T + ` = ` + recurse on V + ` in ` + recurse on B. The `ann` inlines as a type annotation on the binder.
- `(rec E0 E1 … EN-1 B)` — emit `rec {` + N bindings separated by `;` + `} in ` + recurse on B. Each binding uses the sidecar's `binders[k]` name. Names are all in scope within each Ek and within B (per [ADR 0007](../../decisions/0007-debruijn-rec-indexing.md)).
- `(module E0 … EN-1)` — emit `module {` + N bindings + `}`. (Authoring view syntax for top-level `module` is deferred per the grammar doc's note; Phase 0 only emits this if given a module tree as input.)
- `(if C T E)` — emit `if ` + C + ` then ` + T + ` else ` + E. Parenthesize T and E only if they themselves are control-flow forms that would parse ambiguously.
- `(match S arm…)` — emit `match ` + S + ` with` + arms. Each arm: ` | ` + pattern + ` => ` + body.
- `(arm P B)` — render pattern, then body with the pattern's pat-var names pushed onto the binding stack.
- `(record s0 v0 s1 v1 …)` — if sidecar `field_order` is present, emit fields in that order; else in canonical order. Use tight-brace form `{f0: v0, f1: v1}`.
- `(proj R f)` — emit R + `.` + f with no surrounding spaces (compact).
- `(ctor C a0 a1 …)` — emit `C a0 a1 …` (ctor name capitalized; args space-separated; parenthesize any arg that is itself a non-atomic form).
- `(ann E T)` — only emitted in standalone `ann` position; as a type annotation on a `let`, it inlines per the `let` rule. Standalone form: `(E : T)` parenthesized.
- `(var N)` — emit the resolved name per [inspection-view.md § 2](../inspection-view.md)'s name-resolution rules (sidecar-first, synthetic fallback).
- `(int V)`, `(str "..."`), `(sym S)` — emit `V`, `"..."`, `@S` respectively, matching the grammar's literal/symbol rules.
- `(hole DIAG-ID (str "..."))` — emit `_` (the authoring view's unified hole marker) regardless of diag-id. Round-trip preserves canonical bytes only if the authoring view tool re-reads the original canonical form's diag-id and payload; authoring-view output alone is lossy here. Readers needing the diag-id switch to the inspection view.
- `(pat-wild)` → `_`.
- `(pat-var)` → the sidecar `binder` (or synthesized `p0`, `p1`, …).
- `(pat-ctor C p0 p1 …)` → `C p0 p1 …`.

### Direction 3: round-trip guarantees

- **`canonical + sidecar → authoring view → canonical` is the identity** *iff* the sidecar is fresh (§ 4 of [sidecar-format.md](../sidecar-format.md)). Names come back out in the same form they went in, field order is reconstructed via `field_order`, and everything else is structural.
- **`authoring view → canonical` is deterministic** given a fixed canonicalizer implementation. Two independent canonicalizers agreeing on the same bytes is Stage 2's exit condition ([ADR 0013](../../decisions/0013-canonical-text-format-frozen.md)); that guarantee carries through as long as the authoring-view parser is deterministic (which it is — this grammar has unambiguous precedence rules).
- **Hole round-trip is lossy through the authoring view.** `(hole unexpected-token "...")` and `(hole expected-expr "...")` both render as `_` in authoring view. This is accepted: the authoring view is for writing, and users typically encounter holes via the inspection view which preserves them visibly (§ 3.10 of [inspection-view.md](../inspection-view.md)).
- **Comment round-trip is trivially preserved** because the authoring view has no comment syntax — comments flow only through the sidecar, untouched by either direction.

### Direction 4: stale or missing sidecar

- **Missing sidecar (authoring → canonical):** the authoring view requires names to resolve to DeBruijn indices, so a pure canonical text cannot be produced from a scope-agnostic authoring-view source. This direction is not affected by missing sidecars — the sidecar is a *output*, not an input, of this direction.
- **Missing sidecar (canonical → authoring):** synthetic names from [sidecar-format.md § 5](../sidecar-format.md) are used. The output is valid authoring-view bytes but not identity-to-original (different `v0` vs `x`).
- **Stale sidecar (canonical → authoring):** the view layer walks both trees in lockstep; at any node where the sidecar shape doesn't match the AST, it falls back to synthetic names for that node and its descendants, preserving user-authored names wherever possible ([sidecar-format.md § 4](../sidecar-format.md)).
