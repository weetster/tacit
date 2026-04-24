# 0028 — Phase 1 libc call surface: `@name` sym heads at function position

**Status:** Accepted
**Date:** 2026-04-24
**Phase:** 1, Stage 4
**Closes:** [phase-1-plan.md Appendix A.7](../plans/phase-1-plan.md)

## Context

[ADR 0025](0025-phase-1-libc-surface.md) pinned the three-symbol Phase
1 libc set (`write`, `read`, `exit`) and committed effect signatures
to `stdlib/libc-effects.toml`. It stopped short of specifying how a
Tacit source program *names* those symbols.

Drafting [phase-1-plan.md Appendix A](../plans/phase-1-plan.md) —
the hello-world end-to-end trace — surfaced the gap. Tacit-Lite has
no free variables (every authoring-view identifier resolves to a
binder or errors), no module system yet, and the canonical-text
format is frozen, so inventing a new node kind is a spec bug per
[CLAUDE.md § Ground rules](../CLAUDE.md). The question is which of
the existing kinds absorbs the libc-call surface and what the
authoring view looks like above it.

Factors weighed:

1. **Authoring-view token count** under the cl100k target frozen in
   [ADR 0001](0001-target-tokenizer.md). Tacit is an AI-first
   language; call-site tokens compound across programs.
2. **Shadowing / scope pollution.** A user's lambda-bound name must
   not collide with the libc surface, and the codegen's recognition
   pattern must not misfire on ordinary `app` spines.
3. **Spec stability.** Options that touch the frozen canonical-text
   format are disfavoured; those that reuse existing kinds as the
   spec already described them are strictly cheaper.
4. **Stdlib scalability.** Phase 1 has three symbols; the stdlib in
   [ADR 0022](0022-pure-kernel-host-model.md) grows to cover IO,
   filesystem, network, and threading. The convention must absorb
   50+ names without friction.
5. **Phase 10 rename cost.** ADR 0022 anticipates swapping libc for
   direct syscalls. Conventions that encode "libc" in source require
   rewriting every IO call when that swap happens.
6. **Round-trip cleanliness** through the sidecar.
7. **Error ergonomics** — misuse should produce a clear allowlist
   diagnostic, not a codegen panic.

## Decision

**Phase 1's three libc symbols are referenced in the authoring view
as `@write`, `@read`, `@exit` and lower to `(sym write)`, `(sym read)`,
`(sym exit)` at the function position of an `App` spine.**

Concretely:

1. **Authoring view.** The `@`-prefixed form marks "this is a
   primitive / stdlib reference, not a lambda-bound identifier."
   Surface syntax: `@name arg₀ arg₁ …` parses as the same
   left-associative juxtaposition as ordinary application. The
   authoring-view grammar in
   [`candidates/authoring-bpe-compact.md`](../plans/candidates/authoring-bpe-compact.md)
   already reserves `@foo` as the surface form for `sym` nodes; this
   ADR extends that reservation to function position.
2. **Canonical form.** `@name` projects to `(sym name)` — the `@` is
   surface-only, not part of the canonical bytes. A call `@write 1
   "hi\n" 3` emits as
   `(app (app (app (sym write) (int 1)) (str "hi\n")) (int 3))`.
   No new canonical node kind is introduced.
3. **Codegen recognition.** `tacit-codegen` identifies a libc call by
   pattern-matching an `App` spine whose leftmost head is
   `Sym(name)` with `name` in the Phase 1 allowlist
   `{"write", "read", "exit"}`. The right-spine arguments are
   collected in source order and passed to a direct LLVM `call` on
   the libc symbol declared as `declare i64 @write(i32, i8*, i64)`
   (and analogues).
4. **Allowlist enforcement.** A `Sym(name)` in function position
   whose name is *not* in the Phase 1 allowlist fails codegen with
   `CodegenError::UnknownPrimitive { name, span }`. The `sym`
   namespace is reserved for Phase 2+ stdlib expansion — user code
   cannot define new `@name` primitives.
5. **`sym` in non-function positions.** Unaffected. `sym` retains
   its existing uses (record field names, ctor names, hole diag-ids)
   per [canonical-text-format.md § 2](../plans/canonical-text-format.md).
   Only `sym` at the function position of an `App` chain is treated
   as a primitive call.
6. **Sidecar.** No new sidecar entries. `sym` nodes do not carry
   binder metadata; the name is already in canonical form. Round-trip
   is structural.

## Alternatives considered

- **Bare reserved identifiers (`write 1 "..." 3`).** Cheapest in
  authoring-view tokens (~1 per head) but breaks Tacit-Lite's "every
  identifier resolves to a binder" invariant. A user writing
  `let write = ... in write ...` either shadows the primitive
  (surprising), doesn't shadow (equally surprising), or requires a
  special scoping rule carved out just for three names. Rejected on
  semantic grounds: the simplicity gain is illusory once shadowing
  rules are specified.
- **Capitalized `ctor` heads (`Write 1 "..." 3` → `(ctor Write ...)`).**
  Token-cheap (~1–2 per head) but semantically wrong. `ctor` denotes
  data constructors (ML/Haskell sense); Phase 2+ will introduce user
  ADTs where `Write` could plausibly be a user's constructor name.
  Also cross-cuts the authoring-view capitalization convention for a
  purpose it was not designed for.
- **`__libc_*` prefix (`__libc_write 1 "..." 3` → `(sym __libc_write)`).**
  Mechanically equivalent to the accepted decision in canonical
  structure, but worse on three axes: (a) ~4 tokens per head vs ~2
  for `@name`, compounding across programs; (b) leaks the libc
  implementation choice into source, which ADR 0022 anticipates
  replacing with direct syscalls in Phase 10 — every historical
  program would need a rename; (c) the `__`-prefix convention is
  borrowed from C/Python and does not signal "primitive" as
  strongly as `@` does.
- **Module projection (`sys.write 1 "..." 3` → `(proj sys-ref write)`).**
  Semantically the cleanest long-term answer — a stdlib is a real
  module, and `proj` is the right node. Rejected for Phase 1 because
  it requires a module surface-syntax and cross-module reference
  semantics that
  [canonical-text-format.md § 11](../plans/canonical-text-format.md)
  defers to Phase 1+. Pulling that work forward for three symbols is
  schedule risk without matching payoff. Phase 2+ may migrate the
  `@name` surface to `sys.name` once the module system lands; that
  migration is mechanical and content-address-preserving for
  non-libc code.
- **New canonical node kind (e.g., `(extern write ...)`).**
  Semantically precise but requires reopening the frozen canonical
  spec. Per [CLAUDE.md § Ground rules](../CLAUDE.md), such changes
  are spec bugs, not scope work. Rejected unless no in-spec option
  exists — which, given the accepted decision, it does.

## Consequences

- Stage 4 codegen gets a concrete recognition rule: match
  `App(App(...App(Sym(n), a₀), ...), aₙ)` where `n ∈ {"write",
  "read", "exit"}`; collect the right-spine arguments; emit a direct
  libc call. Everything else on the `App` spine lowers via the
  closed-lambda path ([ADR 0026](0026-phase-1-closed-lambdas.md)).
- Authoring-view programs do not encode "libc" in source.
  [ADR 0022](0022-pure-kernel-host-model.md)'s Phase 10 swap from
  libc to direct syscalls rewrites codegen only; source code and
  content hashes are untouched.
- The `sym`-at-function-position namespace becomes the Phase 2+
  stdlib surface. Adding `@open`, `@close`, `@socket`, etc., is a
  one-line allowlist change per symbol plus the corresponding
  `libc-effects.toml` entry. No new node kinds, no new authoring-view
  grammar.
- User code cannot define new `@name` primitives. This is intentional
  — the primitive surface is curated, not user-extensible. Tacit-Full
  capabilities (ADR 0022's reserved escape hatch) may lift this in a
  much later ADR.
- Error diagnostics are clean: `CodegenError::UnknownPrimitive` names
  the offending symbol and the span. A typo like `@wrie` fails at
  codegen with a clear message rather than a cryptic link error.
- Round-trip through the sidecar is trivial: `(sym name)` carries no
  binder metadata, so authoring ↔ canonical is purely structural for
  primitive calls.
- [Phase 1 Appendix A](../plans/phase-1-plan.md) and Appendix B smoke
  corpus entries update to the `@`-prefixed form; the LLVM IR in A.5
  is unaffected (LLVM-level symbols remain bare `@write` etc., which
  coincidentally matches the authoring-view surface but is a
  different namespace).
- Token-count confirmation deferred. The semantic-cleanliness margin
  over `__libc_*` was judged decisive; an after-the-fact tiktoken
  measurement on the Phase 1 smoke corpus is welcome but not
  blocking, per the decision to commit without measuring.

## Related decisions

- [ADR 0001](0001-target-tokenizer.md) — `cl100k_base`/`o200k_base`
  as the tokenizer target; the 10% margin rule frames when token
  count is decisive vs informational.
- [ADR 0003](0003-authoring-view-bpe-compact.md) — BPE-compact
  authoring view; this ADR extends the `@foo` surface reservation
  from name positions to function position.
- [ADR 0022](0022-pure-kernel-host-model.md) — pure kernel + host
  model; the Phase 10 syscall swap that makes leaking "libc" into
  source undesirable.
- [ADR 0025](0025-phase-1-libc-surface.md) — fixed the three-symbol
  libc set and effect signatures; this ADR specifies the source
  surface above it.
- [ADR 0026](0026-phase-1-closed-lambdas.md) — closed-lambda
  lowering; the fallback path for any `App` spine that doesn't match
  the primitive pattern.
- [ADR 0027](0027-phase-1-rec-lowering.md) — C calling convention
  applies to libc calls emitted under this ADR's recognition rule.
- Future Phase 2 ADR — may migrate `@name` surface to module
  projection (`sys.name`) once module composition lands. Migration
  is source-level only; content hashes are preserved for non-libc
  code, and libc-call hashes change deterministically under the new
  projection structure.
- Future Phase 10 ADR — replaces the three libc declarations with
  per-platform syscall wrappers; the `@name` source surface and the
  canonical `(sym name)` form both survive unchanged.
