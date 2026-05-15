# 0083 - Phase 6 package tests and structured results

**Status:** Accepted
**Date:** 2026-05-15
**Phase:** 6, Stage 5 design
**Closes:** [phase-6-plan.md Q-P6-7](../plans/phase-6-plan.md)
**Amends:** [ADR 0082](0082-phase-6-package-manifest-lockfile-cache.md) -
additive manifest extension

## Context

ADR 0080 made definitions and units hash-addressed. ADR 0081 made a project
load a deterministic set of units, and ADR 0082 added package identity,
manifest metadata, lockfiles, and the local cache. This decision adds a
package-level test surface on top of those decisions.

The test surface has to satisfy several constraints:

- Tests must run against the same package graph that `tacit check` and
  `tacit compile` see.
- Tests must call Tacit definitions through the normal `unit` import/export
  rules. No test-only visibility escape should let one unit import another
  unit's private definitions.
- Test declarations need stable machine-readable identities for AI repair
  loops, but display names must remain advisory.
- Test results must be structured data, not scrape-only CLI text.
- This stage should not introduce a general assertion framework, property
  testing, dynamic plugin loading, arbitrary host FFI, or a second source
  discovery model.
- No design, implementation, or validation work may read, list, or otherwise
  depend on `corpus/sealed/`.

This ADR is the design artifact for package tests. Implementation work for
manifest parsing, CLI execution, result emission, fixtures, and CI coverage
follows it.

## Decision

### Test declaration model

Package tests are ordinary Tacit definitions listed from `tacit.toml`.

The manifest gains an optional array-of-tables named `[[tests]]`:

```toml
[[tests]]
name = "double_zero"
target = "blake3:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
effects = []

[[tests]]
name = "round_trip_file"
target = "blake3:89abcdef0123456789abcdef0123456789abcdef0123456789abcdef01234567"
effects = ["Alloc", "IO", "Mut"]
```

Rules:

- `name` is a required display alias. It is advisory and not part of package
  identity, but it must be unique within the manifest so test results have a
  stable human handle.
- `target` is required and must be a `blake3:<64-hex>` definition hash for a
  local definition in the package graph.
- `effects` is optional and defaults to `[]`. It is the effect set the test
  runner is allowed to execute for this target.
- `effects` entries are drawn from `Alloc`, `IO`, and `Mut`, sorted in the
  same order as ADR 0035. `Div` is not permitted for runnable package tests;
  a test target whose definition-evaluation effect includes `Div` produces an
  `effect-fail` result.
- Unknown keys inside `[[tests]]` are rejected with `manifest-unknown-field`,
  matching ADR 0082's strict-manifest rule.
- Duplicate test names are rejected with `duplicate-test-alias`.
- Duplicate test targets are rejected with `duplicate-test-target`.

The manifest is still not hashed for package identity. Adding, removing, or
renaming a `[[tests]]` entry changes test selection but not the package hash.
Changing the test definition body or signature changes the definition hash and
therefore the containing unit and package hashes, because tests are ordinary
package units.

### Test definition shape

A runnable test target is a zero-input definition whose value type is `Bool`.
The target's boundary signature is therefore:

```text
(sig (sym Bool) (eff-set ...))
```

Execution semantics:

- `true` means the test passes.
- `false` means the test fails.
- A runtime trap, host error, timeout, or runner failure produces an `error`
  result.
- A static failure before execution produces either `compile-fail` or
  `effect-fail`, as defined below.

This ADR does not add assertion syntax or assertion primitives. Source-level
helpers may later wrap boolean checks, but the runner's minimum contract is
only "evaluate this `Bool` definition".

Function-valued tests, table-driven tests, randomized tests, benchmarks,
snapshot tests, and property tests are out of scope for this test surface.

### Effects

The test runner enforces an explicit effect policy per target:

1. Resolve the target definition hash.
2. Read the target's declared definition-evaluation effect set from `sig`.
3. Reject the target as `effect-fail` if the declared set contains `Div`.
4. Reject the target as `effect-fail` if the declared set is not a subset of
   the manifest entry's `effects`.
5. Otherwise, compile and run the target.

The default `effects = []` keeps pure tests concise. Effectful tests must opt
in to their effects at the manifest boundary. This makes IO and mutation
visible in review and keeps the stable JSON result from depending on hidden
runner policy.

The policy covers definition-evaluation effects only. Function call effects
inside `fn-ty` continue to be checked by the existing type/effect checker when
the test definition body calls other functions.

### Visibility and package boundaries

Tests do not introduce a new canonical visibility level.

The test target itself may be `public`, `package`, or private to its owning
unit because the runner executes a local package definition, not an import from
another package. This does not let ordinary Tacit code name private definitions
across unit boundaries.

A test that calls definitions from other units or dependencies must do so
through normal `imp` / `ref` semantics:

- same-package tests may import `public` and `package` definitions,
- external package definitions must be `public`,
- another unit's private definitions remain inaccessible,
- imported signatures and hashes are checked exactly as in `tacit check`.

This makes the test runner a package-level entry selector, not a new module
system.

### CLI

The CLI adds:

```text
tacit test [ROOT] [--format text|json]
```

`ROOT` defaults to the current directory and is resolved as a package root by
the same rules as package-aware `check` and `compile`.

Execution order is deterministic:

1. Load the package graph, manifest, lockfile, dependencies, and cache objects.
2. Validate `[[tests]]` entries.
3. Sort test entries by target hash bytes, then by `name`.
4. For each test, run static validation, compile the test entry, execute it,
   and record one result object.

The runner emits one structured result envelope even when some tests fail. If
package loading fails before test entries can be resolved, the envelope has an
empty `results` array and package-level diagnostics.

Output channels:

- `--format json` writes the result envelope to stdout.
- `--format text` writes a human-readable summary to stdout and diagnostics to
  stderr. Text output is not a stable machine contract.

Exit codes:

- `0`: every discovered test has status `pass`.
- `1`: at least one test has status `fail`, and no test has status
  `compile-fail`, `effect-fail`, or `error`.
- `2`: at least one test has status `compile-fail`, `effect-fail`, or
  `error`, or package-level diagnostics prevent execution.
- `3`: internal runner error.

### Implementation-facing names

Commands, schema versions, file names, diagnostic kinds, Rust symbols, and
other implementation-facing identifiers must use product or domain names, not
phase, stage, or ADR numbers. This ADR therefore reserves `tacit-test-v1` for
the JSON schema version. Identifiers that encode phase numbers, stage numbers,
or decision-log numbers must not appear in emitted data or implementation
APIs.

### Structured result JSON

`tacit test --format json` emits this top-level shape:

```json
{
  "schema_version": "tacit-test-v1",
  "package": {
    "hash": "blake3:...",
    "name": "math"
  },
  "outcome": "pass",
  "summary": {
    "total": 2,
    "pass": 2,
    "fail": 0,
    "compile_fail": 0,
    "effect_fail": 0,
    "error": 0
  },
  "diagnostics": {
    "schema_version": "p2.0",
    "errors": []
  },
  "results": [
    {
      "name": "double_zero",
      "definition_hash": "blake3:0123...",
      "unit_hash": "blake3:4567...",
      "status": "pass",
      "declared_effects": [],
      "allowed_effects": [],
      "observed": {
        "bool": true
      },
      "diagnostics": {
        "schema_version": "p2.0",
        "errors": []
      }
    }
  ]
}
```

All fields are present. Missing semantic content is represented by `null` or
an empty array, not by omitting the field.

Field rules:

- `schema_version` is exactly `tacit-test-v1`.
- `package.hash` is the resolved package hash. It is `null` only if package
  loading fails before a hash can be computed.
- `package.name` is the manifest display name when present, otherwise `null`.
- `outcome` is `pass`, `fail`, or `error`.
- `summary` counters are derived from `results`.
- `diagnostics` is the package-level ADR 0041 diagnostic envelope.
- `results` is sorted by target hash bytes, then by `name`.
- `name` is the manifest test alias.
- `definition_hash` is the target hash from the manifest.
- `unit_hash` is the owning unit hash when resolution succeeds, otherwise
  `null`.
- `status` is one of `pass`, `fail`, `compile-fail`, `effect-fail`, or
  `error`.
- `declared_effects` is the target's definition-evaluation effect set, sorted
  as ADR 0035 effect atoms.
- `allowed_effects` is the manifest entry's effect set, sorted the same way.
- `observed.bool` is `true` for `pass`, `false` for `fail`, and `null` for
  static or runtime errors.
- `diagnostics` is the ADR 0041 diagnostic envelope for this test result.

No wall-clock durations, absolute paths, random seeds, raw stdout, raw stderr,
or host-specific executable paths appear in the stable JSON. Implementations
may add a separate opt-in profiling or trace mode later, but it must not
change `tacit-test-v1`.

### Result statuses

| Status | Meaning |
| --- | --- |
| `pass` | The target compiled, ran, and evaluated to `Bool true`. |
| `fail` | The target compiled, ran, and evaluated to `Bool false`. |
| `compile-fail` | The package or test target failed parse, typecheck, entry lowering, codegen, or link before execution. |
| `effect-fail` | The target's definition-evaluation effects are not permitted by the manifest entry, or include `Div`. |
| `error` | The target began execution or runner setup but hit a runtime trap, host error, timeout, cache corruption, or other non-compile infrastructure failure. |

`compile-fail` and `effect-fail` are result statuses, not successful negative
test expectations. This package test surface does not support "this test
should fail to compile" as a green test. A future conformance-test ADR may add
expected-negative fixtures without changing the ordinary package test
contract.

### Diagnostics

This ADR reserves these structured diagnostic kinds for package test handling:

| Kind | Severity | Meaning |
| --- | --- | --- |
| `duplicate-test-alias` | error | Two `[[tests]]` entries have the same `name`. |
| `duplicate-test-target` | error | Two `[[tests]]` entries have the same `target` definition hash. |
| `test-target-unresolved` | error | A `[[tests]]` target hash is not a local definition in the package graph. |
| `test-signature-mismatch` | error | A test target does not have value type `Bool`. |
| `test-effect-violation` | error | A test target declares effects outside the manifest entry's allowed set, or declares `Div`. |
| `test-compile-failure` | error | Test entry lowering, codegen, or link failed after package checking. |
| `test-runtime-error` | error | A compiled test trapped, timed out, failed through host execution, or could not be launched. |

Manifest syntax and schema problems that are not test-specific continue to use
ADR 0082 diagnostics such as `manifest-parse` and `manifest-unknown-field`.
Type and effect failures inside the package graph continue to use ADR 0041
and ADR 0080 diagnostics such as `type-mismatch`, `effect-violation`,
`missing-import`, and `visibility-violation`.

Diagnostics should include the test name when known, the target
`blake3:<hash>`, the owning unit hash when known, and the relevant package
hash. Display names are navigation aids; hashes are the stable repair target.

### Derived output

The runner may materialize build products and cached result envelopes under:

```text
<root>/.tacit/derived/project-<package-hash>/tests/
```

Reserved entries:

- `results.json` for the most recent `tacit-test-v1` JSON envelope,
- `build/` for intermediate test executables or objects,
- `logs/` for optional non-stable human trace output.

The dependency cache remains content-addressed and immutable. Test outputs,
executables, logs, and result envelopes are derived artifacts and must not be
stored under `.tacit/cache/objects/` or become part of package identity.

### LLM-facing design constraints

The test surface follows the same repair-oriented structure as ADRs 0080 and
0082:

- Test entries name a definition hash explicitly.
- Display names are required for readability but are not identity.
- Effects are explicit at the manifest boundary.
- The JSON result is deterministic for a fixed package graph and test
  manifest.
- Every failure status carries ADR 0041 diagnostics rather than relying on
  prose-only output.
- The stable JSON avoids timing, absolute paths, and raw process output so
  repair loops can diff results reliably.

## Alternatives considered

### Mark tests with a new canonical `test` node

Rejected. A new canonical node would make tests part of the core AST surface
when the existing `def` and `sig` nodes already describe executable package
definitions. The manifest can select test entries without changing canonical
text.

### Add `export test` visibility

Rejected. Tests are not a visibility class. They are package-local entry
points selected by tooling. Adding `export test` would blur module boundaries
and invite test-only imports from other units.

### Discover tests by file path or directory name

Rejected. `tests/`, `*_test.tac`, and similar conventions are familiar, but
they make layout semantic. File layout remains non-semantic; the manifest
names test definitions by hash.

### Treat every `Bool` definition as a test

Rejected. Many packages legitimately contain helper predicates. Implicitly
running every `Bool` definition would make adding a helper a test-suite edit
and would make test selection unstable.

### Let test effects be inferred without manifest policy

Rejected. Effectful tests are useful, but hidden IO or mutation at test time
is bad review surface. The manifest opt-in gives humans and repair tools an
explicit contract.

### Support expected-negative compile tests now

Rejected for ordinary package tests. Expected-negative tests are useful for
compiler conformance suites, but they require a source-fixture model that is
separate from ordinary package loading. This ADR records `compile-fail` and
`effect-fail` as result cases for ordinary tests; it does not turn them into
passing expectations.

### Include raw stdout/stderr and timings in stable JSON

Rejected. Raw process output and timings are useful for humans but make
machine repair loops noisy. Implementations may expose them in text mode or an
explicit trace mode outside `tacit-test-v1`.

## Consequences

- ADR 0082's manifest schema gains an optional `[[tests]]` table.
- Implementation work adds `tacit test`, manifest parsing for tests, test
  target validation, deterministic result ordering, and `tacit-test-v1`
  emission.
- Test code is ordinary package code. Its definitions participate in unit and
  package hashes like any other definition.
- Test selection metadata is not part of the package hash because manifest
  bytes are not package identity.
- Tests can call definitions across units and package dependencies through
  existing hash imports and visibility rules.
- Effectful tests are available without allowing `Div` tests to hang the
  runner by design.
- Future assertion libraries, expected-negative conformance fixtures, raw
  output capture, and timing/profiling can be added without changing the
  minimal package test contract.
- No work covered by this ADR may use `corpus/sealed/` contents, paths,
  metadata, or feedback to validate this design.

## Related decisions

- [ADR 0035](0035-p2-effect-set-canonical.md) - fixed effect-set atoms and
  ordering.
- [ADR 0041](0041-p2-structured-error-format.md) - structured diagnostic
  envelope reused inside test results.
- [ADR 0079](0079-phase-6-scope.md) - Phase 6 scope and Stage 5 requirement.
- [ADR 0080](0080-phase-6-module-semantics.md) - unit imports, exports,
  visibility, signatures, and definition hashes.
- [ADR 0081](0081-phase-6-project-graph.md) - deterministic project graph and
  derived layout.
- [ADR 0082](0082-phase-6-package-manifest-lockfile-cache.md) - package
  manifest, lockfile, cache, and package hash.
