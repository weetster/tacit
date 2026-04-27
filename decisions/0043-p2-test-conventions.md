# 0043 — Phase 2 test conventions for typed programs

**Status:** Accepted
**Date:** 2026-04-27
**Phase:** 2, Stage 1
**Closes:** [phase-2-plan.md Q-P2-10](../plans/phase-2-plan.md); parent plan Q4 (Phase 2 portion)

## Context

Phase 1's smoke corpus test harness verifies stdout and exit code for each
program. [ADR 0032](0032-stage-4-frozen.md) established the gate: seven
programs, checked by `cargo test -p tacit-codegen --features llvm19-1`.

Phase 2 adds a type and effect checker. The smoke corpus programs will each
have inferred types and effect sets. The question ([phase-2-plan.md Q-P2-10](../plans/phase-2-plan.md)):

> Whether the smoke corpus carries per-program type/effect expectations
> alongside stdout/exit-code expectations.

The answer must satisfy two competing concerns:
1. **Regression safety**: the typechecker should be tested against concrete
   expected outputs, not just "runs without error." If the typechecker
   infers the wrong type for `factorial`, the test should catch it.
2. **Maintenance cost**: type annotations in test fixtures can drift as
   the type system evolves. Annotations should be easy to update and
   ideally machine-verifiable.

A third concern: where do the expectations live? Inline in the `.tac` file
(as comments), in a sidecar, in a separate file, or in the test runner
itself?

## Decision

**Type and effect expectations for smoke programs are stored in the
existing `.tac.sidecar.toml` files as a new optional `[types]` table.
The format is a map from binding name to type annotation string and effect
atom list. Missing `[types]` tables generate a `module-missing-annotation`
warning during testing, not a hard failure, until Stage 2 populates them.**

### Sidecar extension

The sidecar format ([ADR 0014](0014-sidecar-format.md)) is extended with
an optional `[types]` table. Each key is a binding name visible in the
authoring view (the name, not the DeBruijn index). Example for
`factorial.tac`:

```toml
[types.factorial]
type = "Int -> Int"
effects = []
```

For programs without a `module` top-level, the main expression's type and
effects are stored under the key `"main"`:

```toml
[types.main]
type = "Int"
effects = []
```

For `hello.tac`, which calls `@write` (IO effect) and returns 0:

```toml
[types.main]
type = "Int"
effects = ["IO"]
```

`type` is an authoring-view type string (the same surface syntax used in
module binding annotations, per [ADR 0039](0039-p2-module-authoring-syntax.md)).
`effects` is a sorted list of effect atom strings from the lattice
({Alloc, Div, IO, Mut}).

### What the test runner checks

The `tacit-typecheck` crate exposes a function:

```rust
pub fn check_against_sidecar(
    ast: &Node,
    sidecar: &Sidecar,
) -> Result<(), Vec<Diagnostic>>
```

If the sidecar has a `[types]` table, `check_against_sidecar` parses each
type string, constructs the corresponding canonical type expression, and
compares against the typechecker's inferred type. Mismatches produce
`type-mismatch` diagnostics. If the sidecar has no `[types]` table, the
function returns `Ok(())` and a `module-missing-annotation` warning is
logged (not returned as an error in Phase 2).

The existing test runner in `tacit-codegen` runs `check_against_sidecar`
after successful typecheck in Stage 2. A new `cargo test -p tacit-typecheck`
step is added to CI in Stage 2.

### Population schedule

Stage 2 typechecks all seven Phase 1 smoke programs and populates their
`[types]` tables as it validates each program. Populating is a Stage 2
deliverable (not Stage 1). Stage 1 only establishes the format.

Expected types for the Phase 1 smoke corpus (provisional — Stage 2 confirms):

| Program         | Binding   | `type`         | `effects`       |
|-----------------|-----------|----------------|-----------------|
| `return-zero`   | `main`    | `"Int"`        | `[]`            |
| `return-computed`| `main`   | `"Int"`        | `[]`            |
| `hello`         | `main`    | `"Int"`        | `["IO"]`        |
| `if-branch`     | `main`    | `"Int"`        | `[]`            |
| `factorial`     | `factorial`| `"Int -> Int"` | `[]`            |
| `even-odd`      | `even`    | `"Int -> Int"` | `["Div"]`       |
| `even-odd`      | `odd`     | `"Int -> Int"` | `["Div"]`       |
| `exit-nonzero`  | `main`    | `"Int"`        | `["IO"]`        |

(Phase 2 also adds smoke #7 and #8 to the corpus; their `[types]` entries
are populated when they are implemented in Stage 4.)

### Negative-test corpus (typecheck-specific)

In addition to the smoke corpus, `tacit-typecheck` maintains a negative-
test corpus under `tests/negative/` in the crate. Each case is a `.tac`
file plus a `.expected.json` file containing the expected JSON diagnostics.
The test runner runs `tacit-typecheck::check` and compares the JSON output
to the expected, ignoring the `message` field (per [ADR 0041](0041-p2-structured-error-format.md)).

At Stage 2 exit, the corpus must contain at least one case per error kind
enumerated in [ADR 0041](0041-p2-structured-error-format.md).

### Update discipline

When the type system evolves in Phase 3+, sidecar `[types]` values may
need updating. The process:
1. Run `tacit check --format json examples/smoke/foo.tac` to get the new
   inferred type.
2. Update `foo.tac.sidecar.toml`'s `[types]` table.
3. Commit alongside the spec or implementation change that caused the drift.

There is no automatic update script in Phase 2. If the corpus grows large
enough to make manual updates costly, Phase 3+ may add a `tacit check --update-sidecar`
flag. This ADR does not anticipate that flag; it is mentioned only to note
the upgrade path.

## Alternatives considered

- **Inline type annotations as comments in `.tac` files.** E.g.,
  `-- @type: Int -> Int`. Comments are stripped before parsing; the
  authoring parser would need a special convention to extract them. The
  sidecar is already the established channel for advisory metadata that
  is not part of the AST. Rejected: sidecar is the right home.

- **Separate `.types.toml` file per program.** One extra file per smoke
  program, not embedded in the sidecar. Simpler schema, but proliferates
  files (each program already has a `.tac` and a `.tac.sidecar.toml`; a
  third file increases the surface). The `[types]` table in the sidecar
  is already at the right granularity. Rejected.

- **Types expectations in the test runner (Rust code).** Hard-coded in
  the test struct: `assert_type("factorial", "Int -> Int")`. Requires
  a recompile to update, is not accessible to non-Rust consumers, and
  couples the type-expectation format to Rust string syntax. The sidecar
  TOML is a better data file. Rejected.

- **Require `[types]` immediately (hard-fail on missing).** Would block
  Stage 2 from running any smoke tests until all seven `[types]` tables
  are populated. Stage 2's exit gate is that all programs typecheck; the
  tables can be populated iteratively as each program is verified. Using
  a warning-not-error for missing tables allows the typechecker to come
  online incrementally. Accepted.

- **Store types as canonical type expressions (JSON), not authoring-view
  strings.** Precise but unreadable. The authoring-view string `"Int -> Int"` is
  far more legible for human review than
  `{"fn-ty": {"arg": {"sym": "Int"}, "ret": {"sym": "Int"}, "eff": {"eff-set": []}}}`.
  The test runner parses the authoring string anyway; the TOML value is
  advisory/human-readable, not machine-canonical. Rejected for TOML;
  canonical form is in the error JSON output per ADR 0041.

## Consequences

- The sidecar format ([ADR 0014](0014-sidecar-format.md)) is extended with
  an optional `[types]` table. No breaking change to existing sidecars;
  absence of `[types]` is handled gracefully.
- Stage 2 populates `[types]` for the seven Phase 1 smoke programs as
  the typechecker validates them.
- CI gains a `cargo test -p tacit-typecheck` step in Stage 2. The step
  runs both positive (smoke corpus with `[types]`) and negative
  (`tests/negative/`) test cases.
- The format is forward-compatible: if Phase 3+ needs richer annotations
  (e.g., constraints, refinement predicates), additional keys can be added
  to each binding table without changing existing entries.

## Related decisions

- [ADR 0014](0014-sidecar-format.md) — sidecar format; extended with
  `[types]` table.
- [ADR 0032](0032-stage-4-frozen.md) — Stage 4 freeze; existing smoke
  corpus regression contract. Phase 2 extends, not replaces, it.
- [ADR 0041](0041-p2-structured-error-format.md) — JSON diagnostic format
  used by the negative-test corpus.
- [ADR 0039](0039-p2-module-authoring-syntax.md) — type-signature surface
  syntax used in the `type` string values.
- [phase-2-plan.md Q-P2-10](../plans/phase-2-plan.md) — closed.
