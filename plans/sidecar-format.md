# Display Metadata Sidecar Format (`.tacd`)

**Status:** Frozen 2026-04-22 ([ADR 0017](../decisions/0017-stage-3-frozen.md))
**Parent:** [phase-0-plan.md](phase-0-plan.md)
**Decision:** [ADR 0014](../decisions/0014-sidecar-format.md)

The sidecar is the advisory companion to a `.tac` file. It carries the information canonical text throws away: binder display names, user-authored record-field order, and comments. Both the authoring view and the inspection view consume it; neither requires it (missing and stale sidecars degrade gracefully via synthetic names).

This document specifies the sidecar's JSON schema, its staleness-detection rule, and the authoring/inspection view's fallback behavior.

## Glossary

- **Parallel tree** — the sidecar's tree structure is isomorphic to the AST's structure. Walk both in lockstep; at each node, the sidecar entry holds metadata for the co-positional AST node.
- **Canonical child order** — the order in which a node's children appear in canonical text (per [canonical-text-format.md § 2](canonical-text-format.md)). For most kinds this matches authoring order; `record` is the exception (§ 3.4 below).
- **Synthetic name** — a name generated on the fly by a view layer when no sidecar name is available. Format is specified in § 5.

## 1. File layout

```
foo.tac    # canonical text, authoritative
foo.tacd   # JSON sidecar, advisory
```

The sidecar is pure JSON (UTF-8), no comments, conventional formatting. It is not hashed, not canonicalized, and not part of the content-addressing surface. Two semantically identical sidecars that differ only in JSON whitespace or key order are both valid.

## 2. Top-level schema

```json
{
  "tacd_version": "1",
  "targets_hash_blake3": "<64 lowercase hex chars>",
  "display": { ... }          // parallel tree; see § 3
}
```

- `tacd_version` (string, required) — schema version. Currently `"1"`. Readers MUST reject a sidecar whose version they do not recognize (warn + fall back to no-sidecar rendering, not hard error).
- `targets_hash_blake3` (string, required) — BLAKE3 of the canonical text bytes of the paired `.tac` file, as 64 lowercase hex characters. Used for staleness detection (§ 4).
- `display` (object, required) — the root of the parallel tree. § 3.

Unknown top-level keys MUST be ignored (additive evolution).

## 3. Node entries

Every AST node has a corresponding sidecar entry. An entry is a JSON object with two classes of fields:

1. **Metadata keys** — this node's display metadata. All optional. Defined in § 3.1–3.4.
2. **Structural key** — `children`, a JSON array of child entries in canonical child order. Absent if the AST node has zero canonical children.

### 3.1 Binder names: `binder` and `binders`

For nodes that introduce exactly one name:

| AST kind   | Metadata key | Meaning                                  |
|------------|--------------|------------------------------------------|
| `lam`      | `binder`     | Parameter name; scope is the body.       |
| `let`      | `binder`     | Let-bound name; scope is the body only.  |
| `pat-var`  | `binder`     | Pattern-variable name; scope is the arm body, per [canonical-text-format.md § 4](canonical-text-format.md). |

For nodes that introduce N names simultaneously:

| AST kind | Metadata key | Shape                        | Meaning                              |
|----------|--------------|------------------------------|--------------------------------------|
| `rec`    | `binders`    | array of string, length N    | Position K = name for `(var K)`.     |
| `module` | `binders`    | array of string, length N    | Same; see [ADR 0007](../decisions/0007-debruijn-rec-indexing.md). |

Names are strings drawn from whatever character set the authoring view accepts (Phase 0: ASCII identifiers matching `[A-Za-z_][A-Za-z0-9_]*`, per authoring-bpe-compact.md). Phase 1+ may loosen.

The sidecar never stores names for `var` references — those are computed during rendering by maintaining a binding stack as the view layer walks the tree.

### 3.2 Comments: `comment`

Any node may carry a `comment` string. It is advisory and renders only in views that choose to show comments (the inspection view does; the authoring view does not, because it's optimized for density).

### 3.3 Record field order: `field_order`

`record` is the only AST kind where canonical and authoring orders diverge ([ADR 0008](../decisions/0008-record-field-ordering.md) sorts fields alphabetically at canonical emission). The sidecar records the authoring-view order via:

- `field_order` (array of int, length N) — `field_order[i]` is the canonical field index that should render at authoring position *i*.

Equivalent to saying: "To render this record in authoring-view order, visit canonical fields in the order given by `field_order`."

Example: if the user wrote `{snd: 2, fst: 1, mid: 3}` (authoring order `snd, fst, mid`) and canonical sorts to `fst, mid, snd`, the sidecar is:

```json
{ "field_order": [2, 0, 1] }
```

because canonical index 2 is `snd` (authoring position 0), canonical index 0 is `fst` (authoring position 1), canonical index 1 is `mid` (authoring position 2).

If `field_order` is absent, authoring view renders in canonical order. Length mismatch (array length ≠ N) is a stale sidecar; § 4.

### 3.4 Children: `children`

Array of child entries in canonical child order.

Child entries may be:

- A full JSON object (metadata + optional `children`).
- `null` — means "no metadata anywhere in this subtree." Equivalent to `{}`.

**Truncation rule.** Let *N* be the AST node's canonical child count. `children` may have length *K* with `0 ≤ K ≤ N`; entries at positions `K`, `K+1`, …, `N-1` are implicitly `null`. Absent `children` (the key is not present) is equivalent to `K = 0`, i.e., every canonical child entry defaults to `null`. A node with zero canonical children (`N = 0`) therefore always omits the key; a node whose subtree carries no metadata anywhere can also omit it. Internal absent entries inside an explicit array are forbidden — arrays must be dense up to their literal length.

A `children` array with **more** entries than the AST has canonical children (`K > N`) is a structural mismatch and triggers the stale-sidecar path (§ 4).

### 3.5 Type and effect hints (live keys)

`type_hint` and `effect_hint` are **live keys** per ADR 0071, promoted from reserved status. Both are optional and may appear on any `SidecarNode`; on the root `display` node they describe the program's evaluated value type and effect set.

| Key | Type | Meaning |
|---|---|---|
| `type_hint` | `string` | Authoring-view type string for the node's value type, e.g. `"Int -> Int"`. |
| `effect_hint` | `string[]` | Sorted list of effect atom names, e.g. `["IO", "Mut"]`. Empty array and absent key are both treated as "pure." |

**Worked example** — a pure factorial function:

```json
{
  "tacd_version": "1",
  "targets_hash_blake3": "a1b2c3...",
  "display": {
    "type_hint": "Int -> Int",
    "effect_hint": [],
    "binders": ["factorial"],
    "children": [...]
  }
}
```

Per-binding hints on child nodes (non-root) are a future expansion path and are not produced by current tooling; readers that encounter them on child nodes MUST ignore them rather than error.

No `tacd_version` bump is required: § 2 already mandates that readers ignore unrecognised keys, so existing readers silently skip these keys when absent and new readers can consume them.

### 3.6 Reserved keys

The following keys are reserved for future phases and MUST NOT appear in a `tacd_version: "1"` sidecar produced by current tooling:

- `source_range` — source-position mapping for authoring-view editors.
- `diagnostic_extra` — extended payload for `hole` nodes beyond the canonical payload string.

Readers MUST ignore any key they do not recognize (§ 2).

## 4. Staleness detection and partial-match recovery

A sidecar is **fresh** if its `targets_hash_blake3` equals BLAKE3 of the paired `.tac` file's canonical bytes. A fresh sidecar is assumed structurally aligned with the AST, and its metadata is consumed without further validation.

A sidecar is **stale** otherwise. View layers handle stale sidecars by:

1. Emitting a diagnostic (file-level warning) naming the expected vs. actual hash.
2. Walking the AST and sidecar trees in parallel. At every node:
   - If the sidecar entry's shape is **compatible** with the AST node, use its metadata. Compatible means `children` length *K* satisfies `K ≤ N` (where *N* is the AST's canonical child count); shorter arrays extend with implicit `null` per § 3.4.
   - If the sidecar entry has `K > N` (more children than the AST), or any metadata key that structurally contradicts the AST node (e.g., `binders` on a `lam`, or `binder` on a `rec`), use synthetic names (§ 5) for this node and all descendants.

This means a one-character edit to a distant subtree doesn't wipe out the entire sidecar — the unaffected parts still render with their names. The `K ≤ N` direction is treated as compatible (not stale) because it's the shape the § 3.4 truncation rule emits in the fresh case; only over-count or key/kind mismatches signal that the sidecar was written against a different tree shape.

### Hash mismatch should be rare in practice

The authoring flow is: edit canonical text → re-derive sidecar from the same AST → save both. An AI author that emits `.tac` and `.tacd` together will only produce mismatches when a human or tool edits one file without the other. Tooling around the project should prefer atomic save-pairs.

## 5. Synthetic names (no/stale sidecar)

When a view layer cannot find a name in the sidecar, it generates one. The scheme is stable within a single rendering (same AST, same fallback mode → same synthetic names), but is *not* a canonical form.

- **`lam` parameters** — `v0`, `v1`, … numbered outward from the innermost enclosing `lam` or `let`. (I.e., in a rendering walk, each `lam` introduces `v{depth}` where `depth` is the number of enclosing `lam`/`let`/etc. already in scope.)
- **`let` binders** — same `v0`, `v1`, … sequence; `let` and `lam` share the binder numbering.
- **`rec` bindings** — `B0`, `B1`, …, `B{N-1}` matching the canonical position (and therefore matching the DeBruijn index per [ADR 0007](../decisions/0007-debruijn-rec-indexing.md)).
- **`module` bindings** — same `B0`…`B{N-1}` sequence.
- **`pat-var` inside an arm** — `p0`, `p1`, … numbered in the textual order of `pat-var` appearances in the pattern. Note that [canonical-text-format.md § 4](canonical-text-format.md) assigns the *highest* DeBruijn index to the first-encountered `pat-var` and index 0 to the last; so synthetic `p0` corresponds to the highest DeBruijn index, `p_{K-1}` to index 0. The synthetic-name numbering tracks textual order (readable left-to-right in the pattern), not DeBruijn order.

Synthetic names are advisory and for legibility only; they never flow into the canonical form.

## 6. Worked example

Canonical (the Stage 2 worked example from canonical-text-format.md § 10):

```
(let (lam (var 0)) (app (var 0) (int 5)))
```

Authoring view: `let id = lambda x. x in id 5`.

Full sidecar:

```json
{
  "tacd_version": "1",
  "targets_hash_blake3": "<blake3 of the canonical bytes above>",
  "display": {
    "binder": "id",
    "children": [
      {
        "binder": "x",
        "children": [ null ]
      },
      {
        "children": [ null, null ]
      }
    ]
  }
}
```

Reading the tree:

- Root is `let`. Its `binder` is `id`. Its canonical children are `(lam ...)` and `(app ...)`.
- First child is `lam`. Its `binder` is `x`. Its canonical child is `(var 0)`; the `var` has no metadata of its own, so the entry is `null`.
- Second child is `app`. No metadata on the `app` node. Its canonical children are `(var 0)` and `(int 5)`, both leaves with no metadata — both `null`.

Compressed equivalent (trailing nulls omitted):

```json
{
  "tacd_version": "1",
  "targets_hash_blake3": "<hex>",
  "display": {
    "binder": "id",
    "children": [
      { "binder": "x" },
      {}
    ]
  }
}
```

(The inner `lam`'s `children: [null]` and the `app`'s `children: [null, null]` are both "only nulls," which is equivalent to omitting `children` when the reader validates via the AST.)

## 7. Mutual recursion worked example

Canonical (truncated for brevity; see canonical-text-format.md § 10 for the full form):

```
(rec (lam ...) (lam ...) (app (var 0) (int 10)))
```

Sidecar:

```json
{
  "tacd_version": "1",
  "targets_hash_blake3": "<hex>",
  "display": {
    "binders": ["even", "odd"],
    "children": [
      { "binder": "n", "children": [ ... ] },
      { "binder": "n", "children": [ ... ] },
      { "children": [ null, null ] }
    ]
  }
}
```

Per [ADR 0007](../decisions/0007-debruijn-rec-indexing.md), `binders[0]` = `even` = `(var 0)` and `binders[1]` = `odd` = `(var 1)`, matching canonical position. The rec body's own children (app/var/int) have no sidecar metadata.

## 8. Record example with `field_order`

Canonical:

```
(record fst (int 1) mid (int 3) snd (int 2))
```

(alphabetical: `fst, mid, snd`; record field-syms are bare in canonical form — the `@` prefix is authoring/inspection-view decoration only, per [canonical-text-format.md § 3](canonical-text-format.md) and [inspection-view.md § 3.11](inspection-view.md).)

Authoring view that the user wrote: `{snd: 2, fst: 1, mid: 3}` (`snd, fst, mid`).

Sidecar:

```json
{
  "display": {
    "field_order": [2, 0, 1],
    "children": [ null, null, null, null, null, null ]
  }
}
```

The `children` array has 2N = 6 entries, one per canonical (sym, val) child. The sym and val nodes typically have no metadata (syms are already opaque symbols; ints are leaves), so all six entries are `null`. Leading-and-trailing nulls collapse to `children: []` or omit.

## 9. Edge cases

- **Empty records.** `(record)` has zero canonical children; `field_order` is omitted or `[]`. Sidecar entry: `{}`.
- **Nullary ctors.** `(ctor Nil)` has one canonical child (the name-sym) and no meaningful per-child metadata. Sidecar entry typically `{}` or `null`.
- **Holes.** `(hole unexpected-token "payload")` is like any other AST node; its sidecar entry carries a `comment` if desired (e.g., a human-written note about why the hole exists). The payload string itself lives in canonical form and is not re-stored in the sidecar.
- **Pattern variables.** `(pat-var)` is nullary canonically. Its sidecar entry carries `binder` (the pattern-variable's display name). The enclosing `pat-ctor` does not need any metadata for its sub-pattern names — each `pat-var` carries its own.
- **Deeply nested all-null subtrees.** A subtree with no metadata anywhere collapses to `null` at any level. A rendering walk treats `null` the same as `{}` with an inferred `children` array matching the AST.

## 10. Open items

- **Comment placement conventions.** Phase 0 defines the key; conventions around multi-line comments, doc comments on bindings, etc. are an inspection-view concern and resolve with ADR 0015.
- **Phase 1 extension points.** `type_hint`, `effect_hint`, `source_range`, and `diagnostic_extra` are reserved; their precise schemas land with the Phase 2 effect system and Phase 4 debugging tooling respectively.
- **Cross-project name overlays.** The deferred object-store design ([tacit-plan.md § Object store](tacit-plan.md)) will need a way for one project to bind different names to an imported hash. A content-hash-keyed overlay format is the natural fit and can consume the Phase 0 parallel-tree sidecars as a source. Out of scope here.

## 11. Exit criteria for this doc

Frozen 2026-04-22 alongside [inspection-view.md](inspection-view.md) per [ADR 0017](../decisions/0017-stage-3-frozen.md). At this freeze:

- The schema in § 2–3 is locked, including the § 3.4 `children` truncation rule and § 4 staleness rule.
- The synthetic-name scheme in § 5 is locked.
- The worked examples in § 6–8 are regression fixtures. No end-to-end round-trip test was executed at freeze time; Phase 1's first renderer will close that loop, and any divergence it surfaces is a Phase 0 spec bug per [CLAUDE.md](../CLAUDE.md) rather than new design work.

Changes after this freeze require a new ADR per [CLAUDE.md](../CLAUDE.md) ground rules.
