# 0022 — Tacit is a pure computational kernel; impurity lives in the host

**Status:** Accepted
**Date:** 2026-04-24
**Phase:** Post-Phase-0 design clarification (forward-looking; no frozen artifact changes)

## Context

[tacit-plan.md § Semantic commitments](../plans/tacit-plan.md) currently states:

> **No foreign function interface beyond libc.** [...] Arbitrary C interop is never supported. This is a permanent design constraint, not a phase-ordering decision.

The constraint is the right one. The framing is wrong. Stated as a prohibition ("no FFI"), it reads as a limitation that closes the door on general-purpose application programming — game emulators, multimedia, anything that needs to drive an external library. Several real use cases the project would like to support (e.g. an emulator using SDL for window/audio/input) appear ruled out by the line.

Working through the question revealed that the prohibition follows from a *positive* architectural commitment that the plan never names directly: **Tacit is designed to be a pure computational kernel, not a self-contained application runtime.** The distinction this draws is not between "all impurity outside Tacit" and "all impurity inside Tacit," but a narrower one:

- **In-language:** IO, filesystem, network, and (eventually) threading, via a curated effect-annotated stdlib. Every such operation has an explicit effect signature (`IO`, `Alloc`, `Mut`, `Div`, and Tacit-Full's handler-based concurrency effects). Phase 1 backs the stdlib with libc; Phase 10 replaces libc with direct syscalls. This is *not* FFI and libc is *not* a host — libc is a lowering detail for the stdlib, the same way Rust's `println!` eventually reaches libc `write` without making Rust an FFI language.
- **Host-owned:** arbitrary ecosystem libraries (SDL, OpenGL, SQLite, OpenSSL, libcurl, etc.) — anything outside the curated stdlib. These are not expressible in Tacit and have no FFI mechanism to bind. Programs that need such libraries use a host that embeds the Tacit module via declared imports and exports. Structurally the same shape as WebAssembly or embedded scripting languages: the language artifact is a sealed computational module; the host is where the messy ecosystem world lives.

Once stated positively, the SDL question dissolves. The user writes the emulator's CPU/PPU/APU/memory-bus core in Tacit (pure state machines, exactly what the language is for); a small non-Tacit host binary (Rust or C) links SDL and drives the event loop, calling into the Tacit module each frame. Standard IO the Tacit code does along the way (logging, reading a ROM file from disk, writing a save state) uses the in-language stdlib and is not affected. Every Tacit principle survives — content-addressing covers the whole computational kernel, signatures are honest, capability gating is meaningful, ownership is verified. SDL's impurity is quarantined outside Tacit, where it belongs; the stdlib's libc-backed IO stays inside Tacit, where it also belongs.

This framing also explains *why* the FFI constraint is right rather than just *that* it is. FFI degrades every Tacit principle simultaneously:

- Content-addressing covers the call site but not the callee. `extern fn SDL_Init` hashes to a string-name reference into a world Tacit cannot describe.
- Signatures lie. `fn SDL_Init(flags: u32) -> i32` does not say "initializes global state, spawns threads, opens device handles, mutates a process-wide error string."
- Capability tokens become advisory. Any bound function can do anything.
- Ownership is asserted, not verified. C allocations sit outside Tacit's lifetime discipline.
- The AI-first thesis weakens specifically. A model writing FFI bindings is reasoning from training-data impressions of a library's behavior, not from facts the language guarantees.

Curated FFI (only language authors write bindings, users get a blessed wrapper module) was considered and rejected. The wrapper still has to do the FFI work; moving authorship from users to maintainers changes who writes the bindings, not the underlying semantics. Every degraded principle above remains degraded.

## Decision

**Tacit is a pure computational kernel by design. In-language IO, filesystem, network, and (eventually) threading are provided by a curated effect-annotated stdlib; arbitrary ecosystem-library access lives in a host that embeds a Tacit module, not in Tacit itself.** The constraint from `tacit-plan.md § Semantic commitments` is preserved; its framing is replaced by this positive architectural commitment.

Concretely:

### 1. The language

- Tacit modules are sealed computational artifacts. They contain pure functions, data definitions, effect signatures, and calls into the curated stdlib. They do not contain `extern` declarations, `dlopen` calls, or any other mechanism for naming symbols outside the Tacit hash universe that are not part of the curated stdlib.
- The scope of the no-FFI rule is precisely "no way to reach outside the curated stdlib." In-language stdlib IO/filesystem/network/threading is orthogonal to the rule — it is Tacit, not FFI, and its libc-backed implementation is a lowering detail. The Phase 1 libc-wrapper set (Q-P1-1) and the Phase 10 scratch-stdlib plan are unchanged in scope and unchanged in framing.
- There is no user-visible FFI mechanism for arbitrary ecosystem libraries, no curated-FFI mechanism, and no plan to add one. A future ADR could revisit this only with evidence that the host model fails on real workloads — a very high bar.

### 2. The host interface

Module boundaries gain explicit imports and exports. A Tacit module declares what host-provided values and effects it requires (`needs framebuffer: &mut [u32; 256*240]`, `needs audio_sink: AudioQueue`, `needs input: InputState`) and what entry points it exposes (`exports step_frame: (input: InputState) -> FrameOutput`). The host satisfies imports; the host calls exports.

The full specification of the host-interface surface is **deferred** until module composition becomes a live concern (Phase 2 or later, depending on when types and effects need to traverse module boundaries). What this ADR commits to:

- The host interface is a first-class part of Tacit's specification, not an afterthought or an external tool's responsibility.
- Module imports and exports are expressible in canonical form. Whether they appear as new node kinds or as a sidecar-style auxiliary file is a future ADR's call; the structural decision (imports/exports must be machine-readable from the canonical artifact) is made here.
- An ABI-expressible subset of Tacit types must exist for the boundary. Not every Tacit value can cross — closures with captured environments, refinement-typed values, and effect-polymorphic functions need either a stable representation or a structural ban at the boundary. The subset is deferred to the host-interface ADR; the requirement that one must exist is committed here.
- Memory ownership at the boundary is part of the host-interface spec — who allocates, who frees, what lives across calls. Tacit's ownership/lifetime model extends to the boundary; the host honors it.

### 3. Compile targets

The Phase 1 target (LLVM IR → native standalone executable) is unchanged. For the host model, two output forms are envisioned:

- **Linkable artifact + machine-readable interface description.** Tacit emits a static or shared library (`.a`/`.so`/`.dylib`/`.dll`) plus a language-neutral interface description (likely a structured file derived from canonical form plus the host-interface sidecar). Small generators consume the description to produce C headers, Rust bindings, or any other host language's binding layer. Preferred over hand-written C headers because Tacit's type info (effects, ownership, lifetimes) survives the boundary in the description but would be lost in a C header.
- **WASM (added as a candidate backend, not committed).** The pure-kernel-with-host model is structurally identical to WebAssembly's. Targeting WASM directly — alongside or instead of LLVM-native for the embedded use case — would inherit a mature solution to host sandboxing, capability-like import declarations, and a growing ecosystem of hosts (browsers, WASI runtimes, WASM-in-app embeddings). WASM is now on the candidate-backend list; the choice is deferred until the host-interface ADR (because backend choice and interface shape interact).

Phase 1's LLVM-native, libc-linked, standalone-executable target remains the current path. WASM does not displace it; it is logged as a candidate for when embedded-host use cases come online.

### 4. What this excludes

Tacit is not the right language for programs that are fundamentally thin orchestration layers around an ecosystem library — most GUI framework apps, programs whose value is mostly "calls into GTK/Qt/Cocoa," etc. The host model handles those by inverting the relationship (host is the GUI framework, Tacit is called from event handlers), but for programs that are mostly framework-orchestration code, that inversion is awkward and Tacit gives up most of its advantages. **This is acknowledged scope, not a defect.** Most application programs are mostly pure computation with IO at the edges; those are Tacit's target. Thin-wrapper-around-C-library programs are not.

## Alternatives considered

- **Add user-visible FFI with capability gating.** Considered and pursued for one turn of design discussion before being rejected. Capability gating is real protection in Tacit-Full but advisory in Tacit-Lite, and even with gating the underlying degradation of content-addressing, signature honesty, and ownership verification remains. The argument "ship curated wrappers on top" was the strongest pro, but it is a Rust/Zig/Swift pattern that assumes a human-designed language; an AI-first language is uniquely sensitive to the unverifiable-claim problem FFI introduces. Rejected.
- **Curated FFI only (language authors write bindings, users consume).** Considered and rejected. Moves authorship of bindings without changing the semantics of binding. Every degraded principle (opaque state, lying signatures, advisory capabilities, asserted-not-verified ownership) survives intact, just hidden from users. Also creates a governance bottleneck — every new library to bind becomes a PR-to-Tacit discussion, and the curated list always lags real application needs.
- **Drop the SDL/emulator goal; keep the constraint with no reframing.** Considered. Tenable, but leaves "Tacit is general-purpose" as an unsupported aspiration. The host model preserves the constraint *and* the general-purpose ambition by clarifying what "general-purpose" means in this language: pure kernel + host. Rejected as the lower-information option.
- **Defer the framing question entirely and just leave line 58 as-is.** Considered. Rejected because real design questions (host-interface spec, WASM as backend candidate, where libc usage sits in the architecture) hang on the framing. Leaving it implicit means each future ADR has to relitigate the underlying commitment.
- **Commit now to the full host-interface spec (imports, exports, ABI subset, ownership rules at boundary).** Rejected as premature. Phase 1 has not concretized module composition; designing the host-interface surface in isolation from how modules actually compose risks getting it wrong. The decision logged here is the *architectural commitment* that there will be such a spec; the spec itself waits for the right phase.

## Consequences

- **`tacit-plan.md` line 58 is rewritten** to state the positive commitment (pure kernel; host hosts impurity) with the no-arbitrary-FFI rule as a consequence rather than a standalone prohibition. The "permanent design constraint" framing is dropped; the constraint itself is preserved.
- **`tacit-plan.md` Open Question Q3** is updated to remove the parenthetical "Arbitrary C FFI beyond libc is never supported" — that statement now lives in this ADR's framing, not as a stray note in an open-questions section.
- **Backend section (`tacit-plan.md § Backend`)** gains a note that WASM is a candidate alongside LLVM IR for the embedded-host use case, deferred to the host-interface ADR.
- **Module composition (Phase 2+ work) is now scoped to include the host interface**, not just Tacit-to-Tacit module references. This is a meaningful expansion of what that future spec must cover; logged here so it is not a surprise when the work begins.
- **Phase 1 is unaffected.** libc-linked standalone executables remain the Phase 1 target; libc remains the stdlib's backing implementation. The reframing changes the overall architectural description (pure kernel vs. runtime); it does not recharacterize libc-as-stdlib-backing as any form of host or FFI.
- **The AI-first thesis sharpens.** "Tacit is the pure kernel; impurity lives in the host" is a more defensible architectural claim than "Tacit is general-purpose with a curated FFI list," because it preserves every first principle the language is built on. Programs that don't fit the kernel-plus-host shape are honestly out of scope rather than awkwardly accommodated.
- **Future workloads have a clear pattern.** Emulators, signal processors, codecs, parsers, interpreters, simulation cores, and most computational kernels fit naturally. GUI framework apps and ecosystem-orchestration programs do not, and the project does not pretend otherwise.
- **The host-interface ADR is now a known future deliverable** with scope sketched here (imports/exports in canonical form, ABI-expressible type subset, boundary ownership rules, compile-target choice including WASM). Likely Phase 2 or whenever module composition is first concretized; the trigger is "module boundaries become real," not a calendar.

## Related decisions

- [tacit-plan.md § Semantic commitments](../plans/tacit-plan.md) — the line this ADR reframes.
- [tacit-plan.md § Backend](../plans/tacit-plan.md) — gains a WASM-candidate note per this ADR.
- [ADR 0004](0004-rec-arity.md) — `module` kind reservation, on which the future host-interface spec builds.
- [phase-1-plan.md § Stage 4](../plans/phase-1-plan.md) — Phase 1 libc-wrapper work, now understood as the backing implementation of the curated stdlib (not as a host-model instance and not as an FFI exception).
- Future host-interface ADR (number TBD; expected Phase 2+) — will specify imports/exports surface, ABI-expressible type subset, boundary ownership rules, and finalize the LLVM-vs-WASM-vs-both backend choice.
