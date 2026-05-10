# 0080 - Phase 6 module imports, exports, visibility, and hash semantics

**Status:** Accepted
**Date:** 2026-05-10
**Phase:** 6, Stage 1
**Closes:** [phase-6-plan.md Q-P6-1](../plans/phase-6-plan.md),
[phase-6-plan.md Q-P6-2](../plans/phase-6-plan.md),
[phase-6-plan.md Q-P6-3](../plans/phase-6-plan.md)
**Amends:** [ADR 0013](0013-canonical-text-format-frozen.md) - additive
extension; [ADR 0014](0014-sidecar-format.md) - additive metadata extension

## Context

Phase 6 starts the move from one Tacit program at a time to packages of
composable Tacit definitions. The first dependency is Tacit-to-Tacit module
semantics: a definition in one logical module must be able to refer to a
definition from another logical module by content hash, and the checker must
verify that the imported value has the declared type and effect signature.

The existing canonical `module` node is a simultaneous binding group. It has
no import table, export table, visibility metadata, or stable way to name a
single exported binding independently from sibling order. Reusing it directly
for Phase 6 modules would make file or author order semantically important and
would make content hashes depend on display names or DeBruijn positions. That
conflicts with the Phase 6 goal: modules must be friendly to LLM authors and
machine tools by making boundaries explicit, hash-based, and independent of
file layout.

The design is also constrained by frozen commitments:

- Canonical text is the hashed source of truth; names and comments live in
  `.tacd` sidecars.
- Existing canonical tags are additive-only; no existing tag may be
  re-purposed.
- Type and effect annotations at module boundaries are explicit.
- `rec` remains the unit of lexical mutual recursion and hashes as one atom.
- General FFI is out of scope. Host imports/exports later consume these module
  boundaries but do not broaden them.

## Decision

Phase 6 introduces a new canonical logical-module artifact, `unit`, plus
definition, signature, export, import, and hash-reference nodes. A `unit`
contains a sorted import table, a sorted export table, and a sorted list of
definition artifacts. Definition hashes identify definition content; visibility
is module-interface metadata and is not part of the definition hash.

### Canonical node kinds

The following rows are appended to the canonical node table.

| Tag | Arity | Children | Notes |
| --- | --- | --- | --- |
| `unit` | 3 | imports, exports, defs | Phase 6 logical module artifact. |
| `imports` | N >= 0 | imp_0, ..., imp_n | Import declarations, sorted by hash bytes. |
| `imp` | 2 | hash-str, sig | Declares one imported definition hash and expected signature. |
| `exports` | N >= 0 | exp_0, ..., exp_n | Export declarations, sorted by hash bytes. |
| `exp` | 2 | visibility-sym, hash-str | Exports a local definition hash as `public` or `package`. |
| `defs` | N >= 1 | def_0, ..., def_n | Local definition artifacts, sorted by definition hash bytes. |
| `def` | 2 | sig, body | Content-addressed definition artifact. |
| `sig` | 2 | type-node, eval-eff-set | Boundary signature: value type plus definition-evaluation effects. |
| `ref` | 1 | hash-str | Value-level reference to a local or imported definition hash. |

`hash-str` is a canonical string containing exactly 64 lowercase hexadecimal
characters. BLAKE3 remains the only hash algorithm, so the canonical string
does not carry an algorithm prefix. Authoring and inspection views render the
same value as `blake3:<hex>` for readability.

`eval-eff-set` is a concrete `eff-set`. Function call effects remain inside
`fn-ty` nodes as specified by ADRs 0034-0036. Most definitions have pure
evaluation effects, represented canonically as `(eff-set)`.

The existing `(module ...)` node remains valid for the frozen single-file
surface. It is not re-purposed as the Phase 6 module artifact.

### Definition identity

The content hash of a definition is:

```text
definition_hash = BLAKE3(canonical_text((def SIG BODY)))
```

The hash includes:

- the declared boundary signature,
- the definition body,
- every `ref` hash used by that body,
- any lexical `let`, `lam`, `rec`, record, pattern, or type structure inside
  the definition.

The hash excludes:

- display aliases,
- comments,
- file paths,
- authoring order,
- whether the definition is exported as `public`, exported as `package`, or
  kept private inside its owning module.

Changing visibility changes the containing `unit` hash and package interface,
but not the definition hash. Changing the body, signature, or dependency hashes
changes the definition hash.

### Versioning and compatibility

Definitions and units are immutable content-addressed artifacts.

- A new definition version is a new `def` hash. Any change to the declared
  signature, body, or referenced dependency hashes creates a different
  definition hash.
- A new unit version is a new `unit` hash. Any change to imports, exports,
  visibility, local definitions, or definition set creates a different unit
  hash.
- Existing artifacts are never updated in place. Package caches and lockfiles
  preserve the exact hashes consumed by existing dependents.
- Dependents import exact definition hashes, so upgrades are explicit: a
  consumer moves from one version to another by changing the imported hash and
  rechecking the declared signature.

Backward compatibility is therefore structural, not name-based. A new unit may
add exports without invalidating old dependents, because old dependents still
refer to the old exported definition hashes. If a definition changes, old
dependents continue to use the previous hash until they opt in to the new one.

Forward compatibility is conservative. A tool that encounters canonical unit
tags, visibility atoms, signature shapes, or metadata rules it does not
understand must reject the artifact with an unsupported-format diagnostic
rather than silently treating it as an older unit. Content-addressed code must
not degrade through best-effort interpretation of unknown semantics.

Semantic-version ranges are not part of `unit`. Human release labels,
compatibility policies, and upgrade selection belong to the later package
manifest and lockfile ADR. The unit layer only says what artifact is being
used, exactly.

### Ordering

To preserve the rule that file layout and authoring order have no semantic
weight:

- `imports` entries are sorted by imported hash bytes.
- `exports` entries are sorted by exported hash bytes.
- `defs` entries are sorted by each `def` node's computed definition hash.
- Duplicate import hashes are rejected.
- Duplicate export hashes are rejected. A definition cannot be exported both
  `public` and `package` from the same `unit`.

Authoring order is sidecar metadata only.

### Authoring view

The Phase 6 authoring view uses explicit boundary declarations:

```tacit
module Math {
  import increment : Int -> Int from blake3:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef;

  private double : Int -> Int =
    lambda x. x + x;

  export package normalize : Int -> Int =
    lambda x. double x;

  export public add_two : Int -> Int =
    lambda x. increment (increment x);
}
```

Rules:

- `module Alias { ... }` gives the logical module a display alias only.
- `import alias : type-sig from blake3:<64-hex>;` declares an imported
  definition and its expected signature.
- `export public alias : type-sig = expr;` exports a definition outside the
  package.
- `export package alias : type-sig = expr;` exports a definition only to
  modules in the same package.
- `private alias : type-sig = expr;` creates a module-private definition that
  other definitions in the same logical module may reference.
- Every import, public export, package export, and private top-level
  definition has an explicit type signature. Function call effects are written
  in the function type. Pure definition-evaluation effects lower to
  `(eff-set)`.
- References to top-level imports or definitions lower to `(ref "<hash>")`.
  Lexical variables inside `lambda`, `let`, `rec`, and patterns continue to use
  DeBruijn `var`.

The authoring view may elide a pure definition-evaluation effect, but the
canonical `sig` always contains it.

### Visibility

There are three visibility levels.

`public` definitions:

- appear in the `exports` table as `(exp public "<hash>")`,
- may be imported from dependent packages,
- may later be considered for host-interface exports if their types are
  ABI-expressible.

`package` definitions:

- appear in the `exports` table as `(exp package "<hash>")`,
- may be imported only by modules in the same package,
- are not part of the package's external public interface.

`private` definitions:

- do not appear in the `exports` table,
- may be referenced only by definitions in the same logical module,
- may still be stored and cached by hash as part of the dependency closure of a
  public or package definition.

Visibility controls who may name a definition at a module boundary. It does
not alter the definition's content hash.

### Import resolution and type/effect checking

The checker builds a definition environment from:

1. local `def` entries in the current `unit`,
2. declared `imp` entries,
3. package/cache objects supplied by later Phase 6 package resolution.

For each local `def`, the checker verifies that the body type and
definition-evaluation effects are compatible with its `sig`. For each `ref`,
the checker resolves the hash to either a local definition or a declared
import. The resolved signature becomes the type and effect information for the
reference.

For each `imp`, the checker compares the declared `sig` with the signature of
the resolved definition artifact. Because signatures are canonical AST
fragments with DeBruijn type/effect variables, byte-identical canonical
signature text is the equality check after canonical emission. A missing
provider object may still let downstream inference continue using the declared
import signature, but it is an error.

Visibility is checked at resolution time:

- external packages may import only `public` definitions,
- modules in the same package may import `public` or `package` definitions,
- no module may import another module's `private` definition,
- a module may reference its own private definitions through local `def`
  entries.

### Mutual recursion and cycles

Mutual recursion does not cross module boundaries in Phase 6. A definition may
contain a lexical `rec`; that entire `rec` is part of the enclosing `def` and
therefore part of one definition hash.

The graph of `def` artifacts connected by `ref` hashes must be acyclic. A
cycle among definition hashes is a module/package dependency cycle and is
reported as an error. Authors who need mutually recursive functions must place
the recursive group inside one definition artifact, usually with `rec`.

This rule keeps imported hashes meaningful: every `ref` points to an already
content-addressed definition, never to a future member of a cross-module
recursive knot.

### Sidecar aliases

Aliases are advisory display metadata in `.tacd`, not canonical content.
ADR 0014 is extended with optional top-level module metadata:

```json
{
  "module_alias": "Math",
  "definition_aliases": {
    "<64-hex-definition-hash>": "add_two"
  },
  "import_aliases": {
    "<64-hex-import-hash>": "increment"
  },
  "export_aliases": {
    "<64-hex-exported-definition-hash>": "add_two"
  }
}
```

Readers ignore these keys if absent or stale. A fresh authoring sidecar must
avoid duplicate aliases in the value namespace of one logical module. If a
stale or hand-written sidecar provides duplicates, renderers fall back to
hash-based synthetic names for the ambiguous entries.

### Diagnostics

Stage 1 reserves the following structured diagnostic kinds for module-boundary
checking:

| Kind | Severity | Meaning |
| --- | --- | --- |
| `missing-import` | error | A `ref` or `imp` hash cannot be resolved from local definitions, declared imports, or the package/cache environment. |
| `hash-mismatch` | error | An artifact supplied for a hash key does not hash to that key. |
| `signature-mismatch` | error | An import signature, export signature, or definition body does not match the resolved/declared signature. |
| `visibility-violation` | error | A module imports a `package` definition from outside the package or any definition marked private to another module. |
| `cyclic-dependency` | error | The graph of definition hashes contains a cycle outside a lexical `rec`. |
| `duplicate-import` | error | A `unit` declares the same import hash more than once. |
| `duplicate-export` | error | A `unit` exports the same definition hash more than once. |
| `dangling-export` | error | An `exp` entry names a hash that is not a local `def`. |

Diagnostics should include both the display alias, if available, and the
`blake3:<hash>` value. That is intentionally redundant: aliases are what LLMs
and humans use to navigate, while hashes are the stable repair target.

### Inspection view

Inspection output renders module boundaries before definition bodies:

```text
module Math
imports
  increment : Int -> Int = blake3:01234567...
exports
  public add_two : Int -> Int = blake3:89abcdef...
  package normalize : Int -> Int = blake3:456789ab...
private
  double : Int -> Int = blake3:cafebabe...
definitions
  add_two =
    lambda x.
      increment (increment x)
```

Default inspection rendering shows hash prefixes in import/export/private
tables. `--hashes` shows full hashes and may add per-node hash overlays as
already established by ADR 0015. A `ref` inside a body renders as the sidecar
alias when available, with hash detail visible in the boundary table or under
`--hashes`.

### LLM-facing design constraints

The module surface intentionally favors explicit, repairable structure over
implicit convenience:

- Boundary declarations use ordinary words (`import`, `export public`,
  `export package`, `private`) rather than punctuation-heavy modifiers.
- Hashes appear at import/export boundaries, not hidden in project files.
- Names are aliases only, so an LLM may rename for clarity without changing
  definition identity.
- Diagnostics include both aliases and hashes, giving models a readable handle
  and an exact repair target.
- Cross-module cycles are rejected instead of represented with placeholder
  hashes or hidden fixpoints.
- File paths and authoring order are never part of module meaning.

### Test-vector expectations

Stage 1 commits these future vector classes. The implementation stages add the
actual fixture files.

Canonical vectors:

- a `unit` with one public export and no imports,
- a `unit` with one import and one public export whose body uses `ref`,
- sorted `imports`, `exports`, and `defs` independent of authoring order,
- a private helper referenced by a public export,
- a lexical `rec` inside a single exported `def`.

Authoring vectors:

- `import ... from blake3:<hash>`,
- `export public`,
- `export package`,
- `private`,
- module alias preserved only in sidecar metadata.

Sidecar vectors:

- fresh alias metadata for module, import, export, and private definition,
- stale alias metadata falling back to hash-based synthetic names,
- alias-only edits leaving all definition hashes unchanged.

Diagnostic vectors:

- missing import,
- hash mismatch,
- signature mismatch,
- visibility violation for package export from another package,
- visibility violation for private definition from another module,
- cyclic dependency through `ref`,
- dangling export.

## Alternatives considered

### Reuse the existing `module` node

Rejected. The existing node is a simultaneous DeBruijn binding group. Binding
position is semantically meaningful, all bindings are peers, and there is no
place for import/export metadata. Retrofitting Phase 6 semantics onto it would
make definition identity depend on authoring order and would blur the frozen
meaning of `module`.

### Name-based imports

Rejected. `import Math.add_two` is convenient for humans but makes names and
project layout semantic. Tacit's package model is hash-based; authoring aliases
can sit on top, but canonical imports must be hash references.

### Put aliases in canonical text

Rejected. Names are intentionally sidecar metadata. Hashes should survive
renaming, and LLM-generated repairs should be able to rename for clarity
without moving content addresses.

### Include visibility in the definition hash

Rejected. Visibility is interface policy, not definition content. If a pure
definition is promoted from package-local to public, dependents should see a
package-interface change, not a new definition body. The containing `unit` and
later package hashes capture visibility changes.

### Allow cross-module mutual recursion

Rejected for Phase 6. Cross-module recursive knots require either placeholder
hashes, group hashes with member selectors, or a fixpoint object model. All
three make imported-hash diagnostics harder and are hostile to the simple
LLM-facing model: a `ref` should point to a complete definition. Lexical `rec`
inside one definition remains available.

### Permit direct re-export of an imported hash

Rejected for Stage 1. Re-export policy interacts with package manifests,
lockfiles, provenance, and public API curation. A module can wrap an imported
definition in a local exported definition if needed. Direct re-export can be
reopened by the package ADR.

## Consequences

- Phase 6 implementation adds canonical parsing and emission for `unit`,
  `imports`, `imp`, `exports`, `exp`, `defs`, `def`, `sig`, and `ref`.
- The checker gains a module-resolution pass that maps `ref` hashes to
  signatures before ordinary expression inference.
- Definition hashes become the unit consumed by the Stage 3 package cache and
  object store.
- Unit and definition versioning is hash-exact. New code versions create new
  hashes; old dependents remain pinned to old hashes until explicitly upgraded.
- The Stage 2 whole-project graph can load logical modules from any file
  layout, canonicalize them into sorted `unit` artifacts, and ignore path
  ordering.
- The Stage 10 host-interface ABI can build on `public` exports without
  inventing a separate boundary concept.
- Existing single-program and legacy `(module ...)` programs remain valid.
  Migration to `unit` is additive.
- No Phase 6 work may use `corpus/sealed/` contents, paths, metadata, or
  feedback to validate this design.

## Related decisions

- [ADR 0013](0013-canonical-text-format-frozen.md) - canonical format,
  amended additively by this ADR.
- [ADR 0014](0014-sidecar-format.md) - sidecar metadata, extended with
  optional module alias maps.
- [ADR 0015](0015-inspection-view-scope.md) - inspection rendering model.
- [ADR 0034](0034-p2-type-subset-ann.md) - canonical type expressions.
- [ADR 0035](0035-p2-effect-set-canonical.md) - concrete effect sets.
- [ADR 0036](0036-p2-effect-polymorphism-syntax.md) - effect variables in
  function types.
- [ADR 0039](0039-p2-module-authoring-syntax.md) - legacy top-level module
  authoring syntax.
- [ADR 0071](0071-storage-format-reconciliation.md) - `.tac` canonical
  storage and `.tacd` sidecar roles.
- [ADR 0079](0079-phase-6-scope.md) - Phase 6 scope and required ADR
  sequence.
