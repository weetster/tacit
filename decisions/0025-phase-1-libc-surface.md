# 0025 — Phase 1 libc surface: OS-boundary symbols only

**Status:** Accepted
**Date:** 2026-04-24
**Phase:** 1, Stage 4
**Closes:** [tacit-plan.md Open Question Q3](../plans/tacit-plan.md);
[phase-1-plan.md Q-P1-1](../plans/phase-1-plan.md)

## Context

[tacit-plan.md § Open Questions Q3](../plans/tacit-plan.md) asks which
libc operations get effect-annotated wrappers in Phase 1. The question
was left open pending architectural clarity on what libc is *for* in
Tacit.

[ADR 0022](0022-pure-kernel-host-model.md) resolved the architectural
framing: libc is a **lowering detail for the curated in-language
stdlib**, not FFI and not a host. That ADR preserved the "no arbitrary
FFI" rule but made the positive commitment explicit — Tacit is a pure
computational kernel whose stdlib provides effect-annotated IO,
filesystem, network, and (eventually) threading, with libc as the
Phase 1–9 backing implementation and direct syscalls replacing it in
Phase 10.

With that framing in hand, the Q-P1-1 question becomes: what is the
*minimum* libc surface that lets Phase 1 demonstrate the pipeline, and
how is it stored so Phase 2's effect checker can read it without
rework?

libc functions split cleanly into two categories:

- **OS-boundary symbols.** Functions whose value is to cross from user
  space into the kernel: `write`, `read`, `exit`, `open`, `close`,
  `brk`/`mmap`, etc. These are what genuinely carry `IO` in Tacit's
  effect lattice. Phase 10's scratch stdlib replaces them with direct
  per-platform syscalls.
- **Pure compute.** Functions whose value is algorithmic, with no OS
  interaction: `strlen`, `memcmp`, `memcpy`, `atoi`, `sprintf` integer
  formatting, `qsort`, etc. These have no effect signature beyond what
  a Tacit-level implementation would carry. LLVM provides intrinsics
  for several of them (`llvm.memcpy.*`, `llvm.memset.*`, etc.) that
  the codegen can emit directly, without introducing a libc dependency.

Mixing pure-compute libc thunks into the effect-signature table would
conflate "calls libc" with "carries IO" — a category error that would
make Phase 10's migration larger than it needs to be and would muddy
the Phase 2 effect checker's semantic model.

## Decision

**Phase 1's libc surface is restricted to OS-boundary symbols. The
minimum set for the Phase 1 smoke corpus is three: `write`, `read`,
`exit`. Pure-compute libc functions are not used; their work is
inlined by the codegen as LLVM intrinsics, expressed in Tacit, or
scoped out of Phase 1 entirely.**

### Phase 1 libc set

| Symbol | Purpose | Tacit effect set |
|---|---|---|
| `write`  | Byte output to a file descriptor (stdout=1, stderr=2). Used for hello-world output via `write(1, buf, len)`. | `{IO}` |
| `read`   | Byte input from a file descriptor (stdin=0). Used for interactive smoke programs. | `{IO}` |
| `exit`   | Process termination with integer exit code. May be replaced by `return` from an `int main`-shaped entry point; kept available for non-`main` exit paths. | `{IO}` |

Adding a symbol to this list requires demonstrating it crosses an OS
boundary **and** a follow-up ADR amending the set. The boundary test
is mechanical: could the symbol's behavior be implemented without any
`syscall` instruction on Linux or `svc` on macOS arm64? If yes, it is
pure compute and does not belong in this set.

### Pure-compute handling

- **String literals.** Lengths are compile-time-known; codegen emits
  the length directly as an IR constant. `strlen` is never called.
- **Block memory operations.** `memcpy`/`memset`/`memmove`/`memcmp` are
  emitted as LLVM intrinsics (`llvm.memcpy.*`, etc.) when the codegen
  needs them. LLVM lowers the intrinsic to whatever the target expects;
  no libc linkage is required for these.
- **Integer formatting (for numeric output in smoke programs).** Not
  needed for hello-world. When a smoke program needs it, the options
  are (a) hand-written in Tacit-Lite once Phase 1 has integer
  arithmetic and `write`, or (b) skip the program for Phase 1 and
  defer to Phase 2's richer stdlib. Neither path re-introduces libc
  for pure compute.

### Effect-signature storage

- **File:** `stdlib/libc-effects.toml` at the workspace root,
  created as part of Stage 4.
- **Schema (per entry):**
  - `name` — libc symbol name (`write`, `read`, `exit`).
  - `c_signature` — the libc prototype as a string
    (e.g., `"ssize_t write(int fd, const void *buf, size_t count)"`).
  - `tacit_effect_set` — a list drawn from the fixed Lite lattice
    `{IO, Alloc, Mut, Div}`. All three Phase 1 entries are `["IO"]`.
  - `notes` — free-text context for future readers (e.g.,
    "stdout is fd 1; stderr is fd 2; errno is not surfaced to Tacit
    in Phase 1").
- **Consumer.** Phase 1 codegen does not consume this file — it emits
  calls by hard-coded symbol name. The file exists for Phase 2's
  effect checker to read without rework when the type and effect
  system comes online.

## Alternatives considered

- **Include pure-compute symbols (`strlen`, `memcmp`, `memcpy`,
  `sprintf`) in the Phase 1 libc set.** Rejected. Each is either
  expressible in Tacit, available as an LLVM intrinsic, or unneeded
  for Phase 1's smoke corpus. Including them conflates the
  OS-boundary architectural commitment with codegen convenience and
  inflates Phase 10's migration surface for no Phase 1 payoff.
- **Use `puts`/`printf`/`fputs` instead of `write`.** Rejected.
  `printf` in particular is a format-string interpreter that pulls in
  significant libc surface and whose effect signature is harder to
  write honestly (it calls `write` internally but also does
  pure-compute formatting). `write` is the smallest OS-boundary
  primitive and maps 1:1 to a Phase 10 syscall wrapper.
- **Defer writing the effect signatures until Phase 2 begins.**
  Rejected. Writing three entries now (while the symbols are in front
  of us) costs nothing; deferring adds a re-derivation task to
  Phase 2's schedule and invites drift between what Phase 1 emits and
  what Phase 2 believes the signatures are.
- **Larger Phase 1 surface (`open`, `close`, `stat`, `fork`, ...) to
  enable richer smoke programs.** Rejected as scope creep. Smoke
  programs demonstrate the pipeline; they do not exercise libc. Each
  added symbol would have to clear the OS-boundary bar and earn its
  own ADR amendment, which correctly raises the cost of expansion.
- **Store effect signatures inline in codegen source as Rust constants.**
  Rejected. A single `libc-effects.toml` is auditable in isolation,
  survives the codegen-crate layout changing, and can be read by
  Phase 2 without pulling in the codegen crate as a dependency.

## Consequences

- Phase 1's libc dependency is exactly three symbols. The Phase 10
  scratch-stdlib migration replaces three per-platform syscall
  wrappers, not a sprawling surface.
- The dormant effect-signature table is small, unambiguous, and
  entirely `{IO}`-carrying. Phase 2's first task against it (loading
  and checking) is correspondingly small.
- Hello-world codegen is concretely: a global constant holding the
  byte string, a `write(1, &str, len)` call, and either `exit(0)` or
  a `return 0` from an `int main`-shaped entry.
- Future smoke programs that want numeric output become a Phase 1
  sub-task to either emit-via-intrinsic-sequence or write in Tacit.
  The architectural line stays clean.
- [ADR 0022](0022-pure-kernel-host-model.md)'s framing is reinforced
  at the code level — libc is visible as the three-symbol OS-boundary
  shim, not as a general-purpose library the codegen reaches into.
- `stdlib/libc-effects.toml` becomes a new top-level artifact. Its
  schema is frozen by this ADR for the three Phase 1 entries;
  extending the schema (e.g., adding capability annotations for
  Tacit-Full) is a future ADR's call.

## Related decisions

- [ADR 0022](0022-pure-kernel-host-model.md) — pure kernel + host
  model; this ADR is Phase 1's concrete instantiation of "in-language
  stdlib backed by libc at the OS boundary."
- [tacit-plan.md § Open Question Q3](../plans/tacit-plan.md) — closed
  by this ADR. Parent plan updated to mark resolved.
- [phase-1-plan.md § Stage 4, § Open Questions Q-P1-1](../plans/phase-1-plan.md)
  — closed by this ADR.
- [tacit-plan.md § Phase 10](../plans/tacit-plan.md) — the scratch
  stdlib that will replace these three symbols with direct syscalls.
- Future Phase 2 ADR — may add capability annotations, refine the
  effect set (e.g., separating `{Read, Write}` subeffects), or extend
  the schema.
