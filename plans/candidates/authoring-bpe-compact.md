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

- **Stage 2 canonical text format** will cite this grammar as the authoring-view source of truth for round-trip test vectors.
- **Stage 3 inspection view** is a separate, indented/annotated projection — not this grammar. The two views land together (ground rule in [CLAUDE.md](../../CLAUDE.md)).
- **Q5 sidecar format** must carry the display-name table that this authoring view depends on.
- **Third data point** (non-lambda-calc-shaped program) is deferred to the Stage 4 corpus freeze; if bpe-compact's lead reverses there, ADR 0003 is superseded.
