# Tacit: An AI-First Programming Language

**Project plan, v0.1**

## Vision

Tacit is a programming language designed for AI models to read and write, not humans. By removing the constraints of human readability, we can optimize for three goals simultaneously that human-oriented languages force tradeoffs between:

1. **Token efficiency** — dense representation for AI generation and consumption
2. **Runtime performance** — compile-time guarantees eliminate runtime checks
3. **Safety and security** — correctness properties are structural, not conventional

The CPU constraint remains. Tacit compiles to LLVM IR, then to native code, so we inherit decades of codegen work and can run anywhere LLVM runs.

The tension we accept: Tacit code is nearly unreadable to humans by design. Mitigation is two-way transpilation to Python (deferred stretch goal) and good inspection tooling.

---

## Core Language Concepts

### What we strip out (pure human conveniences)

- **Whitespace and formatting** — no indentation, no line breaks, no formatters needed
- **Comments** — reasoning lives in a separate metadata channel, not inline
- **Human-readable names** — variables are integer IDs; names are optional display metadata
- **Syntactic sugar** — one canonical form per operation, no alternatives
- **Prose error messages** — errors are structured data (code, AST path, expected vs actual, candidate fixes)
- **Operator precedence** — prefix/postfix notation, no PEMDAS, no parentheses for grouping
- **Free-form surface syntax** — programs are stored as a canonical text projection of the AST; AI authors through purpose-built views. No stylistic variance, no formatter debates, no syntax errors that aren't structural

### What we keep (genuinely load-bearing)

- **Type definitions** — structural types are compression, not decoration
- **Module boundaries** — define compilation units and capability boundaries; decoupled from filesystem

### What we add (miserable for humans, great for AI)

- **AST as source of truth, with multiple views** — the canonical form is the AST, not any textual surface. Views are purpose-built projections: an *authoring view* optimized for AI generation tokens, an *inspection view* annotated for reading and debugging, and (later) reasoning-specific views like data-flow or dependency projections. Storage is one canonical text view. No surface is privileged as "source code"
- **Effect tracking (Lite and Full)** — every function's type includes its effect set; no hidden side effects. Simple fixed lattice in Lite (`IO`, `Alloc`, `Mut`, `Div`) with basic effect polymorphism for higher-order functions
- **Refinement types (Full only)** — every value carries verified constraints (e.g., `int where 0 ≤ x < len(array)`)
- **Capability-based security (Full only)** — privileged operations require explicit capability tokens; no ambient authority
- **Content-addressing** — every function, type, and value identified by the BLAKE3 hash of its canonical text form; names are hints, not identifiers
- **Explicit evaluation order and memory layout** — no compiler magic; optimizations are local rewrites with provable equivalence
- **Versioned semantics at the expression level** — operators and stdlib functions carry version tags
- **Progressive disclosure of type info** — minimal signature by default, full spec on demand
- **Content-addressed modules** — a definition (function, type, value) is the atom, not a file. Each definition is identified by the BLAKE3 hash of its canonical text. Imports resolve to hashes; names are local aliases in display metadata. No dependency-version resolution — every version of a definition has a different hash, so "upgrade" is rebinding a local name to a new hash, and conflicts become structural questions about type compatibility. Cycles are impossible at the definition level (hash-of-A cannot transitively depend on hash-of-A); mutual recursion is expressed via explicit `rec { ... }` groupings that hash as a single atom. The AI emits these groupings directly (it knows which definitions call each other), so group boundaries stay stable under unrelated edits rather than shifting implicitly as a whole-program SCC analysis would. Registries are optional name→hash lookup services; hashes are authoritative. Module boundaries carry capability scopes in Full.

### Semantic commitments

These decisions define the shape of Tacit-Lite. They are deliberately chosen to keep local reasoning tractable for AI — every relevant fact about a function's behavior should be derivable from its signature, without whole-program analysis.

- **Memory model: ownership and borrowing.** Rust-style, but stricter. All lifetime information lives in the signature so an AI can read memory behavior without whole-program reasoning. No implicit lifetime elision — elision is a human ergonomic that hides information. When lifetimes appear, they are explicit scope IDs.
- **Errors are result types, not exceptions.** Every failure path is visible in the return type. No unwinding. Panic aborts the process and is not a normal control-flow construct. The effect lattice deliberately does *not* contain `Exn`.
- **Effect signatures are explicit at module boundaries; inferred locally.** Every exported definition carries an explicit effect signature. Internal helpers and local `let`-bindings infer. This keeps effect checking decidable by construction and means the effects of an imported function are legible from its signature alone, without whole-program reasoning.
- **Numeric types have explicit widths.** `i8`/`i16`/`i32`/`i64`/`u8`/... etc. No default integer type; declarations must specify a width. Overflow traps by default; wrapping and saturating are explicit operators. No implicit coercion between numeric types.
- **Strings are UTF-8 byte sequences.** Indexing returns bytes. Grapheme, code-point, and locale-aware operations live in explicit stdlib modules, not on the base string type.
- **Concurrency: none in Lite.** Single-threaded, deterministic execution. Structured concurrency via effect handlers is a Tacit-Full feature (Phase 7). Explicitly deferred, not undefined.
- **Pure computational kernel; ecosystem-library impurity lives in the host.** Tacit has in-language IO, filesystem, network, and (eventually) threading via a curated effect-annotated stdlib — Phase 1 backs the stdlib with libc, Phase 10 replaces libc with direct syscalls. libc is a lowering detail for the stdlib, not FFI and not a host. What Tacit does *not* have is a way to reach outside that curated stdlib: no user-visible FFI, no curated-FFI mechanism, no way to bind arbitrary ecosystem libraries (SDL, OpenGL, SQLite, etc.) from within Tacit code. Programs that need such libraries use the host model — the Tacit module declares imports and exports, a non-Tacit host satisfies imports and calls into the module, and ecosystem-library impurity is quarantined in the host. Structurally the same shape as WebAssembly or embedded scripting languages. The host-interface surface for non-degenerate embedders is deferred to a future ADR when module composition is concretized. See [ADR 0022](../decisions/0022-pure-kernel-host-model.md).

### Two variants

- **Tacit-Lite** — canonical text AST, structural types, simple effect tracking, single-threaded execution, two views (authoring and inspection). Designed to stand alone as a practical language for low-to-medium complexity programs, not merely a stepping stone to Full. Smaller models (Sonnet/Haiku class) should be able to write it proficiently from a primer alone.
- **Tacit-Full** — adds refinement types (SMT-backed), capability tokens, proof obligations, and richer effect discipline (handlers, user-defined effects, row polymorphism). Requires Opus-class models plus a specialized verifier (Z3 or similar). Research-grade; correctness-critical domains.

**Default focus: Tacit-Lite.** Tacit-Full is a stretch goal after Lite is working.

### Explicitly deferred features

- Two-way transpilation with Python (interesting use case, but adds metadata/merge complexity we don't need at v0)
- Self-hosting (bootstrap compiler in Tacit itself — only attempt once language is stable)
- Multiple parallel representations of same logic (4x token multiplier not worth the consistency benefit)
- Mandatory performance contracts (optional annotations only)
- Synthetic training corpus / fine-tuning. Was originally planned as a conditional Phase 5 ("urgent if primer-only fluency falls short"). Phase 3 measured primer-only fluency at 97.9% Sonnet (library-mediated) and 91.5% GPT-5.4 (primer-only) per [ADR 0070](../decisions/0070-p3-frozen.md), and Phase 4 improved the open Sonnet repair-loop result to 100% final per [ADR 0075](../decisions/0075-phase-4-frozen.md), so the triggering condition cannot fire. Re-open only if later language-shape work materially degrades fluency.

---

## Technology Choices

### v0 compiler language: **Rust**

Rationale:
- Semantic proximity to Tacit (ownership ≈ capabilities, traits ≈ effects) means the v0 compiler's internal data structures will feel natural when translated to Tacit for eventual self-hosting
- Excellent LLVM bindings via `inkwell`
- Strong pattern matching for AST manipulation
- Pragmatic choice: AI assistance for Rust is good, ecosystem is mature
- OCaml was considered (traditional compiler language, arguably cleaner fit) but Rust's ecosystem wins

### Backend: **LLVM IR**

- Inherits decades of optimization work
- Cross-platform code generation for free
- Skipping C as intermediate — LLVM IR gives low-level primitives with better semantics than C
- **WASM is a candidate backend** alongside LLVM native, for programs that embed into non-Tacit hosts per [ADR 0022](../decisions/0022-pure-kernel-host-model.md). The pure-kernel-with-host shape maps directly onto WASM's import/export model. Deferred to the host-interface ADR; not committed for Phase 1.

### Storage format: **Canonical text AST**

- The AST is the source of truth. The canonical form is a strict text projection — every AST configuration has exactly one canonical text serialization (fixed ordering, fixed integer encoding, no whitespace variance).
- Content addressing: BLAKE3 hash of the canonical text of any subtree identifies that subtree. Stable across tools, files, and time.
- Variable references use DeBruijn indices in canonical text — each reference is a depth index into the enclosing binders, so there are no variable IDs at all. Alpha-equivalent programs hash identically by construction, and identical function bodies reused in different places share hashes automatically. The authoring and inspection views project DeBruijn references back to readable integer or name labels; display names live in optional sidecar display metadata. Display names (for both variables and definitions) are auto-generated by the authoring AI from the intent of the code; they are advisory labels bound to hashes, not identifiers. Different projects can bind different names to the same canonical hash.
- Parser error recovery uses typed holes: malformed subtrees become `Hole` AST nodes carrying structured diagnostics (position, expected kinds, what was found). Compilation past a hole is blocked, but diff, view, blame, and hashing still operate on the rest of the file. Stretch target: an AST-edit protocol where the AI emits insert/replace/delete operations against a known-good tree rather than emitting canonical text directly, making malformed state unrepresentable.
- Three file extensions ([ADR 0071](../decisions/0071-storage-format-reconciliation.md)): `.tac` (canonical text, authoritative, hashed), `.tacd` (JSON display sidecar — binder names, comments, field order, type/effect hints), `.taca` (authoring view — transient render, not produced by the normal dev workflow). Regular development reads and writes `.tac` + `.tacd`; `.taca` is rendered on demand for human or AI consumption and is never stored in the working tree under normal use.
- Binary storage is explicitly deferred. If profiling later shows parse overhead is load-bearing, a derived binary cache can be added without changing the canonical form.

### File organization

Files are purely a human convenience; layout carries no semantic weight. Moving a definition between files changes no hashes and breaks no imports. This makes reorganization semantically free — a property worth exploiting rather than worrying about.

- **v0 default: one `.tac` + one `.tacd` file per project.** Simplest option; the compiler truly doesn't care. Defensible for the small codebases Phase 1–3 will produce. The paired files are always written together; tooling treats them as an atomic unit.
- **Layout as a derived artifact.** Once projects grow (rough threshold: ~2K lines of canonical text), a deterministic layout tool partitions definitions into files along call-graph or shared-type boundaries. Layout becomes reproducible output, like formatting — not an authoring decision.
- **AI does not reason about file layout during authoring.** It operates on content-addressed definitions; file location is ambient. This keeps authoring focused on semantics and avoids inconsistent layout choices across agents or sessions.
- **No cross-project dependency story in v0.** When cross-project imports arrive, a local hash-indexed cache (an object store, à la `.git/objects/`) becomes necessary to hold fetched dependencies and preserve historical references. Not a Phase 1 concern; design when dependency needs actually materialize.

### Object store (deferred)

Content-addressed storage keyed by definition hash — conceptually analogous to `.git/objects/`. Becomes necessary for:

- **Dependency caching** once cross-project imports exist.
- **Historical references** — keeping old hashes alive as long as anything still points to them.

Not needed for v0's single-project, no-external-deps compiler. An in-memory hash index built at parse time is sufficient. Specify and implement when the project grows a dependency model.

### Views: **Two from day one**

- **Authoring view** — dense, token-efficient, the format AI reads and writes. Optimized for fewest tokens per AST node under a modern BPE tokenizer.
- **Inspection view** — indented, type-annotated, effect-annotated. Used by `tacit-view` for debugging and code review. The two views are lossless projections of the same AST; switching between them preserves all semantic content.
- Additional purpose-built views (data-flow, dependency, trace) are deferred. But the view *system* is a core compiler subsystem from day one — real infrastructure, not retrofitted UI.

### Standard library approach

- **v0 links against libc** (and macOS equivalents). Pragmatic compromise: Phase 1's goal is a working end-to-end pipeline, not philosophical purity. Every libc wrapper gets a hand-written effect signature.
- **Scratch stdlib** (direct syscalls, no libc) is a late stretch goal — see Phase 10. Honors the AI-first premise (no ecosystem dependencies shaped for humans, no C-era assumptions) and gives full control over primitive effect signatures. Deferrable until the language proves itself.

### Training corpus for AI fluency

Priority order:
1. **Rust** (~60%) — closest living relative; ownership and effect semantics translate well
2. **Haskell / OCaml** (~25%) — effect discipline, pure-function patterns
3. **Python** (~10-15%) — volume and coverage of "normal" programming tasks
4. **Lean / Coq** (~1-2%) — proof obligation patterns (for Tacit-Full only)

Explicitly excluded: C, C++, Java, Go, JavaScript. Each either lacks the semantic richness or actively teaches the wrong patterns.

### Version control

- **Git as storage substrate** — handles binary content, has ecosystem support
- **Custom diff/merge/blame drivers** — structural diff over AST, registered via Git's driver mechanism
- **Long-term**: consider Unison-style content-addressed VCS once language is stable

---

## Phased Plan

### Phase 0: Foundations

**Goal:** Concrete specification and tooling scaffolding before writing real code. Phase 0 is blocking — Phase 1 must not begin until all deliverables here are frozen, because every downstream artifact (parser, content-addressing, primer, evaluation harness) encodes these decisions.

Deliverables:
- **Canonical text format specification (blocking prerequisite).** Exact grammar, integer encoding, field ordering in records, whitespace rules, and hash algorithm (BLAKE3 confirmed or substitute chosen). Three foundational decisions are already settled; Phase 0's job is to specify each down to exact bytes:
  - Variable references use DeBruijn indices; there are no variable IDs in canonical text. Authoring and inspection views project to readable name or integer labels.
  - Mutual recursion is expressed via explicit `rec { ... }` groupings; the whole group hashes as one atom.
  - Parser error recovery produces typed holes — malformed subtrees become `Hole` nodes carrying structured diagnostics, so the rest of the file stays analyzable.

  Every AST configuration must have exactly one canonical text serialization. This must be frozen before any other Phase 0 deliverable — two independent implementations must produce identical bytes for the same AST.
- Formal grammar for Tacit-Lite AST (as a Rust enum hierarchy initially), derived from the frozen canonical format
- Authoring view grammar with token-efficiency analysis under a target BPE tokenizer
- Inspection view grammar
- Content-addressing scheme specification (BLAKE3 over canonical text of each subtree, including the mutual-recursion grouping rule)
- **Phase 3 evaluation task corpus.** Defined and frozen upfront so Phase 3 does not grade its own homework. ~50–100 tasks with reference solutions in Python/Rust and executable test cases. A held-out subset is sealed and cannot be used as primer examples or training material.
- Project repository structure, CI, issue tracking
- Decision log document for recording design choices and rationale

Exit criteria: (1) Canonical text format is frozen and specified precisely enough that two independent implementations produce identical bytes for the same AST. (2) Another developer could read the spec and build a parser for the canonical storage view, and round-trip it losslessly to the authoring view. (3) Phase 3 task corpus is finalized and the held-out set is sealed.

### Phase 1: Minimum viable compiler

**Goal:** Compile a "hello world" equivalent from a canonical-text `.tac` file to a running native executable.

Deliverables:
- Rust crate with AST data structures matching the Phase 0 spec
- Parser for canonical storage view (reads `.tac` files into AST)
- Parser and serializer for authoring view, with lossless round-trip to the storage view
- LLVM IR emitter for basic constructs: integer arithmetic, function definitions, function calls, conditionals, loops
- Minimal libc linkage for hello-world (printf or equivalent)
- Hand-crafted test programs written in the authoring view
- CLI: `tacit compile foo.tac -o foo`, `tacit view foo.tac --as authoring|inspection`
- Documentation of the compiler architecture

Deliberately out of scope: type checking, effects, capabilities, any optimization beyond what LLVM does automatically.

Exit criteria: You can write a program in the authoring view, canonicalize it to the storage view, compile the result, run it, and see expected output.

### Phase 2: Type and effect system (Tacit-Lite)

**Goal:** Static type *and* effect checking for the Lite variant.

Deliverables:
- Local type inference within function bodies; exported definitions require explicit type signatures
- Structural type checking (no refinements yet)
- Basic generic types
- **Simple effect system:** fixed lattice (`IO`, `Alloc`, `Mut`, `Div`), local inference within bodies, mandatory effect annotations at module boundaries, basic effect polymorphism for higher-order functions
- Effect signatures for the libc-wrapper stdlib
- Structured error reporting format (JSON-emittable), covering both type and effect errors
- Type-directed overload resolution for operators
- View rendering of effect sets (dense in authoring view, verbose in inspection view)

Deliberately out of scope: effect handlers, user-defined effects, row polymorphism — all deferred to Full. Scope discipline here is critical; it's easy to nerd-snipe into research-grade effect systems.

Exit criteria: Non-trivial programs (sorting algorithms, basic data structures, file I/O) typecheck with correct effect annotations and compile.

### Phase 3: AI authoring primer

**Goal:** Enable an existing model to write Tacit-Lite in-context.

Deliverables:
- ~10-17K token primer document (written in the authoring view) structured as:
  - One-page semantic summary
  - Progressive examples (Python/Rust ↔ Tacit-Lite pairs) from trivial to complex
  - Idiom catalog (canonical form for each common pattern)
  - Effect-reasoning examples (propagation, requiring purity, fixing effect errors)
  - Negative examples with structured explanations
  - Compiler error catalog with fix patterns
- Evaluation harness that runs the Phase 0 task corpus: auto-check that generated Tacit compiles, typechecks with correct effects, and passes the corpus test cases. The corpus itself is not a Phase 3 deliverable — it was frozen in Phase 0.
- End-to-end token measurement: primer + generation tokens, compared against equivalent Python solutions
- Baseline measurements: Sonnet and Haiku performance with primer alone on the sealed held-out set
- **Maintenance/edit/repair sub-track (open).** A small second evaluation track of edit, repair, and refactor tasks on slightly larger programs. The Phase 0 corpus is a strong "small task solving" benchmark but does not directly test the canonical-AST + lossless-views thesis: that program identity, deterministic projection, and structural diffability improve long-horizon editing, repair, and explanation. This sub-track is the falsification surface for that claim. Scope, task count, and grading rubric are open and resolve early in Phase 3 alongside the primer design.
- **Cross-family evaluation sub-track (open).** Run the primary corpus and the maintenance sub-track against at least one model from each of: Claude family (current Sonnet calibration point), GPT family, and a strong open-weight family (e.g. Llama, Qwen, DeepSeek-class). Report per-family numbers across compile success, test-pass rate, end-to-end token cost, repair success after a deliberate error injection, and authoring-view round-trip stability of generated code. The thesis is "AST-first + lossless views + content-addressed identity removes representational ambiguity for *any* model"; cross-family numbers are how that claim is falsifiable. The current Sonnet/Haiku/Opus framing in this plan reflects calibration history, not the long-term success surface — once cross-family data exists, prose elsewhere should be retuned to match. Family selection, primer-portability rules (one shared primer vs. per-family variants), and grading details are open and resolve early in Phase 3.

Exit criteria: Sonnet achieves > 70% pass rate on a defined task corpus using only the primer in context, AND end-to-end token usage is at least 30% lower than equivalent Python. The maintenance and cross-family sub-tracks each have their own success criteria, not yet defined; they are reported alongside the primary gate but are not part of the go/no-go decision in Phase 3 itself. A material cross-family regression (e.g. open-weight pass rate collapses) is grounds for re-opening the primer design rather than a Phase-3 fail.

### Phase 4: Language-surface expansion

**Status:** Frozen 2026-05-08 by
[ADR 0075](../decisions/0075-phase-4-frozen.md).

**Goal:** Close the dominant remaining structural gap from Phase 3 — the no-tuples / no-closures / no-higher-order-combinators ceiling — by expanding Tacit-Lite's language surface. Per [ADR 0070](../decisions/0070-p3-frozen.md) § Strategic direction, this is the binding scope for Phase 4: language-shape work justified primarily as "reasoning support" rather than density chase.

Deliverables:
- **Records first.** Value-level product types with structural typing, addressing the pattern-5 multi-return failure mode from [ADR 0070](../decisions/0070-p3-frozen.md). [ADR 0072](../decisions/0072-p4-record-products.md) resolves the tuples-vs-records-vs-both choice: records are the Phase 4 product type, tuple syntax is deferred.
- **Closures / first-class function values.** Generalizes the closed-lambda surface of [ADR 0026](../decisions/0026-closed-lambda-surface.md) so functions can be passed, returned, and stored. Free-variable capture, escape analysis, and codegen-time closure conversion.
- **Higher-order combinators.** `map`, `fold`, `for-each` and similar shapes over collections — not expressible without the value-of-function story above.
- **Effect-system extension for closures.** Function values carry an effect signature; capture sites reconcile effect rows. A modest extension to the Lite effect lattice ([ADR 0035](../decisions/0035-p2-effect-set-canonical.md)), not a move to row polymorphism (which remains Phase 7).
- **Primer revision.** Extend the Phase 3 primer with the new constructs, idioms, and worked examples; re-baseline the primer token budget against the expanded surface.
- **Corpus re-evaluation.** Re-run the Phase 3 open corpus against models with the new primer. Report per-task density delta vs the Phase 3 baseline and per-model fluency delta. Held-out/sealed runs require an explicit sealed-grading request and must not be used for development feedback.
- **Density baseline switch.** `corpus-tokens` reporting promotes the Rust ratio to primary and demotes the Python ratio to descriptive (per [ADR 0070](../decisions/0070-p3-frozen.md) § item 4). Phase 4 *may* set a Rust-relative aspiration (e.g., ≤ 1.5× Rust on the corpus); it *may not* set a Python-relative gate.

Deliberately out of scope: refinement types, effect handlers, user-defined effects, row polymorphism, capabilities — all Phase 7. Concurrency remains absent.

Outcome: records, closures, and the `map`/`fold`/`for-each` family compile, typecheck, inspect, round-trip, and execute correctly on the Phase 4 smoke corpus and examples. Open-corpus re-evaluation shows material fluency improvement (38/47 one-shot, 47/47 final after repair) and generated authoring-output improvement when primer is excluded (2.85× Rust after repair), but no measurable Rust-density improvement under the current end-to-end primer-plus-generation metric. `plans/phase-4-plan.md` is the frozen scope artifact; [ADR 0075](../decisions/0075-phase-4-frozen.md) records the mixed density finding.

### Phase 5: Inspection and debugging tooling

**Goal:** Make Tacit debuggable by AI and inspectable by humans. Sequenced after Phase 4 because the existing inspection surface — structured error output ([ADR 0041](../decisions/0041-p2-structured-error-format.md)), `tacit view --types --effects`, and the `corpus-eval` repair loop — already covers the load-bearing inspection needs for advancing the language. Tooling becomes load-bearing once programs grow past the Phase 4 surface and exceed what the existing views and error format make legible.

Deliverables:
- `tacit view` extensions — registered-view system supporting authoring, inspection, and future views (data-flow, dependency). Phase 1–2 already shipped the renderer; Phase 5 generalizes it.
- `tacit-debug` — **AI-first CLI debugger**: step through execution, inspect values and types at any AST node, emit structured JSON output designed for AI consumption rather than human terminal readability.
- `tacit-diff` — structural diff over AST (ignores cosmetic renames and sidecar shuffles).
- `tacit-blame` — AST history traversal.
- Git integration: `.gitattributes` config so standard git operations fall back gracefully on canonical text.

Deferred to stretch: `tacit-merge` (semantic three-way AST merge). Collaborative development isn't a v0 concern, and multiple AI agents concurrently editing the same file isn't a current use case.

Exit criteria: An AI agent can diagnose a failing Tacit program end-to-end using only `tacit-debug` output; a human can read diffs and inspect state through `tacit view`.

### Phase 6: Optimization and hardening

**Goal:** Make Tacit competitive on performance and robustness.

Deliverables:
- Tacit-specific optimization passes (dead code elimination over AST before LLVM, constant folding with refinement awareness)
- Fuzzing infrastructure for the compiler
- Performance benchmarks against equivalent Rust, C, Python code
- Known-bug tracker with regression tests
- Documentation for contributors

Exit criteria: Tacit-Lite performs within 20% of hand-written Rust on standard benchmarks and passes a 72-hour fuzz campaign without compiler crashes.

### Phase 7 (stretch): Tacit-Full features

**Goal:** The research-grade correctness stack.

Deliverables (incremental, each a sub-phase):
- Refinement type system (SMT-backed via Z3)
- Advanced effect discipline: effect handlers, user-defined effects, row polymorphism (building on Lite's simple effect system)
- Structured concurrency via effect handlers (not present in Lite)
- Capability tokens and enforcement (effects with runtime witnesses)
- Proof obligation generation and discharge
- Updated primer for Tacit-Full authoring (probably 30-50K tokens)
- Integration with a proof assistant for complex obligations

No fixed timeline. Each feature is independently valuable and can be deferred indefinitely.

### Phase 8 (stretch): Self-hosting

**Goal:** Tacit compiler written in Tacit.

Prerequisites: Phase 2 (type system) plus enough stdlib for file I/O, strings, data structures.

Sequence:
1. Port v0 compiler data structures and core algorithms to Tacit (manual translation)
2. Compile the Tacit compiler using the Rust compiler (v0)
3. Compile the Tacit compiler using itself
4. Verify fixed point: output of step 3 is functionally equivalent to output of step 2
5. Archive the Rust v0 compiler

### Phase 9 (stretch): Two-way transpilation with Python

Deliverables:
- Tacit → Python transpiler (easy direction)
- Python → Tacit-Lite transpiler with metadata preservation
- Merge algorithm for round-trips through AI edits
- Use case validation: measurable token savings on real coding workflows

### Phase 10 (stretch): Scratch standard library

**Goal:** Remove libc dependency; call OS syscalls directly.

Rationale: Honors the AI-first premise — no ecosystem dependencies shaped for humans, no C-era assumptions. Gives full control over effect signatures of primitives. Deferrable until the language has proven itself.

Deliverables:
- Per-platform syscall bindings (Linux x86_64 and arm64, macOS arm64/x86_64)
- Memory allocator, I/O primitives, string and collection types implemented in Tacit
- Hand-authored effect signatures for every primitive operation
- Migration path for existing code from the libc-backed stdlib

No fixed timeline. Deferrable indefinitely.

### Phase 11 (stretch): Collaborative development

**Goal:** Support multiple agents (AI or human) working on the same codebase.

Deliverables:
- `tacit-merge` — semantic three-way AST merge (pulled forward from Phase 5's deferred list)
- Conflict resolution heuristics at the AST node level
- Integration with review tooling (GitHub-like interfaces that render views of diffs)

Prerequisites: real use cases justifying the complexity. Not a v0 concern.

---

## Open Questions

Phase 0–4 questions are all resolved (Phase 0–2 in their respective freeze ADRs; Phase 3's Q-P3-1 through Q-P3-9 closed by [ADR 0056](../decisions/0056-p3-stage-1-frozen.md), and Phase 4's Q-P4-1 through Q-P4-6 closed by [ADR 0072](../decisions/0072-p4-record-products.md), [ADR 0073](../decisions/0073-p4-function-values-and-closures.md), [ADR 0074](../decisions/0074-p4-higher-order-combinators.md), and [ADR 0075](../decisions/0075-phase-4-frozen.md)). The questions below surface for Phase 5 and beyond.

**Phase 4 (language surface, resolved):**

1. **Tuples vs records vs both.** Resolved by [ADR 0072](../decisions/0072-p4-record-products.md): records first, tuple syntax deferred.
2. **Closure representation.** Resolved by [ADR 0073](../decisions/0073-p4-function-values-and-closures.md): two-word closure pair with minimized by-value captures.
3. **Higher-order combinator surface.** Resolved by [ADR 0074](../decisions/0074-p4-higher-order-combinators.md): compiler-recognized `@map` / `@fold` / `@for-each` over `I64Vec`.
4. **Effect-row extension shape.** Resolved by [ADR 0073](../decisions/0073-p4-function-values-and-closures.md): function call effects stay in `fn-ty`; no row polymorphism.
5. **Testing conventions.** Resolved for Phase 4 by [ADR 0075](../decisions/0075-phase-4-frozen.md): ADR 0043 plus `.tacd` sidecars remained sufficient; no new test construct was required.

**Cross-phase / project-level:**

6. **License.** Permissive (MIT/Apache-2.0) or copyleft. Still open; low urgency until external distribution is on the table.

---

## Risk Register

**Risk: Nobody can write Tacit without AI assistance.**
Mitigation: This is by design for the long term, but early development needs human contributors. `tacit view` shipped in Phase 1 with type/effect annotations added in Phase 2; Phase 5 extends it. Keep Tacit-Lite semantics close enough to Rust that a human can reason about it with effort.

**Risk: AI models don't learn Tacit well from primers alone.** *Resolved by Phase 3 and not regressed by Phase 4.* Sonnet hit 97.9% library-mediated and GPT-5.4 91.5% primer-only on the open corpus per [ADR 0070](../decisions/0070-p3-frozen.md). Phase 4's expanded surface reached 47/47 final after repair on the open corpus per [ADR 0075](../decisions/0075-phase-4-frozen.md). Re-open only if later language work materially degrades fluency.

**Risk: LLVM churn or breaking changes.**
Mitigation: Pin LLVM version (LLVM 19 via `inkwell` 0.9 per [ADR 0032](../decisions/0032-stage-4-frozen.md)). Bumps are deliberate release-engineering tasks.

**Risk: Scope creep toward Tacit-Full before Tacit-Lite is solid.**
Mitigation: Discipline. Phase 7 is explicitly stretch. Do not start refinement types before Phase 6 is complete.

**Risk: Effect system creep in Phase 4.** *Resolved by Phase 4.*
Mitigation: [ADR 0073](../decisions/0073-p4-function-values-and-closures.md) kept closure call effects inside `fn-ty` and the existing fixed lattice ([ADR 0035](../decisions/0035-p2-effect-set-canonical.md)). Row polymorphism, handlers, and user-defined effects remain Phase 7.

**Risk: Record-first products do not address the Phase 3 structural gap.** *Partly materialized in Phase 4.*
Mitigation: [ADR 0072](../decisions/0072-p4-record-products.md) defers tuple syntax rather than rejecting it permanently. [ADR 0075](../decisions/0075-phase-4-frozen.md) records that records plus closures and combinators improved fluency but did not improve Rust-relative density under the current metric; re-open tuple syntax only with specific corpus evidence and a metric ADR.

**Risk: View system treated as UI instead of core infrastructure.**
Mitigation: Two views from Phase 1, both real. Phase 5's tooling work generalizes the existing view system; it does not retrofit one.

**Risk: Phase 3's structural findings don't translate into Phase 4 wins.** *Partly materialized in Phase 4.*
Mitigation: [ADR 0075](../decisions/0075-phase-4-frozen.md) records the result: records + closures + combinators improved open-corpus fluency and repair efficiency, and reduced generated authoring output to 2.85× Rust after repair when primer is excluded. They did not reduce Rust-relative density under the current end-to-end metric. Future density work must start with a metric ADR separating primer cost, generated authoring output, canonical storage size, and reference size rather than adding more Phase 4 surface.

**Risk: Nobody uses it.**
Mitigation: Accept this. The stated worst case is "waste tokens and have fun." Publishing a design paper is a valid outcome even if nobody adopts the language.

---

## Success Criteria

**Minimum viable success:** *Achieved.* Phase 3 closed with a working compiler, a primer, and frontier-model fluency on Tacit-Lite (97.9% Sonnet library-mediated, 91.5% GPT-5.4 primer-only) per [ADR 0070](../decisions/0070-p3-frozen.md). The primer-only thesis is empirically established; the artifact is publishable as-is.

**Reasoning-support success:** *Partly achieved.* Phase 4 is complete, with records, closures, and higher-order combinators landed, and Phase 3 fluency materially improved under the expanded surface. Rust-relative density did not measurably narrow from the Phase 3 baseline under the current end-to-end metric; [ADR 0075](../decisions/0075-phase-4-frozen.md) records that as a strategic finding rather than a reason to resume Python-relative density chase.

**Strong success:** Phase 6 complete, with Tacit-Lite within 20% of hand-written Rust on standard benchmarks and a Phase 4-era Rust-density aspiration met (e.g., ≤ 1.5× Rust on the corpus). Publishable with comparative benchmarks.

**Ambitious success:** Phase 7 or 8 complete, demonstrating that AI-first languages can offer genuinely new capabilities (proof-carrying code at scale, or self-hosting without human maintainers).
