# 0014 — Display metadata sidecar: JSON parallel tree

**Status:** Accepted
**Date:** 2026-04-22
**Phase:** 0, Stage 3
**Resolves:** Open Question Q5 from [tacit-plan.md § Open Questions](../plans/tacit-plan.md)
**Spec:** [plans/sidecar-format.md](../plans/sidecar-format.md)

## Context

Canonical text format (frozen by [ADR 0013](0013-canonical-text-format-frozen.md)) strips every piece of information that is not semantically load-bearing:

- Variable references are DeBruijn indices, not names.
- Binders (`lam`, `let`, `rec`, `module`, `pat-var`, `pat-ctor`-inner) are implicit — the canonical form does not record what the user called them.
- Record fields are sorted alphabetically at emission time; the user-authored order is discarded.
- Comments do not exist in canonical form at all.

The two view layers (authoring per [ADR 0003](0003-authoring-view-bpe-compact.md); inspection per ADR 0015) need all of that information to produce readable output. Q5 asked where that information lives and how it pairs with the `.tac` file.

This ADR answers that question. The inspection-view scope question (what does the inspection view *show*, with or without the sidecar) is orthogonal and handled in ADR 0015.

### Design space

Three attachment models were considered:

1. **Path-keyed flat map.** Display metadata lives in a flat JSON object keyed by string paths from the AST root (e.g., `"0.1.body": {"binder": "odd"}`). Compact but brittle: any edit that changes tree shape invalidates most keys, and the path syntax needs its own spec.
2. **Content-hash-keyed flat map.** Keyed by BLAKE3 hash of each subtree. Robust under subtree moves (a function pasted elsewhere keeps its names). But fails on implicit binders: a `lam` node's "parameter name" is not a property of any subtree that has a hash — it's a property of the binder position, which is implicit. Forcing a keying scheme that doesn't fit implicit binders adds complexity. Also, hashes change under every edit inside a subtree (that's the Merkle property working correctly), so the robustness benefit applies only to unchanged-subtree moves.
3. **Parallel tree.** Sidecar JSON mirrors the AST's structural shape — same children in the same canonical positions — with display metadata attached at each node. The canonicalizer (or any AST walker) visits both trees in lockstep.

### Constraints from the rest of the spec

- Canonical form has no node IDs and no content hashes inlined ([ADR 0009](0009-hashing-rule.md)). Any key-based scheme has to synthesize keys from tree structure.
- Sidecars are authored by the same AI that authors the canonical text. Emitting a parallel tree is zero extra cognition — write the same structure with names filled in. Emitting path strings or hashes is real work.
- Sidecars are **advisory** ([CLAUDE.md](../CLAUDE.md) ground rules). A missing or stale sidecar must degrade gracefully — the canonical text alone is still a valid, compilable program.
- Per [tacit-plan.md § Storage format](../plans/tacit-plan.md), "different projects can bind different names to the same canonical hash." This matters eventually (dependency-cached content needs per-project name overlays) but is deferred to Phase 1+; Phase 0 sidecars are one-file-per-canonical-file.

## Decision

**The display metadata sidecar is a JSON file whose structure is a parallel tree to the AST, stored at `<name>.tacd` alongside `<name>.tac`.**

Concretely:

1. **Format: JSON** (not a second canonical text format). The full schema is in [plans/sidecar-format.md](../plans/sidecar-format.md); highlights:
   - A top-level object with `tacd_version` (currently `"1"`), `targets_hash_blake3` (hex-encoded BLAKE3 of the paired `.tac` file's canonical bytes, for stale-detection), and `display` (the parallel tree).
   - Each node entry is a JSON object holding only this node's metadata keys plus a `children` array in canonical child order. A node with no metadata and no descendant metadata may be represented as `null` in its parent's `children` array (or omitted from the end). This is the only compression allowed.
   - No per-node hashes, no paths, no IDs.
2. **Phase 0 metadata keys** (additive; Phase 1 may extend):
   - `binder` (string) — for `lam` (parameter name), `let` (let-bound name), `pat-var` (pattern variable name).
   - `binders` (array of string, length N) — for `rec` and `module`, one name per binding position (matching the § 5 DeBruijn convention: position K is `(var K)`).
   - `field_order` (array of int, length N) — for `record`, permutation describing authoring-view field order: the field at authoring position *i* is at canonical position `field_order[i]`. Absent means "emit in canonical order."
   - `comment` (string) — advisory comment attached to this node. Allowed on any node kind.
3. **Staleness detection.** A reader compares `targets_hash_blake3` to BLAKE3 of the paired `.tac` file's canonical bytes. Mismatch means the sidecar is stale and the reader *may* still use it on a best-effort basis, but must surface a warning.
4. **Missing-sidecar fallback.** A `.tac` file without a companion `.tacd` is a valid, renderable program. The view layer synthesizes names:
   - Binders get `v0`, `v1`, … in traversal order within each scope.
   - `rec`/`module` binders get `B0`, `B1`, ….
   - Pattern variables get `p0`, `p1`, … per arm.
   - Records render in canonical (alphabetical) field order.
5. **File extension: `.tacd`** (pinned by tacit-plan.md's Storage format section; previously informal, now formal).
6. **The sidecar does not hash.** Only the `.tac` file participates in content-addressing. Two sidecars binding different display names to the same canonical program are *both* valid overlays on the same content-addressed program.

The parallel-tree model is chosen over path-keyed and content-hash-keyed alternatives.

## Alternatives considered

- **Path-keyed flat map.** Rejected. The AI author has to compute paths, which adds mechanical work with no clear benefit. Also, the path syntax is a second spec surface — another set of edge cases to pin and test.
- **Content-hash-keyed flat map.** Rejected for Phase 0. Does not solve implicit binders (the main category of display metadata), and the hash-stability-under-move benefit is a Phase 1+ concern better handled by a separate cross-project name-overlay layer when dependency caching is actually designed. Parallel-tree does not foreclose this — a content-keyed overlay could be added as a second format later, consuming the parallel-tree sidecars as a name source.
- **A second canonical text format for sidecars.** Rejected. Sidecars are not hashed and not content-addressed; they do not benefit from byte-exact determinism. JSON has existing tooling, diff-friendliness, and a well-understood tradeoffs profile. The "cheap default" framing from [phase-0-plan.md § Stage 3](../plans/phase-0-plan.md) applies.
- **Embed display metadata in `.tac` files as structured comments.** Rejected. Canonical form has no comments ([ADR 0006](0006-canonical-lexical-rules.md) § lexical rules); introducing them would be a canonical-format change, i.e., a spec bug per ADR 0013's freeze discipline. Keeping metadata physically separate also keeps the source-of-truth / advisory distinction clear.
- **One sidecar per hash in an object store.** Rejected for Phase 0. The object store is explicitly deferred in [tacit-plan.md § Object store (deferred)](../plans/tacit-plan.md). Phase 0's deliverable is a single-project, single-file sidecar.
- **Require strict staleness (refuse to render if hash mismatches).** Rejected. Advisory sidecars that fail-hard on the most common drift condition (someone edited the `.tac`, no one rewrote the `.tacd`) are not actually advisory. Warning-and-best-effort is the correct failure mode.

## Consequences

- **Authoring AI gets a mechanical emission rule.** Emit the canonical tree, emit the parallel sidecar. No hashing, no pathing, no bookkeeping beyond the names already in scope.
- **Inspection view always renders.** A `.tac` file with a missing or stale sidecar still produces readable output via synthetic names. This keeps inspection-view tooling robust against partial workflows (grabbing a `.tac` off disk without its companion).
- **Sidecar is not hashed.** Deliberate. Content-addressing is for semantics; display is advisory. Two AIs who independently pick different names for the same program produce different `.tacd` files but the same `.tac` and the same hash.
- **Record field ordering needs a permutation.** `field_order` is the only place the sidecar encodes a non-structural fact (original user order vs. canonical alphabetical). This is a concession to [ADR 0008](0008-record-field-ordering.md)'s "canonical reorders user input" choice — authoring-view round-trip needs the original order back.
- **Sidecars don't preserve identifier case or Unicode for fields/ctors.** Symbol bytes live in canonical form already ([ADR 0006](0006-canonical-lexical-rules.md)); only things the canonical form strips go in the sidecar.
- **Phase 1+ extension path is additive.** New metadata keys can be added without a format version bump as long as readers ignore unknown keys. A format version bump is reserved for shape changes (e.g., if a content-hash overlay layer is introduced).
- **Stale sidecars are non-fatal.** A reader surfaces a warning and proceeds with whatever metadata matches tree shape; positions that no longer align fall back to synthetic names. This means diff tools and code review tooling can tolerate in-flight state where one file has been saved and the other has not.

## Related decisions

- [ADR 0003](0003-authoring-view-bpe-compact.md) — authoring view depends on the sidecar for identifier resolution and field reordering.
- ADR 0015 (pending) — inspection view scope; will cite this sidecar as its name source.
- [ADR 0008](0008-record-field-ordering.md) — the reason `field_order` exists.
- [ADR 0009](0009-hashing-rule.md) — reason the sidecar does not hash (BLAKE3 is over canonical text only).
- [tacit-plan.md § Storage format](../plans/tacit-plan.md) — pinned the `.tacd` file extension; this ADR formalizes the format.
- [phase-0-plan.md § Stage 3](../plans/phase-0-plan.md) — the Stage 3 deliverable this resolves.
