# 0047 — Phase 3 stdlib expansion surface for corpus coverage

**Status:** Accepted
**Date:** 2026-04-28
**Phase:** 3, Stage 1
**Closes:** [phase-3-plan.md Q-P3-1](../plans/phase-3-plan.md)
**Amends:** [ADR 0028](0028-phase-1-libc-call-surface.md), [ADR 0030](0030-phase-1-arith-primitives.md), [ADR 0038](0038-p2-writable-buffer.md) — additive `@name` allowlist extension. [ADR 0025](0025-phase-1-libc-surface.md) is consulted but **not** amended.

## Context

[phase-3-plan.md § Stage 4–6](../plans/phase-3-plan.md) requires
hand-authored Tacit-Lite reference solutions for the 47 open corpus
tasks. The current `@name` primitive surface — LIBC `{write, read, exit}`
([ADR 0025](0025-phase-1-libc-surface.md), [ADR 0028](0028-phase-1-libc-call-surface.md)),
ARITH `{add, sub, mul, div, mod}` and CMP `{eq, ne, lt, le, gt, ge}`
([ADR 0030](0030-phase-1-arith-primitives.md)), and STACK-ALLOC
`{buf-alloc}` ([ADR 0038](0038-p2-writable-buffer.md)) — covers exactly
what the Phase 1–2 smoke corpus needs and nothing more.

A walk of the open corpus surfaces five recurring capability gaps that
the existing surface cannot express without introducing a new node
kind to the canonical format:

1. **Decimal integer parsing.** `int(line)` appears in tasks 010, 020,
   023, 027, 031, 036, 040, 046, 052, and most I/O tasks.
2. **Decimal integer formatting.** `print(x)` for `i64` `x` appears in
   001, 005, 010, 040, 046, 052 and elsewhere — every task whose output
   includes a number.
3. **Byte-level buffer access.** Loading and storing single bytes is
   the substrate for every parse/format/compare path. `@buf-alloc`
   produces a buffer; `@read`/`@write` move bytes wholesale; nothing in
   the existing surface accesses a single byte.
4. **Bytewise compare and scan.** Tasks 011, 013, 016, 017, 020, 045,
   056, 059, 060 need to compare byte regions (anagram, palindrome,
   common-prefix, dedup) or to find the next newline / space delimiter.
5. **Dynamic-size buffers.** Tasks 023, 027, 036, 040, 046, 049, 050,
   055, 056, 057 need an array whose size is determined at runtime from
   input length. ADR 0038 § Decision pins `@buf-alloc` to compile-time
   constant size; Phase 3 needs a runtime-sized companion.

Three things the corpus *does not* need, and which this ADR therefore
does not add:

- **New OS-boundary symbols.** The 47 open tasks all read stdin and
  write stdout; `read`, `write`, and `exit` cover the boundary. Adding
  `open`, `close`, `mmap`, etc. is a Phase 7+ stdlib concern, not a
  Phase 3 corpus concern. ADR 0025's "OS-boundary symbols only"
  discipline holds; `libc-effects.toml` is **not** extended by this
  ADR.
- **Hash maps / associative data structures.** Task 056 (`unique-lines`)
  is the only open task whose Python reference uses a hash-keyed
  primitive (`dict.fromkeys`); the Tacit reference for 056 falls back
  to O(n²) bytewise comparison and is acceptable on the test inputs
  per [phase-3-plan.md § Risks](../plans/phase-3-plan.md). Hash maps
  are explicitly Phase 7+ stdlib scope.
- **Heap allocator.** Dynamic stack alloc (`alloca` with a runtime
  size operand) is sufficient for the corpus — every dynamic buffer
  has a lifetime that fits the enclosing `let` scope. Heap allocation
  with explicit ownership is Tacit-Full scope per
  [ADR 0038 § Context](0038-p2-writable-buffer.md) and remains so.

## Decision

**Phase 3 extends the `@name` primitive allowlist with eight new
symbols across three new categories (PARSE, FORMAT, MEM) and one
extension to STACK-ALLOC. No new canonical node kinds, no changes to
`libc-effects.toml`, no changes to the effect lattice.**

### New primitive table

| Category     | `@name`           | Arity | Type                                              | Effect    | LLVM lowering                          |
|--------------|-------------------|-------|---------------------------------------------------|-----------|----------------------------------------|
| `PARSE`      | `@parse-i64`      | 3     | `Buf b → i64 → i64 → i64`                         | `{}`      | inlined digit-loop on `i8` loads       |
| `FORMAT`     | `@fmt-i64`        | 3     | `Buf b → i64 → i64 → i64`                         | `{Mut}`   | inlined divmod-loop with `i8` stores   |
| `MEM`        | `@buf-get`        | 2     | `Buf b → i64 → i64`                               | `{}`      | `load i8` + `zext` to `i64`            |
| `MEM`        | `@buf-set`        | 3     | `Buf b → i64 → i64 → i64`                         | `{Mut}`   | `trunc` to `i8` + `store i8`; returns 0|
| `MEM`        | `@buf-copy`       | 5     | `Buf b → i64 → Buf c → i64 → i64 → i64`           | `{Mut}`   | `llvm.memcpy.p0.p0.i64`; returns 0     |
| `MEM`        | `@buf-eq`         | 5     | `Buf b → i64 → Buf c → i64 → i64 → i64`           | `{}`      | inlined byte-compare loop; 0/1 result  |
| `MEM`        | `@scan-byte`      | 4     | `Buf b → i64 → i64 → i64 → i64`                   | `{}`      | inlined memchr-style loop              |
| `STACK-ALLOC`| `@buf-alloc-dyn`  | 1     | `i64 → Buf`                                       | `{Alloc}` | `alloca i8, %n`                        |

Argument orderings, exhaustively:

- `@parse-i64 buf off len` — parse leading digits (with optional
  leading `-`) of `buf[off..off+len]` and return the parsed `i64`.
  Stops at the first non-digit. Empty range or no leading digit
  returns 0. Overflow is undefined (matches ARITH's `nsw` discipline
  per ADR 0030). The caller is responsible for delimiting via
  `@scan-byte`.
- `@fmt-i64 buf off val` — write the decimal representation of `val`
  (with leading `-` for negatives) to `buf[off..]`. Returns the number
  of bytes written. Caller is responsible for ensuring `buf` has
  sufficient capacity (≤ 21 bytes covers any `i64`).
- `@buf-get buf off` — load the byte at `buf[off]`, zero-extend to
  `i64`. Out-of-range access is undefined.
- `@buf-set buf off byte` — store the low byte of `byte` to
  `buf[off]`. Returns 0 (placeholder unit). Out-of-range access is
  undefined.
- `@buf-copy dst dst-off src src-off len` — copy `len` bytes from
  `src[src-off..]` to `dst[dst-off..]`. Returns 0. Behavior on overlap
  matches LLVM `memcpy` (undefined); tasks needing overlap-safety
  must not arise in Phase 3 corpus and are deferred.
- `@buf-eq a a-off b b-off len` — return 1 if `a[a-off..a-off+len]`
  equals `b[b-off..b-off+len]` byte-for-byte, else 0.
- `@scan-byte buf off len target` — return the smallest offset
  `i ∈ [off, off+len)` such that `@buf-get buf i == target`, or
  `off+len` if no such offset exists. Returning the end is the
  "not found" sentinel rather than a negative value, matching how
  the corpus tasks consume the result (loop bound, not branch).
- `@buf-alloc-dyn n` — stack-allocate `n` bytes; `n` is a runtime
  `i64` expression (no constant-folding requirement). Returns a
  `Buf` handle whose lifetime is the enclosing `let` scope, same
  rule as `@buf-alloc`. The handle's compile-time size is unknown;
  the type is `Buf` (no size index), distinct from `@buf-alloc`'s
  `Buf N`.

Underapplication and overapplication continue to fail with
`CodegenError::PrimitiveArity` per ADR 0030's discipline; arities are
exact.

### Type system overlay

The `Buf` type (no size index) is a new monomorphic type alongside
`Buf N` introduced by ADR 0038. `Buf N <: Buf` — a fixed-size buffer
is usable wherever a dynamic buffer is expected, but not vice versa.
This subtyping is realised by the typechecker as an implicit forget
of the size index; no canonical-form change is required.

`@buf-get`, `@buf-set`, `@buf-copy`, `@buf-eq`, `@scan-byte` accept
either `Buf N` or `Buf` for any buffer argument, mirroring how
`@read` and `@write` already do under ADR 0038. `@parse-i64` and
`@fmt-i64` likewise accept either.

### Effect-lattice integration

All eight primitives use only the four atoms from
[ADR 0035](0035-p2-effect-set-canonical.md): `Alloc`, `Mut`, `IO`,
`Div`. No new atom is introduced. PARSE/FORMAT effect choices:

- `@parse-i64` is pure (`{}`). It reads from a buffer the caller owns;
  the read does not introduce a fresh effect because reads from a
  `Buf` are already covered by the `Mut`-or-empty distinction at the
  buffer's binder. The buffer is observable but not mutated by parse.
- `@fmt-i64`, `@buf-set`, `@buf-copy` carry `{Mut}` because they
  mutate buffer contents; the typechecker propagates `{Mut}` through
  the enclosing function per ADR 0035's join rule.
- `@buf-alloc-dyn` carries `{Alloc}` for symmetry with `@buf-alloc`.

`stdlib/libc-effects.toml` is **not** modified. None of the eight
new primitives crosses an OS boundary; the file's load-bearing
"OS-boundary symbols only" rule from ADR 0025 holds.

### Codegen surface

`tacit-codegen` recognises the eight new names at `App` head per the
ADR 0028 dispatch path. Each emits inline IR (no external linkage).
Two notes:

- **`@buf-copy` uses an LLVM intrinsic.** `llvm.memcpy.p0.p0.i64` is
  the only one of the eight that emits an intrinsic call rather than
  an inline loop. ADR 0025 § "Block memory operations" already
  whitelists `llvm.memcpy`, so this is consistent. No libc linkage
  is added.
- **`@buf-eq` and `@scan-byte` emit explicit loops** rather than
  `llvm.memcmp`/`llvm.memchr` because LLVM does not provide those as
  generic intrinsics. The inline-loop choice keeps the `-O0` IR
  predictable and avoids a libc dependency.

### Conformance test vectors

The new primitives extend the `@name` allowlist; they do not introduce
new canonical-form tags. No new test vectors are landed under
`plans/test-vectors/` — the existing V29–V33 vectors remain the
canonical-format conformance set. This ADR's behavioral conformance is
landed in Stage 2 as `crates/tacit-codegen/tests/p3_primitives.rs`,
exercising each primitive with one positive and one boundary case.

## Alternatives considered

- **Add `malloc`/`free`/`realloc` as libc symbols and a heap allocator.**
  Rejected. ADR 0025 § "OS-boundary symbols only" excludes these; they
  would be a libc-effects.toml schema regression. Stack alloca with a
  runtime size operand is sufficient for the open corpus, and the
  buffer-escape rule from ADR 0038 prevents the lifetime hazard that
  motivates a heap.
- **Inline `@parse-i64` and `@fmt-i64` as Tacit-Lite functions in
  `examples/phase-3/`.** Rejected. The token cost of writing these
  in-line in every reference would inflate Tacit-Lite token counts
  by 30–50 tokens per task — large enough to swamp the 30%
  reduction gate. Pinning them as primitives shifts the cost into the
  one-time stdlib surface, which is what a real Phase 7 stdlib will
  do anyway.
- **Make `@parse-i64` return both value and consumed length** (a
  tuple-shaped result). Rejected. Tacit-Lite has no tuple node and the
  canonical form is frozen; threading two return values through `let`
  bindings is heavier than the current "value + scan-byte for
  delimiter" pattern. The `@scan-byte` companion is cheap.
- **Add `@buf-len` to recover the size from a `Buf`.** Rejected. The
  size is always available at the `let` binder (it was the argument
  to `@buf-alloc-dyn`); pulling it from the value would require either
  a fat handle (size + pointer) or a runtime-stored prefix. Both are
  Phase 7+ work. Phase 3 references thread the size through `let`
  bindings explicitly, which is also more idiomatic.
- **Introduce a hash-map primitive for task 056.** Rejected. One task
  out of 47 is not justification for the surface — adding a hash map
  is a Phase 7 stdlib decision involving collision handling, hash
  function choice, and growable backing storage. Falling back to O(n²)
  comparison on 056's test inputs is acceptable; if the test inputs
  are infeasible at O(n²) the task is moved to sealed-only at
  Stage 4 review per [phase-3-plan.md § Risks](../plans/phase-3-plan.md).
- **Use `llvm.memchr.p0.i64` for `@scan-byte`.** Rejected. LLVM does
  not provide `memchr` as an intrinsic (only `memcpy`, `memmove`,
  `memset`); the candidate is a generic `<256 x i8>` vector
  intrinsic that the optimiser may or may not lower. An explicit
  loop is more predictable at `-O0` and matches the corpus discipline.
- **Bundle PARSE and FORMAT into one category named TEXT.** Rejected
  on stylistic grounds: PARSE is pure, FORMAT is `{Mut}`, and grouping
  them obscures the effect distinction in the codegen recognition
  table. Two narrow categories are cheaper to read.

## Consequences

- **Phase 3 corpus references are expressible.** The eight primitives
  cover every recurring capability gap surfaced by the corpus walk.
  Stage 4–6 authors can write references without per-task primitive
  patches.
- **Stage 2 implementation is bounded.** Eight primitives, each with
  inline IR (or one intrinsic), under one new category extension. Each
  primitive ships with one positive + one boundary test in
  `crates/tacit-codegen/tests/p3_primitives.rs`. The natural
  one-session work is 2–3 primitives plus tests.
- **`libc-effects.toml` stays at three entries.** The OS-boundary
  discipline is preserved; Phase 7's stdlib expansion will be a clean
  cut against a known-small surface.
- **`Buf` (dynamic) joins `Buf N` (fixed) as a typed kind.** The
  Phase 2 type system gains one new monomorphic type. Subtyping
  (`Buf N <: Buf`) is realised by the typechecker; canonical form
  is unaffected. This is the smallest-possible type-system extension
  that supports runtime-sized buffers.
- **Effect-set surface stays at four atoms.** No new effects are
  introduced; the join lattice from ADR 0035 holds. PARSE and the
  zero-effect MEM ops carry `{}`; FORMAT and the mutating MEM ops
  carry `{Mut}`; STACK-ALLOC carries `{Alloc}`.
- **One open primitive gap is acknowledged**: hash-keyed primitives
  for task 056. Per
  [phase-3-plan.md § Stage 6 exit gate](../plans/phase-3-plan.md), a
  task that resists clean Tacit expression is documented as a Q-P3-1
  follow-up rather than triggering scope creep here. If the O(n²)
  fallback is infeasible on 056's actual test inputs at Stage 4–6
  review, the task is moved to sealed-only and the gap is recorded
  for Phase 7.
- **This ADR freezes with Stage 1 freeze.** The eight primitives,
  their arities, types, effects, and lowerings are spec from this
  point. Stage 4–6 authors may not propose additions in-line;
  unanticipated needs surface as Q-P3-1 follow-up ADRs against this
  one, not as relaxations of the reference idiom rules in
  [ADR 0048](0048-p3-tacit-idiom-rules.md).

## Related decisions

- [ADR 0025](0025-phase-1-libc-surface.md) — `libc-effects.toml`
  schema; consulted, not amended.
- [ADR 0028](0028-phase-1-libc-call-surface.md) — `@name` allowlist
  dispatch; this ADR adds eight names and one new category to it.
- [ADR 0030](0030-phase-1-arith-primitives.md) — ARITH/CMP categories;
  PARSE/FORMAT/MEM follow the same `@name` + inline-IR pattern.
- [ADR 0035](0035-p2-effect-set-canonical.md) — effect-set canonical
  form; this ADR uses only the four existing atoms.
- [ADR 0038](0038-p2-writable-buffer.md) — STACK-ALLOC category and
  the `Buf N` type; this ADR extends both with `@buf-alloc-dyn` and
  `Buf`.
- [phase-3-plan.md § Stage 2](../plans/phase-3-plan.md) — implementation
  surface this ADR scopes.
- [phase-3-plan.md § Stage 4–6](../plans/phase-3-plan.md) — corpus
  references that consume this surface.
