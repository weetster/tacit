# 0087 - Phase 6 source-level stdlib foundations

**Status:** Accepted
**Date:** 2026-05-16
**Phase:** 6, Stage 9 design
**Closes:** [phase-6-plan.md Q-P6-11](../plans/phase-6-plan.md)
**Amends:** [ADR 0082](0082-phase-6-package-manifest-lockfile-cache.md),
[ADR 0084](0084-phase-6-fixed-width-integers.md), and
[ADR 0085](0085-phase-6-typed-mutable-memory.md) additively.

## Context

Phase 3 and Phase 4 proved that a curated compiler-recognized primitive
surface can make Tacit-Lite usable for generated programs. By Phase 6, that
same approach has diminishing returns: modules, packages, package tests, and
the local dependency cache now make ordinary Tacit source libraries viable.

Stage 9 starts moving library-shaped behavior out of the compiler primitive
namespace while preserving the lower-level operations that still need direct
checker or codegen participation. It must do this without changing the
hash-addressed package model:

- Canonical imports remain exact definition hashes.
- Package dependencies remain hash-pinned or path-resolved through
  `tacit.toml` and `tacit.lock`.
- Display names, package names, and dependency aliases remain advisory.
- Public library exports still need explicit type and effect signatures.
- Networking and broad host integration remain Stage 10 host-interface work,
  not new built-in primitives.
- No design, implementation, or validation work may read, list, or otherwise
  depend on `corpus/sealed/`.

This ADR is the Stage 9 design artifact. Implementation work follows in the
source package, CLI, fixture, and primer changes that consume this decision.

## Decision

### Standard libraries are ordinary packages

The Tacit stdlib is a set of ordinary Tacit packages shipped with the
repository and toolchain. Source packages live under a reserved source root:

```text
stdlib/
  libc-effects.toml
  tacit/
    core/
    bytes/
    array/
    text/
    collections/
    io/
    host/
```

Each package under `stdlib/tacit/` is a normal package root: it may contain a
`tacit.toml`, a lockfile, `.tac` units, `.tacd` sidecars, package tests, and
ordinary hash-pinned dependencies on other stdlib packages. These packages are
not special canonical forms and are not trusted differently from local source
packages.

Consumers use stdlib exports through ordinary package resolution:

- During repository development, tests and examples may depend on a stdlib
  package by `path`.
- Distributed or cached consumers depend on exact package hashes.
- Optional `source = { registry = "builtin", name = "tacit.bytes" }` metadata
  may be recorded as an advisory hint, but the `hash` remains mandatory for
  cache dependencies.
- A missing stdlib cache object uses the existing `dependency-unresolved` or
  `cache-missing-object` diagnostics from ADR 0082.

Stdlib package names and release labels are display metadata only. Renaming
`tacit.bytes` or changing a human version string does not affect package
identity. Changing a public export's body or signature changes its definition
hash and the containing package hash.

### No implicit prelude in Stage 9

Stage 9 adds no implicit prelude and no `std` name resolver.

Every source-level stdlib use is explicit at the package and unit boundary.
Authoring tools may render helpful aliases from sidecars or manifests, but the
canonical `imp` entry still contains the exact definition hash and declared
signature. A program that does not import a stdlib definition cannot refer to
it by unqualified source name.

Existing compiler-recognized `@name` primitives remain visible exactly as
before for backward compatibility. They are not injected by a source prelude;
they are the current primitive namespace.

This keeps stdlib adoption reviewable and avoids a hidden dependency changing
the package hash of existing programs.

### Package responsibilities

The initial source-level package split is intentionally small and mechanical.
It is a migration boundary, not a permanent module taxonomy.

| Package | Responsibility | Initial primitive relationship |
| --- | --- | --- |
| `tacit.core` | Small pure helpers and result-shape helpers that do not belong to a narrower package. | Source-defined when expressible from existing language constructs. No implicit import. |
| `tacit.bytes` | Byte-order assembly, byte swapping, byte-slice predicates, and byte-bus display aliases. | Wraps `@u16-from-*`, `@u32-from-*`, `@u64-from-*`, `@u16-bswap`, `@u32-bswap`, `@u64-bswap`, and `@u8vec-load/store-*` first. |
| `tacit.array` | Typed-vector allocation, length, get, set, fill, copy, slice, equality, and scan helpers. | Wraps the Stage 7 `@<ty>vec-*` and `@u8vec-*` primitive families. |
| `tacit.text` | ASCII classification/case, UTF-8 codepoint helpers, line/token indexing, byte-span text helpers, and string-literal output helpers. | Source-defines simple ASCII predicates/case helpers where possible; wraps UTF-8 and legacy text-index primitives initially. |
| `tacit.collections` | Range-table accessors, sorting, lower-bound, grouping, and common vector algorithms. | Source-defines `range-start` and `range-len` over vector get; wraps sort/search/grouping primitives initially. |
| `tacit.io` | File-descriptor stream helpers, standard input/output helpers, integer parse/format wrappers, and legacy `Buf` interop. | Wraps curated `@read`, `@write`, `@stdin-slurp`, `@write-range`, `@parse-i64`, and `@fmt-i64`. It does not add path-based file open. |
| `tacit.host` | Conventions for Stage 10 host-backed capability wrapper packages. | Contains no network or arbitrary FFI primitive in Stage 9. |

Public exports in these packages must have explicit signatures and explicit
definition-evaluation effects. Wrapper effects must not hide lower-level
effects:

- wrappers around pure primitives remain pure,
- wrappers around mutation carry `{Mut}`,
- wrappers around allocation carry `{Alloc}`,
- wrappers around host or file-descriptor I/O carry `{IO}`,
- wrappers around operations that can divide or otherwise use `Div` expose
  `Div` if the body requires it.

Package-local helpers may be exported as `package` when shared across stdlib
units. Helper definitions that are only implementation details remain private.

Direct re-export of imported definition hashes remains disallowed by ADR 0080.
If one stdlib package wants to expose another package's behavior, it must
define a wrapper with its own signature, body, and hash.

### First migration set

Stage 9 should migrate library-shaped behavior in this order:

1. Add source stdlib packages for byte-order wrappers, typed-array wrappers,
   ASCII/text helpers, collection/range helpers, and stream I/O wrappers.
2. Consume at least one stdlib package from a normal package test or example
   through ordinary `tacit.toml` dependency resolution and exact-hash imports.
3. Source-define the simple pure helpers that no longer justify compiler
   recognition, starting with ASCII classification/case helpers and
   range-table row accessors.
4. Keep the old `@ascii-*`, `@range-start`, and `@range-len` primitives as
   compatibility shims until all non-legacy checked-in examples and primer
   text prefer the source package exports.
5. Wrap, but do not remove, byte-order helpers and typed-vector helpers in the
   first implementation. Their primitive forms still provide direct lowering,
   bounds behavior, and concise compiler diagnostics.

This satisfies Stage 9 by moving the recommended surface into source
packages without forcing a disruptive primitive removal in the same step.

### What remains compiler-recognized

The following operations remain compiler-recognized primitives in Stage 9:

- fixed-width casts, wrapping/checked/saturating arithmetic, bit operations,
  shifts, rotates, masks, and byte swaps from ADR 0084,
- typed-vector allocation, length, get, set, byte-slice, and byte-bus
  operations from ADR 0085,
- curated process/file-descriptor primitives `@read`, `@write`, and `@exit`,
- legacy `Buf` and `I64Vec` allocation and element operations,
- higher-order combinators `@map`, `@fold`, and `@for-each`, because callback
  effect propagation is still checker-special,
- high-level Phase 3 primitives kept as compatibility shims until source
  replacements are proven in examples.

The source stdlib may wrap any of these primitives. Wrapping is the preferred
Stage 9 migration mechanism when an operation still needs direct bounds
checking, stack-allocation lowering, host linkage, or special effect
inference.

Removing a compiler primitive from the accepted surface is a separate
compatibility decision. It must not happen merely because a wrapper exists.

### Strings and byte spans

Stage 9 does not add a heap string type, string builder, or Unicode text
object. String literals and the existing `Str` handling remain compiler
surface. The source `tacit.text` package treats text as byte spans over `Buf`
or `u8vec` plus explicit offsets and lengths, with UTF-8 helpers available
where codepoint-level behavior is needed.

New source-facing text helpers should prefer `Bool` for predicates. Legacy
compatibility wrappers may preserve old `Int` flag shapes when that is useful
for existing examples.

### File I/O wrappers

Stage 9 `tacit.io` is a wrapper layer over the existing curated stream
surface. It may provide:

- standard stream aliases for file descriptors 0, 1, and 2,
- read/write helpers that make offset and length conventions explicit,
- whole-buffer stdin slurp and byte-range write wrappers,
- parse and format helpers for integer text.

It does not add path-based `open`, directory traversal, sockets, HTTP,
filesystem metadata, or process spawning. Those require host capability and
ABI decisions that belong to Stage 10 or later.

### Host-backed capability wrapper conventions

Stage 9 reserves a convention for future host-backed libraries without adding
networking as a built-in primitive.

A host-backed stdlib wrapper package must:

- live under `stdlib/tacit/host/<capability>/` or use a manifest display name
  beginning with `tacit.host.`,
- expose ordinary Tacit definitions with explicit type and effect signatures,
- declare `{IO}` for operations that cross the host boundary, plus `{Alloc}`
  or `{Mut}` when their Tacit body or ABI contract allocates or mutates,
- keep host import names and capability names advisory; canonical references
  remain hash-addressed or, after Stage 10, ABI metadata-addressed,
- never introduce arbitrary `extern "C"`, direct ecosystem-library bindings,
  dynamic plugin loading, or untyped pointer escape hatches.

No HTTP, networking, database, graphics, audio, windowing, or platform
integration package is accepted by Stage 9. Such packages can only become real
after Stage 10 defines host-provided imports, ownership, allocator boundaries,
and ABI-expressible type rules.

### Tests and examples

Stage 9 implementation tests must prove that source stdlib packages behave
like ordinary packages:

- at least one package test or durable example imports a public stdlib export
  by exact definition hash,
- the stdlib package is resolved through ordinary manifest and lockfile logic,
- public export signatures are checked exactly like non-stdlib package
  signatures,
- package-local stdlib helpers are not externally importable,
- old primitive examples continue to compile while source wrappers are added,
- no test, fixture, or validation step reads, lists, or searches
  `corpus/sealed/`.

## Consequences

- The compiler primitive namespace stops growing for library-shaped helpers.
- The first source stdlib packages can be tested by the same package pipeline
  used by user code.
- Hash-pinned stdlib dependencies preserve reproducibility and avoid a
  hidden, name-based standard library resolver.
- Existing examples remain compatible because primitive removal is deferred.
- Stage 10 receives a clear convention for host capability wrappers without
  Stage 9 adding any host ABI or networking primitive.

## Rejected alternatives

### Implicit prelude

Rejected. A hidden prelude would make source behavior depend on compiler
configuration rather than explicit package imports. It would also complicate
hash-based diagnostics by making an unmentioned package participate in a
definition body.

### Name-based `std.*` imports

Rejected for Stage 9. Name-based imports are convenient but conflict with the
Phase 6 rule that canonical dependencies are definition hashes. A future
authoring tool may resolve names to hashes, but the resolved artifact must
still be an ordinary package import.

### Rewrite every library primitive in source immediately

Rejected. Some primitives still need codegen or checker cooperation: stack
allocation, bounds-checked vector operations, byte-bus multi-byte access,
callback effect propagation, and host file-descriptor calls. Stage 9 starts
the migration with wrappers and simple pure source definitions.

### Remove old primitives as soon as wrappers exist

Rejected. Existing Phase 1 through Phase 6 examples and primers still mention
the current primitive surface. Compatibility shims let the source stdlib
become the recommended path without turning Stage 9 into a breaking cleanup.

### Built-in HTTP or network primitives

Rejected. Networking is host-owned capability work. Adding `@http-get` or a
socket primitive would violate the Phase 6 non-goal against networking as a
built-in language primitive and would bypass the Stage 10 ABI decisions.

### Treat stdlib packages as compiler magic

Rejected. Magic stdlib packages would be easier to load but would undermine
the package/cache model. The stdlib must exercise the same content-addressed
paths as user packages so package semantics stay honest.

## Related decisions

- [ADR 0080](0080-phase-6-module-semantics.md) - hash-addressed units,
  exports, imports, visibility, and explicit signatures.
- [ADR 0082](0082-phase-6-package-manifest-lockfile-cache.md) - package
  manifests, lockfiles, dependency cache, and package hashes.
- [ADR 0083](0083-phase-6-package-tests.md) - package tests and structured
  results.
- [ADR 0084](0084-phase-6-fixed-width-integers.md) - fixed-width integer and
  byte-order primitive surface.
- [ADR 0085](0085-phase-6-typed-mutable-memory.md) - typed-vector and byte-bus
  primitive surface.
- [phase-6-plan.md Q-P6-11](../plans/phase-6-plan.md) - closed by this ADR.
