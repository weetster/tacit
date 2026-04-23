# Inspection View

**Status:** Frozen 2026-04-22 ([ADR 0017](../decisions/0017-stage-3-frozen.md))
**Parent:** [phase-0-plan.md](phase-0-plan.md)
**Decision:** [ADR 0015](../decisions/0015-inspection-view-scope.md)

The inspection view is the read-only projection of a Tacit-Lite AST used for code review, debugger output, and diff presentation. It is not round-trippable back to canonical bytes — canonical text is the authoritative source, and the inspection view is a function of (AST + sidecar + flags).

This document specifies the projection rules: line-break policy, per-kind rendering, and the progressive-annotation flag layers (L0 default, L1 DeBruijn, L2 hashes).

## Glossary

- **L0 / L1 / L2** — the three annotation layers defined in [ADR 0015](../decisions/0015-inspection-view-scope.md). L0 is the default rendering; L1 and L2 are additive flag-gated overlays. Phase 1+ may introduce additional layers (e.g., `--types`, `--effects`) but Phase 0 ships L0–L2.
- **Inline subtree** — a subtree whose L0 rendering contains no line breaks. This is determined recursively by the per-kind rules in § 3; it is the *single* line-break criterion used throughout this document. "Inline" replaces the looser "trivial" term used in earlier drafts.
- **Always-break kind** — a compound kind (`let`, `rec`, `module`, `if`, `match`) whose L0 rendering always spans multiple lines, regardless of child inlineability. Reason: these forms are the readability anchors for an inspection-view reader; forcing them multi-line means structure never collapses into a single hard-to-scan line.
- **Binding stack** — the view layer's walk-state: a stack of names introduced by enclosing `lam`/`let`/`rec`/`module`/`pat-var`, used to resolve `var N` references to names.

## 1. Surface form

Pseudo-code: indented, keyword-led. UTF-8 output; ASCII for identifiers and keywords, Unicode permitted for annotation glyphs.

**Indentation:** 2 spaces per nesting level. No tabs.

**Line-break policy:** deterministic, per-kind, specified in § 3. Summarized:

- Leaves (`var`, `int`, `str`, `sym`, `pat-wild`, `pat-var`) and `hole` are always inline.
- `ctor`/`pat-ctor`, `proj`, `ann`, `app`, `record`: inline iff all their children are inline (§ 3.2, § 3.7–§ 3.9).
- `lam` and `arm`: inline iff the body is inline.
- `let`, `rec`, `module`, `if`, `match`: always-break kinds; never inline at L0.

The renderer makes these decisions bottom-up: each node decides based on its already-rendered children. There is no column-width budget at L0 — break-or-inline is a structural function of the subtree, not of its printed width. (Phase 1+ renderers may add a width-aware soft-wrap layer; Phase 0's spec deliberately does not.)

**Comment glyph:** `#` introduces a line-trailing or line-leading comment (both for sidecar `comment` metadata and for L1/L2 annotations that the spec renders as comments).

## 2. Name resolution

For each `var N` the view layer consults the binding stack:

- If the stack has a name at depth N, render that name.
- Otherwise (stale sidecar or no sidecar), render the synthetic name per [sidecar-format.md § 5](sidecar-format.md).

Names from the sidecar take precedence over synthetic names when both are available. Shadowing is allowed: if the author wrote `lambda x. let x = … in x`, the inner `x` resolves to the `let` binding per the standard DeBruijn rule.

## 3. Per-kind rendering at L0

Every kind in [canonical-text-format.md § 2](canonical-text-format.md) has a rule below.

### 3.1 `lam`

```
lambda X. BODY                   # if BODY is inline

lambda X.                        # otherwise
  BODY
```

`X` is the sidecar-supplied binder name (or synthetic). If a `lam` has an immediately-nested `lam`, both binders have sidecar names, **and** the innermost body is inline, the chain collapses:

```
lambda X Y Z. BODY               # collapsed; equivalent to nested lambdas
```

The collapse is a rendering choice, not a grammar change — canonical form still has nested `lam` nodes. If any inner body is not inline, the chain does not collapse.

### 3.2 `app`

Left-associative curried call chains flatten:

```
F X Y Z                          # inline iff F and every argument are inline
```

If any argument is not inline, render the head inline and break arguments onto their own lines at one deeper indent:

```
F
  X
  Y
  Z
```

Arguments that are themselves applications get parenthesized only when disambiguation is needed — the reader relies on left-associativity as the default. A parenthesized argument is inline iff its contents are inline.

### 3.3 `let`

`let` is an **always-break kind**. `B` always appears on a new line below `in` at one deeper indent. `V` inlines if V is inline; otherwise `V` breaks onto its own indented line and `in` takes its own line:

```
let X = V in                     # V is inline
  B

let X =                          # V is not inline
  V
in
  B
```

Chained `let`s (`let a = … in let b = … in …`) render without nested indentation growth — each `in` resets the indent. Only the final `B` receives the deeper indent:

```
let a = v0 in
let b = v1 in
let c = v2 in
  BODY
```

(If any intermediate `V` on the chain is itself not inline, that `let` uses the three-line form and the chain-without-nesting property applies to the `let`s that follow.)

If any `let` on the chain has a type annotation (via an `ann` child at the rhs), the annotation renders inline on the binder:

```
let X: T = V in
  B
```

### 3.4 `rec` and `module`

`rec` and `module` are **always-break kinds**. Each binding occupies its own line, led by `|` (visual separator, not canonical syntax) at one indent level deeper than the `rec`/`module` keyword. Inline RHS stays on the `|` line; a non-inline RHS breaks below with the continuation indented two levels deeper than the `|` marker (so a `|` at column 2 places the broken RHS at column 6). The extra indent gives the continuation visual clearance from the `|`:

```
rec
  | X0 = E0                      # E0 inline
  | X1 =                         # E1 not inline
      E1
  | X2 = E2                      # E2 inline
in
  BODY

module
  | X0 = E0
  | X1 = E1
```

Worked example with nested always-break kinds (`match` inside `lambda` inside the binding RHS):

```
rec
  | length =
      lambda xs.
        match xs
        | Nil => Zero
        | Cons h t =>
            Succ (length t)
  | isEmpty =
      lambda xs.
        match xs
        | Nil => True
        | Cons _ _ => False
in
  length myList
```

Per [ADR 0007](../decisions/0007-debruijn-rec-indexing.md), binding position K matches `(var K)` — the L1 overlay makes this explicit.

### 3.5 `if`

`if` is an **always-break kind**. Always renders three-line:

```
if C
then T
else E
```

Each of `C`, `T`, `E` may itself expand further if not inline. Chained `else if` reflows so the second `if` starts at the same indent level as the first:

```
if C1
then E1
else if C2
then E2
else E3
```

### 3.6 `match` and `arm`

`match` is an **always-break kind**. Each arm renders on its own `|`-led line. An `arm` is inline iff its body is inline:

```
match S
| P0 => E0                       # body E0 is inline
| P1 =>                          # body E1 is not inline
    E1
| P2 => E2
```

Arm order is preserved from canonical (first-match-wins; [canonical-text-format.md § 6](canonical-text-format.md)).

### 3.7 `record` and `proj`

`record` fields render in authoring order if the sidecar's `field_order` is present, otherwise in canonical (alphabetical) order.

- An empty `record` renders `{}`.
- A single-field record whose value is inline renders inline: `{ fst: 1 }`.
- Otherwise (≥ 2 fields, or any field value not inline) the record breaks one-field-per-line with a trailing comma:

```
{
  fst: id 1,
  snd: id 2,
  mid: factorial 5,
}
```

(Under this rule a `record` is inline only in the empty or single-inline-field case; multi-field records always break. This simplifies the spec — a Phase 1+ renderer may introduce a width-aware inline form for small all-inline records, but Phase 0 does not.)

Projection chains flatten: `r.a.b.c` for `(proj (proj (proj r a) b) c)`. A `proj` is inline iff its record sub-expression is inline.

### 3.8 `ctor` and `pat-ctor`

```
Nil                              # nullary (zero args)
Cons 1 2                         # applied; inline iff every arg is inline
```

A `ctor` (or `pat-ctor`) is inline iff every one of its arguments is inline. If any argument is not inline, break with args indented one level:

```
Cons
  (factorial 5)
  rest
```

`pat-ctor` renders the same way in pattern position.

### 3.9 `ann`

```
(E : T)
```

A standalone `ann` is inline iff `E` and `T` are both inline; if so it renders as `(E : T)` on one line, parenthesized. Otherwise it renders with `E` and `T` on their own lines. For typed let bindings, the colon migrates up onto the binder (§ 3.3 above) rather than wrapping the whole rhs. For standalone `ann` expressions (uncommon), the parenthesized form is used.

### 3.10 `hole`

```
⟨hole:DIAG-ID "payload"⟩
```

**Always inline.** Rendered with Unicode bracket glyphs (`⟨ ⟩`, U+27E8 / U+27E9) on a single visual line so the reader can spot holes at a glance. The diag-id and payload string come directly from canonical ([canonical-text-format.md § 7](canonical-text-format.md)). A `hole` never introduces a line break regardless of payload length — a very long payload produces a long single line, not a multi-line expansion. This simplifies break-propagation and is acceptable because holes are rare.

### 3.11 Leaves

- `var N` → the resolved name (§ 2).
- `int V` → decimal digits as-is from canonical.
- `str "..."` → the canonical-form string literal as-is, including escapes. The view layer does not un-escape for display.
- `sym NAME` → `@NAME` (the `@` is a display-layer decoration matching authoring view; not present in canonical bytes).

### 3.12 `pat-wild` and `pat-var`

- `pat-wild` → `_`
- `pat-var` → the sidecar binder name (or synthetic `p0`, `p1`, …)

## 4. Annotation layers

### 4.1 L1 — `--debruijn`

Each `var N` gains a trailing comment showing the DeBruijn index:

```
x  # ≡ var 0
```

Multi-token expressions get their annotations on the line that hosts the `var`:

```
id 5  # id ≡ var 0
```

Inside an arm body with pattern variables, the comment names the binder:

```
Succ (length t)  # length ≡ var 3, t ≡ var 0
```

For compactness, if multiple `var` references appear on one line, they share a single trailing comment joined by commas.

### 4.2 L2 — `--hashes`

Each non-leaf rendered node gains a leading hash badge:

```
[abc12345] let id =
  [def67890] lambda x.
    [01234567] x
in
  [89abcdef] id 5
```

Hashes are the first 8 hex characters of BLAKE3 over the canonical text of the subtree. Leaves (`var`, `int`, `str`, `sym`, `pat-wild`, `pat-var`) do not get hash badges — their canonical forms are short enough that rendering them inline provides the same information.

L2 is intended for debugging content-addressing: it's visually heavy by design.

### 4.3 L1 + L2 combined

```
[abc12345] let id = [def67890] lambda x. x in  # inner x ≡ var 0
  [89abcdef] id 5  # id ≡ var 0
```

### 4.4 Reserved flags

Specified here for grammar-level reservation; not implemented in Phase 0.

- `--types` — adds inferred types from Phase 2's inference. Rendered as `: T` annotations inline at binder sites that didn't already carry explicit `ann` annotations.
- `--effects` — adds effect-set annotations from Phase 2. Rendered as `! {IO, Alloc}` inline after type annotations, per the effect-set syntax landing in Phase 2.
- `--tree` — replaces pseudo-code surface with Unicode tree-drawing (deferred; not in this doc).
- `--table` — replaces surface with tabular output for machine consumption (deferred; probably consumed by `tacit-debug` in Phase 4 with a JSON emission mode).

Phase 0 does not specify these; the ADR path keeps flags additive so future phases can spec them without revisiting Phase 0's grammar.

## 5. Sidecar comments

When a node has `comment: "…"` in the sidecar, the view layer renders it:

- **Single-line comment** (≤ 80 chars, no newlines): appears on the line before the node, at the node's indentation, prefixed by `# `:
  ```
  # the identity function
  let id = lambda x. x in ...
  ```
- **Multi-line comment** (newlines present or > 80 chars): each line prefixed by `# ` and indented to the node's level.

Comments are rendered even at L0 — they are user-authored content, not annotation noise.

## 6. Worked examples

Using the Stage 2 worked examples from [canonical-text-format.md § 10](canonical-text-format.md).

### 6.1 Identity-of-5

Canonical: `(let (lam (var 0)) (app (var 0) (int 5)))`

Sidecar:
```json
{
  "tacd_version": "1",
  "targets_hash_blake3": "...",
  "display": {
    "binder": "id",
    "children": [{"binder": "x"}, {}]
  }
}
```

**L0:**
```
let id = lambda x. x in
  id 5
```

**L0 + L1:**
```
let id = lambda x. x in  # inner x ≡ var 0
  id 5                   # id ≡ var 0
```

**L0 + L2** (hashes illustrative). Badges are prefixed at the start of each non-leaf node's rendering — inline on the same line when the node is inline, or at the start of the first line when the node breaks:
```
[aaaaaaaa] let id = [bbbbbbbb] lambda x. x in
  [cccccccc] id 5
```

### 6.2 Mutual recursion

Canonical: `(rec (lam (if (var 0) (app (var 2) (ctor sub (var 0) (int 1))) (int 1))) (lam (if (var 0) (app (var 1) (ctor sub (var 0) (int 1))) (int 0))) (app (var 0) (int 10)))`

Sidecar binders: `["even", "odd"]`; lam params both `n`.

**L0:**
```
rec
  | even =
      lambda n.
        if n
        then odd (sub n 1)
        else 1
  | odd =
      lambda n.
        if n
        then even (sub n 1)
        else 0
in
  even 10
```

**L0 + L1** (showing the cross-binding DeBruijn relationships):
```
rec                           # binders: [even ≡ var 0, odd ≡ var 1]
  | even =
      lambda n.               # n ≡ var 0 inside; even ≡ var 1, odd ≡ var 2
        if n
        then odd (sub n 1)    # odd ≡ var 2, n ≡ var 0
        else 1
  | odd =
      lambda n.
        if n
        then even (sub n 1)   # even ≡ var 1, n ≡ var 0
        else 0
in
  even 10                     # even ≡ var 0
```

The L1 overlay here makes ADR 0007's DeBruijn convention directly visible — the reader sees that binding position K in `rec` matches `(var K)` in the body.

### 6.3 Hole in a program

Canonical: `(let (lam (hole expected-expr (str "missing body after lambda"))) (app (var 0) (int 5)))`

Sidecar: `{display: {binder: "id", children: [{binder: "x"}, {}]}}`

**L0:**
```
let id = lambda x. ⟨hole:expected-expr "missing body after lambda"⟩ in
  id 5
```

The hole marker is visually distinctive; a reviewer spots parse failures immediately without reading the diag-id carefully.

## 7. Stability and regression fixtures

Per [ADR 0015](../decisions/0015-inspection-view-scope.md) Consequences:

- The L0 renderings of § 6.1, § 6.2, § 6.3 are Stage 3 exit fixtures.
- Changes that alter L0 output on those examples require a new ADR.
- L1 and L2 renderings in § 6 are illustrative at freeze time; subsequent phases may tighten the annotation format with a grammar-doc update (and a regression-fixture refresh) but not with an ADR.

## 8. Open items

- **Width-aware soft-wrap.** Phase 0's L0 rules are purely structural (per-kind break decisions, no column budget). A Phase 1+ renderer may add an optional width-aware layer that selectively breaks long inline lines onto continuation lines without changing the structural break decisions documented here. The 80-char threshold in § 5 applies only to sidecar-authored comments, not to any structural rendering decision.
- **Inline vs. parenthesized `ann`.** § 3.9's rule ("colon migrates up for typed lets, parenthesized otherwise") covers the common case but has edge cases for e.g. `(ann (match ...) T)` where the match is an always-break kind. Resolvable in Phase 1 when a real renderer exists and hits these.
- **Comment-syntax ambiguity with `#` in string literals.** The renderer emits comments verbatim from sidecar values. A comment containing a `\n` must break into multiple `# `-prefixed lines. Corner case: a comment literal itself containing `# ` renders as-is; ambiguity is resolved because comments are advisory and not re-parsed.
- **L1 annotation placement inside very deep rendering.** When many `var` references appear on one line, the trailing comment can grow long. § 4.1 says "joined by commas"; an open item is whether to allow the comment to wrap onto a second `# …`-prefixed line. Deferred to Phase 1 renderer.

## 9. Exit criteria

Frozen 2026-04-22 alongside [sidecar-format.md](sidecar-format.md) (canonical-text-format.md was frozen earlier by [ADR 0013](../decisions/0013-canonical-text-format-frozen.md)). Per [ADR 0017](../decisions/0017-stage-3-frozen.md):

- The per-kind L0 rendering rules in § 3 are locked — in particular, the always-break kinds and the "inline iff children inline" decision procedure.
- The L1 and L2 overlay rules in § 4 are locked.
- The § 6 worked examples at L0 are regression fixtures. Future changes altering their output require a new ADR.

Changes to the view scope (e.g., making the view round-trippable, swapping the surface form) require an ADR revising [ADR 0015](../decisions/0015-inspection-view-scope.md).
