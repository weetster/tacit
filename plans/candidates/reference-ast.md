# Q1 Scoring: Reference AST

**Status:** Draft for Q1 (Phase 0, Stage 1)
**Purpose:** Single shared AST that all three authoring-view candidates encode, against which token counts are measured.
**Tokenizers:** measured under both tiktoken `cl100k_base` and Claude `claude-opus-4-7` (via the Anthropic `count_tokens` endpoint, which is free).

**Scoring tools:**
- [`tools/q1-scoring/score.py`](../../tools/q1-scoring/score.py) — tiktoken
- [`tools/q1-scoring/score_claude.py`](../../tools/q1-scoring/score_claude.py) — Claude API
- [`tools/q1-scoring/candidates.py`](../../tools/q1-scoring/candidates.py) — shared encoding strings; keep in sync with this doc.

## Format choice

The reference AST is described three ways in this document:

1. **Logical form** — pseudo-syntax with display names. Human-readable, format-neutral. Authoritative description of *what* the AST is.
2. **DeBruijn-indexed form** — same tree with names erased, var refs as integers. Matches what canonical will hold.
3. **Per-candidate encodings** — the same DeBruijn AST written out in each candidate's surface syntax, ready for tokenization.

No serialized binary or canonical bytes yet — the canonical text format isn't frozen until Stage 2. The logical/DeBruijn descriptions in this doc are the source of truth for the reference AST during Stage 1 scoring; if Stage 2 changes how DeBruijn or any kind is rendered, this doc gets re-derived.

Two samples are scored:

- **Sample 1** (below) — a 21-node identity/pair program. Exercises `let`, `lam`, `app`, `record`, `proj`, `if`, var refs, literals, symbols.
- **Sample 2** (further below) — a 100-node list-processing program with mutual recursion. Additionally exercises `rec`, `match`, `arm`, `ctor`, `hole`, `ann`. Closes the "larger sample for BPE amortization" and "fragments covering the missing kinds" open items from the initial Stage 1 plan.

---

# Sample 1 — 21-node AST (identity/pair)

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

Per grammar in [authoring-bpe-compact.md](authoring-bpe-compact.md). Display names re-introduced for `id`, `pair`, `x` (the candidate's display-name layer); record uses `name : value` pairs; field projection is `expr . name`.

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

## Results summary (21-node reference AST)

### Under tiktoken `cl100k_base`

| Encoding         | Chars | Tokens | Ratio | Notes |
|------------------|------:|-------:|------:|-------|
| sexpr-int-ids    |   74  |   48   | 1.41× | bare ` <digit>` doesn't fuse — 5 tokens per `(0 0)` |
| glyph-prefix     |   58  |   34   | 1.00× | glyph-with-leading-space fuses cleanly; winner |
| bpe-optimized    |  103  |   36   | 1.06× | display names tokenize as 1 token each |
| bpe-compact      |   93  |   36   | 1.06× | same count, but identifier fragmentation (`.f`/`st`) |
| bpe-hybrid       |   83  |   37   | 1.09× | DeBruijn leaves *cost* tokens vs display names |

### Under Claude `claude-opus-4-7`

Net counts subtract a 12-token envelope baseline (single-char `"x"` user message); ratios use Net.

| Encoding         | Chars | Net Tokens | Ratio | Notes |
|------------------|------:|-----------:|------:|-------|
| sexpr-int-ids    |   74  |   61       | 1.39× | still worst — same structural fragmentation pattern |
| glyph-prefix     |   58  |   46       | 1.05× | tied with bpe-optimized; lost its tiktoken lead |
| bpe-optimized    |  103  |   46       | 1.05× | tied with glyph-prefix |
| **bpe-compact**  |  93  | **44**     | **1.00×** | **winner** — Claude's tokenizer fuses `pair.fst` better than tiktoken |
| bpe-hybrid       |   83  |   47       | 1.07× | DeBruijn leaves still cost tokens here too |

### Cross-tokenizer comparison

| Encoding         | tiktoken ratio | Claude ratio | Δ | Robustness |
|------------------|---:|---:|---:|---|
| sexpr-int-ids    | 1.41× | 1.39× | 0.02 | stable (always worst) |
| glyph-prefix     | 1.00× | 1.05× | +0.05 | flipped from winner to tied 2nd |
| bpe-optimized    | 1.06× | 1.05× | -0.01 | stable middle |
| bpe-compact      | 1.06× | 1.00× | -0.06 | flipped from middle to winner |
| bpe-hybrid       | 1.09× | 1.07× | -0.02 | stable (worst BPE variant) |

**Headline:** the winner is tokenizer-dependent. tiktoken favors **glyph-prefix** (compact structural form); Claude's tokenizer favors **bpe-compact** (no-spaces keyword form). All four BPE/glyph candidates land within a 7% band on either tokenizer — the choice between them is closer than the win/loss margin suggests. **sexpr-int-ids** is robustly the worst across both tokenizers (~40% above the best).

Two findings hold across both tokenizers:
- **DeBruijn leaves are worse than display names** in BPE-friendly contexts (bpe-hybrid loses to bpe-optimized on both).
- **Structural-form encodings (s-expr int-IDs) lose to surface-form encodings (BPE) once the AST is non-trivial** — the gap is wide and tokenizer-stable.

The real Q1 decision should defer to Claude's tokenizer (the production target per Q7), which means **bpe-compact is the current Stage 1 leader** — pending the supplementary fragments below.

---

# Sample 2 — 100-node AST (list processing, mutual recursion)

Closes two open items from Sample 1: (a) covers the kinds Sample 1 couldn't reach (`rec`, `match`, `arm`, `ctor`, `hole`, `ann`) and (b) gives BPE's keyword overhead enough program to amortize across. Exactly 100 nodes.

## Logical form

```
rec {
  length  = λxs. match xs with | Nil => Zero | Cons h t => Succ (length t),
  isEmpty = λxs. match xs with | Nil => True | Cons h t => False,
  head    = λd. λxs. match xs with | Nil => d | Cons h t => h
} in
let xs : List = Cons 1 (Cons 2 (Cons 3 (Cons 4 (Cons 5 Nil)))) in
let first : Nat = head Zero xs in
let r = { len: length xs, empty: isEmpty xs, first: first } in
if r.empty then _ else r.first
```

Exercises (kinds new vs. Sample 1 in **bold**): `let` (×3), `lam` (×4), `app` (×5), `record` (3 fields), `proj` (×2), `if`, var refs, integer literals, interned symbols, **`rec` (3 bindings)**, **`match` (×3)**, **`arm` (×6)**, **`ctor` (×15)**, **`ann` (×2)**, **`hole` (×1)**. Constructors used: `Nil`, `Cons`, `Zero`, `Succ`, `True`, `False`, `List`, `Nat`.

### Convention note: `rec` shape

The draft kind table lists `rec` as arity `N` (N binding RHSes, no body). For this sample, `rec` is treated as `1+N` — N bindings followed by a body that sees them — so the program can actually *use* the bound names. This matches the natural "letrec" shape (and is consistent with `let` and `match` each carrying a body). The `N`-vs-`1+N` question is a spec ambiguity to resolve in Stage 2; it doesn't change the token count comparison here (all five encodings render the same tree).

### Convention note: pattern variables

Constructor patterns like `Cons h t` bind two positional pattern-vars. They're counted as leaf nodes (same as `@name` symbols and literals), one per bound position. In the DeBruijn-indexed / hybrid encodings, pattern-var names are stripped since they're positional; the ctor's arity is assumed known to the reader.

## DeBruijn form (sketch)

```
rec
  B0: λ. match (var 0)
         [ arm (ctor @Nil)          (ctor @Zero)
         , arm (ctor @Cons · ·)     (ctor @Succ (app (var 3) (var 0))) ]
  B1: λ. match (var 0)
         [ arm (ctor @Nil)          (ctor @True)
         , arm (ctor @Cons · ·)     (ctor @False) ]
  B2: λ. λ. match (var 0)
             [ arm (ctor @Nil)      (var 2)
             , arm (ctor @Cons · ·) (var 1) ]
  body:
    let (ann 5-elem-Cons-list (ctor @List))
    let (ann (app (app (var 1) (ctor @Zero)) (var 0)) (ctor @Nat))
    let (record @len → app (var 4) (var 1)
                @empty → app (var 3) (var 1)
                @first → var 0)
    if   (proj (var 0) @empty)
    then (hole #7)
    else (proj (var 0) @first)
```

`·` marks a pattern-var binder (unnamed, positional). Depth numbers assume rec binds its N names into scope for the body (innermost listing = lowest depth among the rec bindings); exact integers are illustrative since any single digit tokenizes to the same token count.

## Node count

1 rec · 18 length-lam · 15 isEmpty-lam · 14 head-lam · 52 rec-body = **100 nodes**.

Breakdown of each lambda:
- **length** (18): lam · match · scrut-var · arm(ctor@Nil + ctor@Zero) · arm(ctor@Cons + 2·patvar + ctor@Succ + app + 2·var) = 1+1+1+(1+2+2)+(1+4+5).
- **isEmpty** (15): same skeleton, body ctors are `@True`/`@False` (no recursion).
- **head** (14): two nested lams, match, arms return pattern-bound vars directly.

Breakdown of the rec body (52):
- outer `let xs : List = …` (ann adds 1+1+17+2 over 5-element Cons list and `@List` type) = 21 plus continuation.
- `let first : Nat = head Zero xs` (ann: head-application + `@Nat` type, 1+1+6+2) = 10 plus continuation.
- `let r = { …3 fields… }` (record: 1+4+4+2) = 12 plus continuation.
- `if r.empty then _ else r.first` = 9.

## Per-candidate encodings

The canonical strings live in [`tools/q1-scoring/candidates.py`](../../tools/q1-scoring/candidates.py); shown here inline for reading.

### sexpr-int-ids

```
(3 (0 (5 0 (6 (9 @Nil) (9 @Zero)) (6 (9 @Cons 0 0) (9 @Succ (1 3 0))))) (0 (5 0 (6 (9 @Nil) (9 @True)) (6 (9 @Cons 0 0) (9 @False)))) (0 (0 (5 0 (6 (9 @Nil) 2) (6 (9 @Cons 0 0) 1)))) (2 (11 (9 @Cons #1 (9 @Cons #2 (9 @Cons #3 (9 @Cons #4 (9 @Cons #5 (9 @Nil)))))) (9 @List)) (2 (11 (1 (1 1 (9 @Zero)) 0) (9 @Nat)) (2 (7 @len (1 4 1) @empty (1 3 1) @first 0) (4 (8 0 @empty) (10 #7) (8 0 @first))))))
```

### glyph-prefix

Variadic kinds (`*`, `|`, `!`) use `glyph( … )` delimiters; fixed-arity kinds are juxtaposition.

```
*( \ |( 0 > !( @Nil ) !( @Zero ) > !( @Cons 0 0 ) !( @Succ . 3 0 ) ) \ |( 0 > !( @Nil ) !( @True ) > !( @Cons 0 0 ) !( @False ) ) \ \ |( 0 > !( @Nil ) 2 > !( @Cons 0 0 ) 1 ) = : !( @Cons #1 !( @Cons #2 !( @Cons #3 !( @Cons #4 !( @Cons #5 !( @Nil ) ) ) ) ) ) !( @List ) = : . . 1 !( @Zero ) 0 !( @Nat ) = { @len . 4 1 @empty . 3 1 @first 0 } ? / 0 @empty _ #7 / 0 @first )
```

### bpe-optimized (display names, spaced)

Extends the Stage 1 BPE grammar with a `rec { name = expr ; … } in body` form and `let name : Type = value in body` for type annotations (no `rec` or `ann` in the original draft grammar).

```
rec { length = lambda xs . match xs with | Nil => Zero | Cons h t => Succ ( length t ) ; isEmpty = lambda xs . match xs with | Nil => True | Cons h t => False ; head = lambda d . lambda xs . match xs with | Nil => d | Cons h t => h } in let xs : List = Cons 1 ( Cons 2 ( Cons 3 ( Cons 4 ( Cons 5 Nil ) ) ) ) in let first : Nat = head Zero xs in let r = { len : length xs , empty : isEmpty xs , first : first } in if r . empty then _ else r . first
```

### bpe-compact (no spaces around `.` / `:`, tight braces)

```
rec {length = lambda xs. match xs with | Nil => Zero | Cons h t => Succ (length t); isEmpty = lambda xs. match xs with | Nil => True | Cons h t => False; head = lambda d. lambda xs. match xs with | Nil => d | Cons h t => h} in let xs: List = Cons 1 (Cons 2 (Cons 3 (Cons 4 (Cons 5 Nil)))) in let first: Nat = head Zero xs in let r = {len: length xs, empty: isEmpty xs, first: first} in if r.empty then _ else r.first
```

### bpe-hybrid (keyword skeleton, DeBruijn integers at var-ref sites)

Drops binder names on `lambda`, `let`, and rec bindings; also drops pattern-var names (Cons arity is assumed from the ctor). Ctor names and field names are retained (they're symbols, not var refs).

```
rec { = lambda match 0 with | Nil => Zero | Cons => Succ ( 3 0 ) ; = lambda match 0 with | Nil => True | Cons => False ; = lambda lambda match 0 with | Nil => 2 | Cons => 1 } in let : List = Cons 1 ( Cons 2 ( Cons 3 ( Cons 4 ( Cons 5 Nil ) ) ) ) in let : Nat = 1 Zero 0 in let = { len : 4 1 , empty : 3 1 , first : 0 } in if 0 . empty then _ else 0 . first
```

## Results

### Under tiktoken `cl100k_base`

| Encoding         | Chars | Tokens | Ratio | Δ vs. Sample 1 |
|------------------|------:|-------:|------:|----------------|
| sexpr-int-ids    |  417  |  230   | 1.78× | worsened (1.41× → 1.78×) |
| glyph-prefix     |  371  |  180   | 1.40× | **collapsed** (1.00× → 1.40×) |
| bpe-optimized    |  447  |  135   | 1.05× | improved (1.06× → 1.05×) |
| **bpe-compact**  |  416  | **129**| **1.00×** | **winner** — flipped from tied-middle to winner |
| bpe-hybrid       |  356  |  131   | 1.02× | closed the gap (1.09× → 1.02×) |

### Under Claude `claude-opus-4-7`

Net counts subtract a 12-token envelope baseline; ratios use Net.

| Encoding         | Chars | Net Tokens | Ratio | Δ vs. Sample 1 |
|------------------|------:|-----------:|------:|----------------|
| sexpr-int-ids    |  417  |  323       | 1.62× | worsened (1.39× → 1.62×) |
| glyph-prefix     |  371  |  282       | 1.42× | **collapsed** (1.05× → 1.42×) |
| bpe-optimized    |  447  |  213       | 1.07× | flat (1.05× → 1.07×) |
| **bpe-compact**  |  416  | **199**    | **1.00×** | **winner** — unchanged (held the title from Sample 1) |
| bpe-hybrid       |  356  |  204       | 1.03× | closed the gap (1.07× → 1.03×) |

### Cross-tokenizer comparison at 100 nodes

| Encoding         | tiktoken ratio | Claude ratio | Δ | Robustness |
|------------------|---:|---:|---:|---|
| sexpr-int-ids    | 1.78× | 1.62× | −0.16 | robustly worst; gap widens with size |
| glyph-prefix     | 1.40× | 1.42× | +0.02 | stable but no longer competitive |
| bpe-optimized    | 1.05× | 1.07× | +0.02 | stable middle |
| bpe-compact      | 1.00× | 1.00× |  0.00 | **stable winner on both tokenizers** |
| bpe-hybrid       | 1.02× | 1.03× | +0.01 | stable close second |

## Headline: BPE amortization hypothesis validated

Between 21 and 100 nodes, the competitive picture changes sharply:

- **bpe-compact** is now the decisive winner on *both* tokenizers (it was already winning on Claude; now it also wins on tiktoken). No winner flip between tokenizers at this size.
- **glyph-prefix** collapsed from the 21-node tiktoken winner to +40–42% over bpe-compact on both tokenizers. Variadic constructs (`rec`, `match`, `ctor`) inflate its paren-delimiter cost exactly where BPE amortizes keywords — the glyph candidate was winning on small terms by avoiding keywords it didn't need, and loses at scale by paying paren costs it can't avoid.
- **sexpr-int-ids** got worse at scale, not better. Every added subtree adds `(` `n` ` ` ... `)` overhead; there's no amortization story here.
- **bpe-hybrid** closed most of its gap with bpe-optimized/compact (1.09× → 1.02× tiktoken, 1.07× → 1.03× Claude). DeBruijn leaves still cost a small amount vs. display names, but the deficit shrinks as the keyword skeleton's share of the program grows.
- The three BPE variants cluster within 1.00–1.07× on both tokenizers. The real decision is between *BPE-family* and *not*.

**Stage 1 recommendation (standing):** bpe-compact. It wins on the production tokenizer, now also wins on tiktoken, and its 100-node ratio is within 3% of its 21-node ratio — the most size-stable of the five.

---

## Scoring procedure

1. Run each encoding through `tiktoken.get_encoding("cl100k_base").encode(...)` and record token counts. — done via `tools/q1-scoring/score.py`, which iterates both samples.
2. Record a side-by-side table here with raw counts and ratios relative to the smallest. — done above for both samples.
3. Re-score against Claude's tokenizer; flag any large divergence (>15%) as a robustness concern for whichever candidate moves the most. — done via `tools/q1-scoring/score_claude.py`. On Sample 1 no candidate moved >15% but the *winner flipped* between tokenizers; on Sample 2 the winner is the same on both (bpe-compact).
4. Keep both ASTs frozen for the duration of Stage 1 scoring — if either changes, all five candidates re-score together so comparisons stay apples-to-apples.

## Open items

- ~~Supplementary fragments needed.~~ **Closed by Sample 2**, which covers `rec`, `match`/`arm`, `ctor`, `hole`, and `ann`. Outcome: the BPE candidate's lead widened, not narrowed, when `match` was added.
- ~~Larger sample for BPE amortization.~~ **Closed by Sample 2.** Keyword amortization confirmed: BPE's ratios improved or held flat; glyph-prefix and sexpr-int-ids both worsened meaningfully.
- ~~Tokenizer substitution (Claude).~~ **Closed** — Q7 resolved to Claude Opus 4.7 and both samples are scored against it.
- ~~**`rec` arity in canonical: `N` vs. `1+N`.**~~ **Closed by [ADR 0004](../../decisions/0004-rec-arity.md):** inner `rec` is 1+N (as Sample 2 used); a separate `module` kind covers the top-level N-arity case. Kind tables in both candidate docs updated.
- ~~**BPE grammar gaps.**~~ **Closed:** Sample 2's `rec { name = expr ; … } in body` and `let name : Type = value in body` extensions folded into [authoring-bpe-compact.md](authoring-bpe-compact.md) (doc renamed from `authoring-bpe-optimized.md` to match the chosen variant per [ADR 0003](../../decisions/0003-authoring-view-bpe-compact.md)).
- ~~**Pattern-var rendering in bpe-hybrid.**~~ **N/A — bpe-hybrid rejected** in [ADR 0003](../../decisions/0003-authoring-view-bpe-compact.md) (one of the reasons was exactly this sidecar dependency). The chosen bpe-compact form retains display names for pattern variables.
- ~~**DeBruijn rendering for the BPE-hybrid form.**~~ **N/A — bpe-hybrid rejected.** The chosen bpe-compact form carries display names at every binder; round-trip to canonical DeBruijn goes through the display-name sidecar (Q5).
- **Third data point?** Both samples are lambda-calc-shaped. A radically different-shaped program (long linear chain of `let`s, or heavy string-literal content) would show whether the bpe-compact lead holds on the evaluation corpus. **Deferred to Stage 4** when the corpus freezes; if the lead reverses there, [ADR 0003](../../decisions/0003-authoring-view-bpe-compact.md) is superseded.
