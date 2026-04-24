# Tacit

Tacit is an AI-first programming language: a language designed for models to read and write, not for humans to type by hand.

The project starts from a different assumption than mainstream languages. If human readability is not the primary constraint, a language can optimize for three things at once:

- token efficiency for AI generation and consumption
- runtime performance via strong compile-time guarantees
- safety and security through explicit, structural semantics

Tacit is intended to compile to LLVM IR and then native code.

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

- `plans/` - project vision and phase plans
- `docs/` - supporting design notes
- `decisions/` - ADR-style design decisions
- `corpus/` - evaluation corpus work

Start with:

- `plans/tacit-plan.md` for the full project vision
- `CLAUDE.md` for the working rules used in this repo
