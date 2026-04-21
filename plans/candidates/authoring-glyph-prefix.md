# Authoring View Candidate: Single-Glyph Prefix Operators

**Status:** Draft candidate for Q1 (Phase 0, Stage 1)
**Parent:** [../plans/phase-0-plan.md](../plans/phase-0-plan.md)

A candidate authoring-view format for the Tacit-Lite AST. Replaces the integer kind table of the s-expression candidate with single-character ASCII prefix operators. The aim is to keep the 1-token-per-glyph efficiency of bare integers while making the kind tag self-describing — no out-of-band table required to read a fragment.

## Grammar

```
expr  ::= int                       ; DeBruijn variable reference
        | glyph expr*               ; node: glyph fixes arity
        | "#" literal               ; primitive literal
        | "@" sym                   ; interned symbol
        | "(" expr ")"              ; grouping (only where arity is variadic)
```

Each kind glyph has a fixed arity, so parens are only required for variadic constructors (`rec`, `match`, `record`, `ctor`). Fixed-arity nodes are juxtaposition — the reader knows when to stop.

## Glyph table (draft)

| Glyph | Kind   | Arity | Mnemonic |
|-------|--------|-------|----------|
| `\`   | lam    | 1     | λ-shape  |
| `.`   | app    | 2     | function application dot |
| `=`   | let    | 2     | binding  |
| `?`   | if     | 3     | conditional |
| `:`   | ann    | 2     | type ascription |
| `/`   | proj   | 2     | record/field separator |
| `*`   | rec    | 1+N   | mutual cluster + body ("star" = group); [ADR 0004](../../decisions/0004-rec-arity.md) |
| `|`   | match  | 1+N   | arms separated by alternation |
| `>`   | arm    | 2     | pattern → body |
| `{`…`}` | record | 2N | brace pair, sym/val interior |
| `!`   | ctor   | 1+N   | constructor application |
| `_`   | hole   | 1     | placeholder for typed `Hole` |
| `%`   | module | N     | top-level recursive bindings, no body; [ADR 0004](../../decisions/0004-rec-arity.md) |

`{` and `}` are the only paired delimiters with semantic meaning beyond grouping; everything else is prefix.

## Examples

```
; identity:  λx. x
\ 0

; K:  λx. λy. x
\ \ 1

; let f = λx. x in f 42
= \ 0  . 0 #42

; if c then a else b
? 0 1 2

; rec { even, odd }
*( \ ...  \ ... )

; { name: "x", age: 30 }
{ @name #"x" @age #30 }

; record projection:  r.field
/ 0 @field

; parse-failure placeholder
_ #7
```

## Tradeoffs vs. integer kind IDs

**Wins**
- Self-describing: a fragment is readable without consulting a kind table.
- Stable across spec revisions: adding a new kind doesn't renumber existing ones.
- Glyphs survive copy/paste and grep more legibly than `(0` / `(7`.

**Losses**
- Glyph budget is small (~20 ASCII punctuation chars usable); Tacit-Full kinds may exhaust it and force escape hatches.
- Fixed arity means whitespace becomes load-bearing for canonicalization (the s-expr form is whitespace-insensitive inside lists).
- Some glyphs (`.`, `/`, `:`) are visually noisy when stacked, hurting the readability win on dense expressions.

## Token notes

- Most punctuation glyphs tokenize as 1 token under cl100k, same as small ints. `\` is sometimes fused with the following char — needs measurement.
- Removing the surrounding `(` `)` on every fixed-arity node is a real saving: identity drops from 5 tokens (`(0 0)`) to 3 (`\ 0`), assuming the leading space tokenizes cleanly.
- Variadic nodes still pay the paren cost; that's where most of the residual overhead lives.

## Open questions to resolve during the prototype

1. Is fixed arity worth the loss of whitespace insensitivity? An alternative is to keep parens everywhere and just swap the integer kind for a glyph (`(\ 0)` instead of `(0 0)`) — costs 2 tokens per node but restores robustness.
2. Glyph choice for `app` is the highest-traffic decision; `.` is mnemonic but visually collides with float literals if `#` is ever dropped.
3. Should Tacit-Full kinds reserve a two-glyph escape (e.g. `\\` prefix) now, or defer until that scope opens?
4. Interaction with the inspection view: glyphs may need expansion to keyword names (`\` → `Lam`) for the inspection projection — confirm the round-trip is byte-exact through canonical.
