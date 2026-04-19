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
- **Source-as-text** — programs are serialized ASTs; no lexer, no parser, no syntax errors possible

### What we keep (genuinely load-bearing)

- **Type definitions** — structural types are compression, not decoration
- **Module boundaries** — define compilation units and capability boundaries; decoupled from filesystem

### What we add (miserable for humans, great for AI)

- **Refinement types** — every value carries verified constraints (e.g., `int where 0 ≤ x < len(array)`)
- **Effect tracking** — every function's type includes its effect set; no hidden side effects
- **Capability-based security** — privileged operations require explicit capability tokens; no ambient authority
- **Content-addressing** — every function, type, and value identified by cryptographic hash; names are hints
- **Explicit evaluation order and memory layout** — no compiler magic; optimizations are local rewrites with provable equivalence
- **Versioned semantics at the expression level** — operators and stdlib functions carry version tags
- **Progressive disclosure of type info** — minimal signature by default, full spec on demand

### Two variants

- **Tacit-Lite** — compressed AST representation without the heavy correctness machinery. Smaller models (Sonnet/Haiku class) can write this proficiently. Pure token-efficiency play. This is the practical target.
- **Tacit-Full** — adds refinement types, proof obligations, full effect discipline, capability threading. Requires Opus-class models plus a specialized verifier (Z3 or similar). Research-grade; correctness-critical domains.

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

### Storage format: **Binary AST**

- Content-addressed nodes (BLAKE3 hashes, 32 bytes each)
- Integer variable IDs assigned by scope depth and usage frequency
- Huffman-style frequency-based encoding for common constructs
- File extension: `.tac` (binary) with optional `.tacd` sidecar for display metadata (names, comments)

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

### Phase 0: Foundations (weeks 1-3)

**Goal:** Concrete specification and tooling scaffolding before writing real code.

Deliverables:
- Formal grammar for Tacit-Lite AST (as a Rust enum hierarchy initially)
- Binary serialization format specification
- Content-addressing scheme specification
- Project repository structure, CI, issue tracking
- Decision log document for recording design choices and rationale

Exit criteria: Another developer could read the spec and build a parser for the binary format.

### Phase 1: Minimum viable compiler (weeks 4-12)

**Goal:** Compile a "hello world" equivalent from binary AST to running native executable.

Deliverables:
- Rust crate with AST data structures matching the Phase 0 spec
- Binary deserializer (reads `.tac` files into AST)
- LLVM IR emitter for basic constructs: integer arithmetic, function definitions, function calls, conditionals, loops
- Hand-crafted test `.tac` files (written as Rust code that constructs AST and serializes it)
- Simple CLI: `tacit compile foo.tac -o foo`
- Documentation of the compiler architecture

Deliberately out of scope: type checking, effects, capabilities, any optimization beyond what LLVM does automatically.

Exit criteria: Round-trip works — you can construct an AST in Rust, serialize it, compile the binary, run it, and see expected output.

### Phase 2: Type system (Tacit-Lite) (weeks 13-20)

**Goal:** Static type checking for the Lite variant.

Deliverables:
- Type inference for function signatures from bodies
- Structural type checking (no refinements yet)
- Basic generic types
- Structured error reporting format (JSON-emittable)
- Type-directed overload resolution for operators

Exit criteria: Non-trivial programs (sorting algorithms, basic data structures) typecheck and compile.

### Phase 3: AI authoring primer (weeks 21-24)

**Goal:** Enable an existing model to write Tacit-Lite in-context.

Deliverables:
- 8-15K token primer document structured as:
  - One-page semantic summary
  - Progressive examples (Python/Rust ↔ Tacit-Lite pairs) from trivial to complex
  - Idiom catalog (canonical form for each common pattern)
  - Negative examples with structured explanations
  - Compiler error catalog with fix patterns
- Evaluation harness: corpus of tasks with test cases, auto-check that generated Tacit compiles and passes tests
- Baseline measurements: Sonnet and Haiku performance with primer alone

Exit criteria: Sonnet achieves > 70% pass rate on a defined task corpus using only the primer in context.

### Phase 4: Version control tooling (weeks 25-28)

**Goal:** Make Tacit usable in collaborative development.

Deliverables:
- `tacit-diff` command implementing structural diff over AST
- `tacit-merge` command implementing semantic three-way merge
- `tacit-blame` command traversing AST history
- Git integration: `.gitattributes` config and driver registration scripts
- Inspection tool: `tacit-view foo.tac` renders AST as human-readable text (for debugging)

Exit criteria: A developer can clone a Tacit repo, view diffs, merge branches, and use GitHub's existing review infrastructure with reasonable fidelity.

### Phase 5: Synthetic training corpus (weeks 29-36)

**Goal:** Generate large-scale aligned pairs for fine-tuning and evaluation.

Deliverables:
- Rust-to-Tacit-Lite rule-based transpiler (deterministic, not LLM-based)
- Corpus of ~1M aligned Rust/Tacit-Lite pairs from public Rust codebases
- Secondary corpus from Haskell / OCaml (~200K pairs)
- Quality metrics: percentage of pairs that round-trip correctly, percentage that compile, percentage that preserve behavior under test
- Held-out evaluation set separate from any fine-tuning corpus

Exit criteria: Corpus is large enough and clean enough that fine-tuning experiments become viable.

### Phase 6: Optimization and hardening (weeks 37-44)

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
- Effect system and inference
- Capability tokens and enforcement
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

---

## Open Questions

These need answers before or during Phase 0:

1. **Exact binary format details.** Variable-length integer encoding scheme, endianness, hash algorithm (BLAKE3 preferred, but confirm), versioning of the format itself.
2. **Scope of the standard library for v0.** Minimal (just what Phase 1 needs) or broader?
3. **Module system specifics.** How do imports work? Is there a registry? How are versions resolved?
4. **Testing conventions.** How are tests expressed in Tacit? As regular functions with a marker, or as a separate construct?
5. **Metadata sidecar format.** JSON? A separate binary format? How tightly coupled to the `.tac` file?
6. **License.** Permissive (MIT/Apache-2.0) or copyleft? Affects corpus choices in Phase 5.

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

**Risk: The token savings don't materialize in practice.**
Mitigation: Phase 3's evaluation harness measures real tokens on real tasks. If savings are less than 30% vs equivalent Python, reconsider the project's premise before continuing.

**Risk: Nobody uses it.**
Mitigation: Accept this. The stated worst case is "waste tokens and have fun." Publishing a design paper is a valid outcome even if nobody adopts the language.

---

## Success Criteria

**Minimum viable success:** Phase 3 complete, with a working compiler and a primer that lets Sonnet write Tacit-Lite competently on a defined task set. This alone is publishable as a research artifact.

**Strong success:** Phase 6 complete, with measured token savings over Python and at-parity performance with Rust. Publishable with comparative benchmarks.

**Ambitious success:** Phase 7 or 8 complete, demonstrating that AI-first languages can offer genuinely new capabilities (proof-carrying code at scale, or self-hosting without human maintainers).
