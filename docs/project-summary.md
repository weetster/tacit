# Tacit Project Summary

This note is a compact foundation for a blog post. It is intentionally
conceptual rather than implementation-heavy.

## Core Concepts

Tacit is an AI-first programming language. It is designed around the idea
that the machine-facing representation should be the primary one, and that
human-oriented views should be derived from that representation instead of
competing with it.

### AST-first design

The abstract syntax tree is the source of truth. Tacit does not treat a
human-authored surface syntax as the canonical program form. That matters
because it removes ambiguity, makes transformations predictable, and lets the
compiler reason about structure directly.

Why it helps:
- fewer representation mismatches between tools
- easier hashing, comparison, and round-tripping
- less room for stylistic drift

### Canonical text and multiple views

Tacit uses a byte-exact canonical text format for authoritative storage, plus
separate lossless views for authoring and inspection.

How it works:
- canonical text stores the real program structure
- authoring view is dense and optimized for model generation
- inspection view is expanded and optimized for debugging and review
- sidecar data carries display metadata such as names and comments

Why it helps:
- AI systems can work with a compact form
- humans can still inspect and debug the same program clearly
- the compiler does not need to guess which surface spelling was intended

### Content-addressed identity

Functions, types, and values are identified structurally using BLAKE3 over
canonical text. Variable references use DeBruijn indices, so names are
presentation metadata rather than semantic identity.

Why it helps:
- identity stays stable across renaming
- code can be compared by structure, not by incidental labels
- modular tooling becomes simpler because the AST itself carries meaning

### Structural errors instead of hard parse failure

When parsing fails, Tacit can produce typed `Hole` nodes with structured
diagnostics instead of stopping at an opaque syntax error.

Why it helps:
- tools can continue operating on partial programs
- diagnostics are machine-readable
- repair loops become practical for model-driven authoring

### Explicit effects

Tacit-Lite tracks effects in function signatures. The current fixed lattice
covers `IO`, `Alloc`, `Mut`, and `Div`.

How it works:
- the compiler infers effects locally from function bodies
- higher-order functions carry callback effects through function types
- effect information is visible at the call site

Why it helps:
- code stays readable as a dependency graph of side effects
- the compiler can enforce behavior without whole-program analysis
- higher-order utilities like `map` and `fold` can remain effect-aware

### Tacit-Lite and Tacit-Full

Tacit-Lite is the practical shipping surface: structural types, explicit
effects, and single-threaded execution. Tacit-Full is the longer-term research
direction for refinement types, handlers, and richer effect systems.

Why the split matters:
- Lite can be built, tested, and reasoned about now
- Full keeps the more speculative work from blocking the core language

## Development Timeline

### April 19-21, 2026: project framing

The early work focused on representation choices: tokenizer target, canonical
form, hashing, and the initial effect-system direction.

Main roadblock:
- deciding whether Tacit should optimize for human readability or for AI-facing
  density and structural stability

How it was addressed:
- the project committed to an AST-first model with a compact authoring view and
  a separate inspection view
- canonical text and structural hashing were chosen early so identity would be
  deterministic

### April 21-26, 2026: Phase 1

Phase 1 built the canonicalization and code-generation baseline.

Main roadblocks:
- preserving round-trip fidelity while keeping the storage form canonical
- getting a practical LLVM-backed pipeline in place

How they were addressed:
- the repo adopted `.tac` as canonical text and sidecar metadata for display
- the compiler pipeline was wired through LLVM using `inkwell`, with LLVM 19
  pinned for repeatable builds
- the CLI exposed `compile` and `view` so the language could be exercised end
  to end

### April 26-28, 2026: Phase 2

Phase 2 added local type inference, structural typing, and the fixed effect
lattice.

Main roadblocks:
- making effects visible without turning the language into a research-only
  system
- handling malformed programs without forcing a full stop

How they were addressed:
- effects were kept to a small fixed set with local inference
- effect polymorphism was added only where higher-order functions needed it
- parser recovery was moved to typed hole nodes, backed by structured JSON
  diagnostics

### April 28-May 6, 2026: Phase 3

Phase 3 expanded the evaluation corpus, primer, and standard library surface.

Main roadblocks:
- keeping sealed evaluation material isolated from development feedback
- avoiding primer contamination from repository-specific wording
- a density target relative to Python turned out to be miscalibrated

How they were addressed:
- the held-out corpus was sealed with explicit guardrails
- the primer was rewritten to stay generic and prompt-facing
- the density gate was retired as structurally misleading, and future density
  work was retargeted against Rust instead
- repair-loop and cross-family evaluation harnesses were added to measure how
  model behavior changed under realistic iteration

### May 6-8, 2026: Phase 4

Phase 4 added record products, first-class function values, closures, and
compiler-recognized higher-order combinators.

Main roadblocks:
- supporting closures without breaking the fixed Lite effect story
- keeping non-escapable buffer and vector handles from being captured in unsafe
  ways

How they were addressed:
- closures were lowered as two-word closure pairs with minimized by-value
  captures
- call effects were tracked through function types
- invalid captures produced targeted diagnostics
- inspection support was extended so records, captures, and combinators remain
  understandable in the view layer

### May 8-9, 2026: freeze and closeout

Phase 4 was frozen, and Phase 5 became the next planned step.

Current status:
- the language surface is stable through Phase 4
- the next work is narrower validation and maintenance, not more feature
  expansion

## Good Source Files For a Blog Draft

- [README.md](../README.md)
- [CLAUDE.md](../CLAUDE.md)
- [docs/effect-system.md](effect-system.md)
- [docs/compiler-architecture.md](compiler-architecture.md)
- [plans/tacit-plan.md](../plans/tacit-plan.md)
- [decisions/0075-phase-4-frozen.md](../decisions/0075-phase-4-frozen.md)

