# Tacit

Tacit is an AI-first programming language: a language designed for models to read and write, not for humans to type by hand.

The project starts from a different assumption than mainstream languages. If human readability is not the primary constraint, a language can optimize for three things at once:

- token efficiency for AI generation and consumption
- runtime performance via strong compile-time guarantees
- safety and security through explicit, structural semantics

Tacit compiles to LLVM IR and then native code. The current Tacit-Lite
compiler can parse, typecheck, inspect, compile, and execute the frozen Phase 4
language surface.

## What Makes Tacit Novel

- **The AST is the source of truth.** Tacit does not treat a human-oriented surface syntax as the authoritative program representation.
- **Programs have multiple lossless views.** A dense authoring view is optimized for AI token efficiency, while an inspection view is optimized for debugging and human review.
- **Canonical text is byte-exact.** Every valid AST has exactly one canonical serialization, which removes stylistic variance and formatter debates.
- **Definitions are content-addressed.** Functions, types, and values are identified by the BLAKE3 hash of their canonical text, so identity is structural rather than name-based.
- **Names are metadata, not identity.** Variable references use DeBruijn indices in canonical form; display names are advisory sidecar data.
- **Errors stay structural.** Malformed code becomes typed `Hole` nodes with structured diagnostics instead of opaque parse failures.
- **Effects are explicit.** Tacit-Lite tracks effects in function signatures so important behavioral facts remain visible without whole-program analysis.

## Design Direction

Tacit deliberately strips out many human conveniences:

- free-form formatting
- comments in source
- human-readable identifiers as semantic identity
- syntactic sugar and multiple spellings for the same construct
- prose-first error reporting

In exchange, it adds machinery that is useful for AI authoring:

- canonical AST storage
- purpose-built authoring and inspection views
- structural typing and explicit effect tracking
- content-addressed definitions and modules
- explicit recursion grouping and evaluation structure

The default target is **Tacit-Lite**, a smaller practical variant with structural types, simple effect tracking, and single-threaded execution. **Tacit-Full** is a longer-term research path that adds refinement types, capability-based security, and richer effect systems.

## Repository Guide

- `plans/` - project vision, phase plans, and frozen specs (canonical text format, inspection view, sidecar)
- `docs/` - supporting design notes (compiler architecture, effect system)
- `decisions/` - ADR-style design decisions (0001 onward)
- `crates/` - Cargo workspace: `tacit-canonical`, `tacit-views`, `tacit-typecheck`, `tacit-codegen`, `tacit-cli`
- `examples/` - smoke programs plus Phase 3 and Phase 4 examples
- `corpus/` - Phase 3 evaluation corpus, with sealed held-out subset
- `stdlib/` - libc effect signatures consumed by the typechecker

## Current status

Phase 4 is frozen by [ADR 0075](decisions/0075-phase-4-frozen.md). The
delivered Tacit-Lite surface includes:

- record product values with structural typing
- first-class function values and capturing closures
- closure effects inside the Lite effect lattice
- compiler-recognized `@map`, `@fold`, and `@for-each` over `I64Vec` prefixes
- inspection support for record types, closure captures, and combinator blocks
- structured diagnostics for the new Phase 4 failure modes

The Phase 4 open-corpus re-evaluation reached `38/47` one-shot task passes and
`47/47` final passes after repair. Generated authoring output improved to
`2.85x` Rust after repair when primer cost is excluded, but end-to-end
primer-plus-generation density did not improve because the Phase 4 primer
dominates the aggregate token count. That mixed result is part of the research
record, not a reason to add more Phase 4 language surface.

The next planned work is **Phase 5A**, a narrow validation phase for larger
maintenance/debugging tasks. Phase 5A should decide whether full inspection and
debugging tooling is worth building before committing to the broader Phase 5
debugger/diff/blame roadmap.

Start with:

- `plans/tacit-plan.md` for the full project vision
- `plans/phase-4-plan.md` and `decisions/0075-phase-4-frozen.md` for the frozen Phase 4 baseline
- `plans/phase-4-results/` for the latest open-corpus evaluation record
- `CLAUDE.md` for the working rules used in this repo
