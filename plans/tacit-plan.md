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
- File extension: `.tac` (canonical text) with optional `.tacd` sidecar for display metadata (names, comments).
- Binary storage is explicitly deferred. If profiling later shows parse overhead is load-bearing, a derived binary cache can be added without changing the canonical form.

### File organization

Files are purely a human convenience; layout carries no semantic weight. Moving a definition between files changes no hashes and breaks no imports. This makes reorganization semantically free — a property worth exploiting rather than worrying about.

- **v0 default: one `.tac` file per project.** Simplest option; the compiler truly doesn't care. Defensible for the small codebases Phase 1–3 will produce.
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

Exit criteria: Sonnet achieves > 70% pass rate on a defined task corpus using only the primer in context, AND end-to-end token usage is at least 30% lower than equivalent Python.

### Phase 4: Inspection and debugging tooling

**Goal:** Make Tacit debuggable by AI and inspectable by humans.

Deliverables:
- `tacit-view foo.tac` — render AST in any registered view (authoring, inspection, and future views like data-flow or dependency)
- `tacit-debug` — **AI-first CLI debugger**: step through execution, inspect values and types at any AST node, emit structured JSON output designed for AI consumption rather than human terminal readability
- `tacit-diff` — structural diff over AST (ignores cosmetic renames, node ID reshuffling)
- `tacit-blame` — AST history traversal
- Git integration: `.gitattributes` config so standard git operations fall back gracefully on canonical text

Deferred to stretch: `tacit-merge` (semantic three-way AST merge). Collaborative development isn't a v0 concern, and multiple AI agents concurrently editing the same file isn't a current use case.

Exit criteria: An AI agent can diagnose a failing Tacit program end-to-end using only `tacit-debug` output; a human can read diffs and inspect state through `tacit-view`.

### Phase 5 (conditional): Synthetic training corpus

**Goal:** Generate large-scale aligned pairs for fine-tuning and evaluation.

**Conditional on Phase 3 outcome.** The primary bet is primer-only prompting. If Phase 3 hits its >70% pass-rate target with the primer alone, Phase 5 is deferred indefinitely; fine-tuning becomes a long-term goal only if the project sees public success. If the primer approach falls short, Phase 5 becomes urgent.

Deliverables (if undertaken):
- Rust-to-Tacit-Lite rule-based transpiler (deterministic, not LLM-based)
- Corpus of ~1M aligned Rust/Tacit-Lite pairs from public Rust codebases
- Secondary corpus from Haskell / OCaml (~200K pairs)
- Quality metrics: percentage of pairs that round-trip correctly, percentage that compile, percentage that preserve behavior under test
- Held-out evaluation set separate from any fine-tuning corpus

Exit criteria: Corpus is large enough and clean enough that fine-tuning experiments become viable.

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
- `tacit-merge` — semantic three-way AST merge (pulled forward from Phase 4's deferred list)
- Conflict resolution heuristics at the AST node level
- Integration with review tooling (GitHub-like interfaces that render views of diffs)

Prerequisites: real use cases justifying the complexity. Not a v0 concern.

---

## Open Questions

These need answers before or during Phase 0:

1. **Authoring view format.** S-expressions over integer IDs? Single-glyph operators? Tokenizer-specific encoding optimized for a target model's BPE? Must round-trip losslessly with the canonical storage view. This is the single most consequential Phase 0 decision — primer, eval harness, and model fluency all depend on it.
2. **Effect polymorphism surface syntax.** How effect variables appear in signatures, especially in higher-order functions; how effect mismatches are rendered in the inspection view.
3. **Scope of libc wrappers for v0.** *Resolved 2026-04-24 by [ADR 0025](../decisions/0025-phase-1-libc-surface.md):* Phase 1's libc surface is three OS-boundary symbols (`write`, `read`, `exit`); pure-compute libc functions are not used. Effect signatures live in `stdlib/libc-effects.toml` as a dormant table for Phase 2's checker. The broader architectural framing — pure computational kernel with in-language stdlib, ecosystem-library impurity quarantined to the host — is settled by [ADR 0022](../decisions/0022-pure-kernel-host-model.md); libc remains the stdlib's backing implementation until Phase 10's scratch stdlib replaces it with direct syscalls.
4. **Testing conventions.** How are tests expressed in Tacit? As regular functions with a marker, or as a separate construct?
5. **Metadata sidecar format.** JSON? A separate canonical-text format? How tightly coupled to the `.tac` file?
6. **License.** Permissive (MIT/Apache-2.0) or copyleft? Affects corpus choices if Phase 5 is undertaken.
7. **Target tokenizer for authoring view optimization.** Optimize for a specific model family (Claude's tokenizer, GPT's tiktoken) or aim for tokenizer-agnostic density? A specific target yields sharper wins but creates a dependency.

---

## Risk Register

**Risk: Nobody can write Tacit without AI assistance.**
Mitigation: This is by design for the long term, but early development needs human contributors. Ship `tacit-view` early (Phase 4) so humans can inspect at least. Keep Tacit-Lite semantics close enough to Rust that a human can reason about it with effort.

**Risk: AI models don't learn Tacit well from primers alone.**
Mitigation: Phase 3 measures this directly. If baseline performance is poor, Phase 5's synthetic corpus becomes urgent. Worst case, the project becomes a fine-tuning project rather than a prompting project.

**Risk: LLVM churn or breaking changes.**
Mitigation: Pin LLVM version. The `inkwell` crate handles a lot of this.

**Risk: Scope creep toward Tacit-Full before Tacit-Lite is solid.**
Mitigation: Discipline. Phase 7 is explicitly stretch. Do not start refinement types before Phase 6 is complete.

**Risk: Effect system creep in Phase 2.**
Mitigation: Phase 2 is scoped to simple effects — fixed lattice, basic polymorphism, no handlers. Effect systems are notorious for nerd-sniping compiler authors into research-grade complexity (Koka took years to nail handler-style effects). If we find ourselves designing row polymorphism or user-defined effects, stop and move it to Phase 7.

**Risk: View system treated as UI instead of core infrastructure.**
Mitigation: Two views in Phase 0/1, not one. If the authoring view is the only thing implemented and the inspection view is postponed, it effectively *becomes* the canonical form and the view abstraction rots. Keep both real from the start, even if the inspection view is minimal.

**Risk: The token savings don't materialize in practice.**
Mitigation: Phase 3's evaluation harness measures real tokens on real tasks. If savings are less than 30% vs equivalent Python, reconsider the project's premise before continuing.

**Risk: Nobody uses it.**
Mitigation: Accept this. The stated worst case is "waste tokens and have fun." Publishing a design paper is a valid outcome even if nobody adopts the language.

---

## Success Criteria

**Minimum viable success:** Phase 3 complete, with a working compiler and a primer that lets Sonnet write Tacit-Lite competently on a defined task set. This alone is publishable as a research artifact.

**Strong success:** Phase 6 complete, with measured token savings over Python and at-parity performance with Rust. Publishable with comparative benchmarks.

**Ambitious success:** Phase 7 or 8 complete, demonstrating that AI-first languages can offer genuinely new capabilities (proof-carrying code at scale, or self-hosting without human maintainers).
