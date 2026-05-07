# 0041 — Phase 2 structured error format: JSON schema for type and effect diagnostics

**Status:** Accepted
**Date:** 2026-04-27
**Phase:** 2, Stage 1
**Closes:** [phase-2-plan.md Q-P2-8](../plans/phase-2-plan.md)
**Amended by:** [ADR 0073](0073-p4-function-values-and-closures.md)

## Context

Phase 1's error reporting is unstructured: `ParseError` and `CodegenError`
are Rust enums whose `Display` implementations produce human-readable
strings. This works for a CLI, but the parent plan commits to "errors are
structured data" — a machine-readable format that editors, LSPs, and AI
tools can consume.

Phase 2 introduces two new error domains: type errors and effect errors.
These interact with hole diagnostics ([ADR 0040](0040-p2-hole-recovery.md))
and must share a common schema so a single consumer (editor, `tacit check`,
test runner) can handle all of them.

The parent plan also adds a `tacit check <input.tac>` subcommand that runs
typecheck only and emits structured diagnostics. The format this subcommand
emits is defined by this ADR.

Phase 2 must produce structured diagnostics for:
- Every `TypeError` variant.
- Every `EffectError` variant.
- `Hole` diagnostics forwarded from parser recovery ([ADR 0040](0040-p2-hole-recovery.md)).

## Decision

**Diagnostics are emitted as a JSON object. A formal JSON Schema is
delivered in Stage 5 (`docs/error-format.schema.json`). This ADR defines
the structure and all error kind names.**

### Top-level object

```json
{
  "schema_version": "p2.0",
  "errors": [ …error objects… ]
}
```

`schema_version` is a string identifying the Phase 2 format. Future phases
bump this string. Consumers must check the version before interpreting the
remainder.

### Error object

```json
{
  "kind":      "<string — error kind name>",
  "severity":  "error" | "warning",
  "location":  { … } | null,
  "message":   "<human-readable string>",
  "expected":  { … } | null,
  "actual":    { … } | null,
  "fix":       { … } | null,
  "related":   [ … ]
}
```

All fields are present in every error object. Absence of semantic content
uses `null`, not field omission.

### `location` object

```json
{
  "ast_path":    [ … step objects … ],
  "source_span": { "start": N, "end": N } | null
}
```

- `ast_path`: path from the AST root to the node in error, as a list of
  step objects (see below). Empty list = the error is at the root.
- `source_span`: byte-offset range in the authoring-view source file
  (half-open: `start` inclusive, `end` exclusive). `null` if the
  source file is not available or the error is purely in canonical form.

**AST path step:**
```json
{ "child": 0 }
```
Each step is the zero-indexed child position at the current node. Steps
are applied left-to-right from the root. A step list `[1, 0]` means: the
second child of the root, then the first child of that. Tags are not
included in the path (they are for human debugging, not routing).

### `expected` / `actual` objects: type and effect representations

Types and effect sets appear in `expected` and `actual` as JSON objects
that mirror the canonical form:

| Canonical form              | JSON representation                                     |
|-----------------------------|---------------------------------------------------------|
| `(sym T)`                   | `{"sym": "T"}`                                          |
| `(ty-var N)`                | `{"ty-var": N}`                                         |
| `(fn-ty arg ret eff)`       | `{"fn-ty": {"arg": …, "ret": …, "eff": …}}`            |
| `(eff-set A B …)`           | `{"eff-set": ["A", "B", …]}`  (atoms sorted)           |
| `(eff-var N)`               | `{"eff-var": N}`                                        |
| `(forall N M body)`         | `{"forall": {"ty": N, "eff": M, "body": …}}`           |
| `(record f₀ t₀ f₁ t₁ …)`   | `{"record": [["f₀", …], ["f₁", …]]}`  (pairs sorted)  |
| `(app f a)`                 | `{"app": {"fn": …, "arg": …}}`                         |

For type mismatches, `expected` is the annotated or required type and
`actual` is the inferred type. For effect errors, `expected` is the
declared effect set and `actual` is the inferred set. For hole diagnostics,
both are `null`.

### `fix` object

```json
{
  "description": "<human-readable suggestion>",
  "edits": [
    { "location": { … }, "replacement": "<authoring-view text>" }
  ]
}
```

`fix` is `null` if no structured fix is available. Phase 2 emits fixes for:
- Missing module-boundary annotation: suggests the inferred signature.
- Wrong effect set: suggests the corrected effect annotation.
- `null` for all other error kinds in Phase 2.

`edits` is a list of (location, replacement text) pairs where the
replacement is in authoring-view surface syntax. The `location` refers to
a `source_span` (byte offsets in the authoring file). An editor applies
edits non-overlappingly to produce the corrected source.

### `related` list

A list of secondary error objects (same structure, may be nested one level).
Used for:
- The definition site of a type variable when an out-of-scope reference
  is reported.
- The other arm of a `match` when arm types do not agree.
- `[]` (empty) if there are no secondary locations.

### Error kind names

**Type errors (`tacit-typecheck` crate):**

| Kind                        | Severity | Meaning                                              |
|-----------------------------|----------|------------------------------------------------------|
| `type-mismatch`             | error    | Expression type T ≠ required type U.                |
| `unbound-type-variable`     | error    | `(ty-var N)` has no enclosing `forall` with N < TY-COUNT. |
| `type-arity-mismatch`       | error    | Type constructor applied to wrong number of args.   |
| `unresolved-type`           | error    | `(sym T)` where T is not a known type or type ctor. |
| `module-missing-annotation` | warning  | Exported binding has no type+effect signature.      |
| `operator-overload-failure` | error    | Operator applied to operands with incompatible types.|
| `buf-escape`                | error    | Buffer handle used outside its `let` scope.         |
| `apply-non-function`        | error    | `app` function position has a non-function type.    |
| `invalid-capture`           | error    | Closure capture set includes a non-capturable value.|

**Effect errors (`tacit-typecheck` crate):**

| Kind                    | Severity | Meaning                                                   |
|-------------------------|----------|-----------------------------------------------------------|
| `effect-violation`      | error    | Inferred effect set E ⊄ declared set F at a boundary.    |
| `unbound-effect-variable` | error  | `(eff-var N)` has no enclosing `forall` with N < EFF-COUNT. |

**Hole diagnostics (forwarded from parser):**

| Kind                | Severity | Meaning                                               |
|---------------------|----------|-------------------------------------------------------|
| `hole-diagnostic`   | error    | Forwarded from a `Hole` node. `message` carries the  |
|                     |          | hole's payload string. `expected`/`actual` are null. |

For `hole-diagnostic`, the `kind` field carries the diag-id from the hole
(`unexpected-token`, `type-parse-error`, etc.) rather than `hole-diagnostic`.
In other words: the hole's diag-id **is** the error kind. This way a consumer
can route on diag-id directly without an extra lookup.

### Output channels

- `tacit check --format json` writes the JSON object to stdout.
- `tacit check --format text` (default) writes human-readable diagnostics
  to stderr, one per line, in the format `filename:line:col: [kind] message`.
- `tacit compile` always writes human-readable diagnostics to stderr;
  structured JSON is not emitted during compile in Phase 2 (this may change
  in Phase 3).

Exit codes for `tacit check`:
- `0`: no errors (warnings are permitted).
- `1`: one or more errors.
- `2`: internal error (e.g., I/O failure, panic).

Exit codes are separate from `tacit compile`'s exit codes, which also
include codegen failures (exit code `3`).

### Negative-test corpus

Stage 2's exit gate requires at least one negative-test case per error kind.
The corpus lives under `tests/negative/` in the `tacit-typecheck` crate.
Each case is a `.tac` file whose expected JSON output is stored alongside
it (`.expected.json`). The test runner runs `tacit-typecheck::check` on
the `.tac` input and compares the JSON output to the expected file, ignoring
`message` text (human-readable messages may vary; only `kind`, `severity`,
`location.ast_path`, `expected`, and `actual` are compared).

## Alternatives considered

- **Plain text error format; structured format deferred.** Phase 2 already
  needs structured output for `tacit check --format json`. Deferring means
  two format implementations later. Rejected: design the format once when
  the domain is clear.

- **Use the Language Server Protocol diagnostic format.** LSP diagnostics
  are a standard used by many editors. But the LSP format is coupled to
  document-URI and protocol message framing; using it for a standalone CLI
  tool introduces unnecessary dependencies and makes the format harder to
  read in a terminal. The fields we need (kind, location, expected/actual,
  fix) are a proper subset of LSP's diagnostic, so a future LSP adapter can
  translate from this format. Rejected for CLI use; accepted as a future
  bridge.

- **AST path as a sequence of tag+index pairs.** `[{"tag": "fn-ty", "child": 2}]`
  instead of `[{"child": 2}]`. More readable but brittle: if a pass rewrites
  the AST (e.g., eta-expands a type), the tag at a given child position may
  change. The integer path is stable as long as the arity of each intermediate
  node is unchanged. Rejected: tags in the path.

- **Line/column addressing instead of byte offsets.** More human-friendly
  but requires the consumer to know the file's encoding and line endings.
  Byte offsets are unambiguous. Editors that need line/column can convert.
  Accepted: byte offsets.

- **Separate JSON objects for type errors and effect errors.** Two separate
  arrays in the top-level object. Rejected: the single `errors` array with
  a `kind` discriminant is easier to consume and covers hole diagnostics
  naturally.

## Consequences

- `tacit-typecheck` emits `Vec<Diagnostic>` where `Diagnostic` is a Rust
  struct matching this schema. A `to_json()` method serializes to the
  format defined here.
- `tacit-cli` gains `tacit check <input.tac> [--format json|text]` in Stage 5,
  consuming `Vec<Diagnostic>` from the typechecker.
- The formal JSON Schema (`docs/error-format.schema.json`) is delivered in
  Stage 5; the schema validates all outputs from `tacit check --format json`.
- The negative-test corpus (`tests/negative/` in `tacit-typecheck`) is
  populated in Stage 2, one case per error kind above.
- Phase 1's unstructured `ParseError` and `CodegenError` are not
  replaced in Phase 2; they remain as-is. Phase 3+ may wrap them in the
  same schema if unification is needed.

## Related decisions

- [ADR 0040](0040-p2-hole-recovery.md) — hole diagnostics forwarded via
  this format.
- [ADR 0042](0042-p2-operator-overload.md) — `operator-overload-failure`
  error kind.
- [ADR 0038](0038-p2-writable-buffer.md) — `buf-escape` error kind.
- [phase-2-plan.md Q-P2-8](../plans/phase-2-plan.md) — closed.
