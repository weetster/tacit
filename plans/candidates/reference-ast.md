# Q1 Scoring: Reference AST

**Status:** Draft for Q1 (Phase 0, Stage 1)
**Purpose:** Single shared AST that all three authoring-view candidates encode, against which token counts are measured.
**Tokenizer:** tiktoken `cl100k_base` (Q7 not yet resolved; Claude tokenizer unavailable in current environment — substitute when accessible and re-score).

**Scoring tool:** [`tools/q1-scoring/score.py`](../../tools/q1-scoring/score.py). Encoding strings live in the script and must be kept in sync with this doc.

## Format choice

The reference AST is described three ways in this document:

1. **Logical form** — pseudo-syntax with display names. Human-readable, format-neutral. Authoritative description of *what* the AST is.
2. **DeBruijn-indexed form** — same tree with names erased, var refs as integers. Matches what canonical will hold.
3. **Per-candidate encodings** — the same DeBruijn AST written out in each candidate's surface syntax, ready for tokenization.

No serialized binary or canonical bytes yet — the canonical text format isn't frozen until Stage 2. The logical/DeBruijn descriptions in this doc are the source of truth for the reference AST during Stage 1 scoring; if Stage 2 changes how DeBruijn or any kind is rendered, this doc gets re-derived.

## Logical form

```
let id = λx. x in
  let pair = { fst: id 1, snd: id 2 } in
    if pair.fst then pair.snd else 0
```

Exercises: `let` (×2), `lam`, `app` (×2), `record` (2 fields), `proj` (×2), `if`, var refs at depths 0 and 1, integer literals, interned symbols (`@fst`, `@snd`).

**Not exercised** (deliberate — out of scope for a 20-node sample): `rec`, `match`, `arm`, `ctor`, `hole`, `ann`. Supplementary fragments will be needed for full coverage; see *Open items* below.

## DeBruijn form

```
let
  rhs:  λ. var 0
  body: let
          rhs:  record { @fst → app (var 0) 1
                       , @snd → app (var 0) 2 }
          body: if (proj (var 0) @fst)
                  then (proj (var 0) @snd)
                  else 0
```

## Node count

Counting every constructor and every leaf (var ref, literal, symbol):

| # | Item              | # | Item              | # | Item              |
|---|-------------------|---|-------------------|---|-------------------|
| 1 | let (outer)       | 8 | var 0 (id)        | 15| proj              |
| 2 | lam               | 9 | #1                | 16| var 0 (pair)      |
| 3 | var 0 (x)         | 10| @snd              | 17| @fst              |
| 4 | let (inner)       | 11| app               | 18| proj              |
| 5 | record            | 12| var 0 (id)        | 19| var 0 (pair)      |
| 6 | @fst              | 13| #2                | 20| @snd              |
| 7 | app               | 14| if                | 21| #0                |

**Total: 21 nodes** — within the "20-node" tolerance the plan specifies.

## Encoding 1 — S-expressions over integer IDs

Per kind table in [authoring-sexpr-int-ids.md](authoring-sexpr-int-ids.md): `0`=lam, `1`=app, `2`=let, `4`=if, `7`=record, `8`=proj.

```
(2 (0 0) (2 (7 @fst (1 0 #1) @snd (1 0 #2)) (4 (8 0 @fst) (8 0 @snd) #0)))
```

Token count under cl100k: **48 tokens** (1.41× the best). Worst of the four. Fragmentation cause: bare ` <digit>` after a `' '` token does **not** fuse — `(0 0)` tokenizes as `'('`, `'0'`, `' '`, `'0'`, `')'` (5 tokens for 5 chars). The candidate doc's claim that "` <digit>` ≈ 1 token" is empirically wrong in the positions this encoding actually uses.

## Encoding 2 — Single-glyph prefix operators

Per glyph table in [authoring-glyph-prefix.md](authoring-glyph-prefix.md): `\`=lam, `.`=app, `=`=let, `?`=if, `/`=proj, `{…}`=record. Fixed-arity nodes are juxtaposition; record is the only delimited construct here.

```
= \ 0 = { @fst . 0 #1 @snd . 0 #2 } ? / 0 @fst / 0 @snd #0
```

Note: this candidate's grammar requires whitespace to disambiguate juxtaposed fixed-arity nodes; spacing above is the minimum legal form.

Token count under cl100k: **34 tokens** (1.00× — winner on this sample). The `' \\'`, `' ='`, `' {'`, `' ?'`, `' /'`, `' .'`, `' @'`, `' #'` glyph-with-leading-space tokens all fuse cleanly to single tokens. Bare ` 0` still costs 2 tokens, but there are fewer of them than in the s-expr form.

## Encoding 3 — BPE-optimized

Per grammar in [authoring-bpe-optimized.md](authoring-bpe-optimized.md). Display names re-introduced for `id`, `pair`, `x` (the candidate's display-name layer); record uses `name : value` pairs; field projection is `expr . name`.

```
let id = lambda x . x in let pair = { fst : id 1 , snd : id 2 } in if pair . fst then pair . snd else 0
```

Hybrid variant (keyword skeleton + DeBruijn leaves, per the candidate's "Hybrid worth considering" section):

```
let = lambda 0 in let = { fst : 0 1 , snd : 0 2 } in if 0 . fst then 0 . snd else 0
```

Token counts under cl100k:

- **bpe-optimized (spaced)**: 36 tokens (1.06×)
- **bpe-compact (no spaces around `.` / `:`)**: 36 tokens (1.06×) — same total, but fragments `pair.fst` into `' pair'`, `'.f'`, `'st'`. Same count is coincidental on this sample; compact form likely degrades on identifier-heavy fragments.
- **bpe-hybrid (DeBruijn leaves)**: 37 tokens (1.09×) — **worse** than the display-name version. The hybrid swaps ` id` (1 token) for ` 0` (2 tokens) at every var-ref position; with three var refs to `id` and three to `pair`, the supposed leaf savings are negative. The "Hybrid worth considering" section in the BPE candidate doc should be revised — display names tokenize *better* than raw DeBruijn ints in BPE-friendly contexts.

## Results summary (cl100k_base, 21-node reference AST)

| Encoding         | Chars | Tokens | Ratio | Notes |
|------------------|------:|-------:|------:|-------|
| sexpr-int-ids    |   74  |   48   | 1.41× | bare ` <digit>` doesn't fuse — 5 tokens per `(0 0)` |
| glyph-prefix     |   58  |   34   | 1.00× | glyph-with-leading-space fuses cleanly; winner |
| bpe-optimized    |  103  |   36   | 1.06× | display names tokenize as 1 token each |
| bpe-compact      |   93  |   36   | 1.06× | same count, but identifier fragmentation (`.f`/`st`) |
| bpe-hybrid       |   83  |   37   | 1.09× | DeBruijn leaves *cost* tokens vs display names |

**Headline:** glyph-prefix wins this sample by ~6% over the BPE candidates and ~30% over int-IDs. The win margin is small enough that the supplementary fragments listed below could plausibly flip the ordering.

## Scoring procedure

1. Run each encoding through `tiktoken.get_encoding("cl100k_base").encode(...)` and record token counts. — done via `tools/q1-scoring/score.py`.
2. Record a side-by-side table here with raw counts and ratios relative to the smallest. — done above.
3. Re-score against Claude's tokenizer when access is restored; flag any large divergence (>15%) as a robustness concern for whichever candidate moves the most.
4. Keep this AST frozen for the duration of Stage 1 scoring — if it changes, all three candidates re-score together so comparisons stay apples-to-apples.

## Open items

- **Supplementary fragments needed.** A 21-node sample can't exercise `rec`, `match`/`arm`, `ctor`, `hole`, or `ann`. Before committing to a winner, score at least one additional fragment that hits these — particularly `match`, which is structurally large and will skew the BPE candidate's win/loss ratio.
- **Larger sample for BPE amortization.** The BPE candidate's argument is that keyword overhead amortizes across larger expressions. The reference AST here may not be large enough to show that; consider a ~100-node sample as a second data point.
- **Tokenizer substitution.** Re-run all measurements against Claude's tokenizer once available (Q7).
- **DeBruijn rendering for the BPE-hybrid form.** The hybrid uses `let = ... in ...` with no display name on the binder, since the binding position is positional. Confirm this round-trips losslessly to canonical or pick a different convention before final scoring.
