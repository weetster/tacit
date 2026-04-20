# 0002 — License

**Status:** Accepted
**Date:** 2026-04-20
**Phase:** 0, Stage 1

## Context

The parent plan (tacit-plan.md § Open Questions, Q6) flagged the license choice as something to settle early, partly because it affects corpus choices if Phase 5 (synthetic training corpus) is undertaken. Choosing early also keeps the repo clean: every file added under an implicit license is a small amount of future cleanup.

Tacit is a language and compiler intended for broad adoption. The license should not create friction for corporate users or for integration with existing open-source ecosystems.

## Decision

**Dual-license under MIT OR Apache-2.0**, matching the convention used by Rust, Cargo, and most of the modern systems-programming ecosystem. Users may choose either license.

Add `LICENSE-MIT` and `LICENSE-APACHE` files to the repository root. Reference both in any future `Cargo.toml` as `license = "MIT OR Apache-2.0"`.

Contributor intent follows the Rust/Apache convention: contributions submitted for inclusion are implicitly dual-licensed unless explicitly stated otherwise. Document this in `CONTRIBUTING.md` when that file is added (Stage 5).

## Alternatives considered

- **MIT alone.** Short and maximally permissive, but silent on patents. For a project that may attract corporate contributors, the lack of an explicit patent grant creates ambiguity both for the contributors (risk of later patent claims covering their own contributions) and for users (risk of exposure to contributor patents). Rejected.
- **Apache-2.0 alone.** Has the patent grant but is incompatible with GPL-2.0 (without the later-version clause). Rules out reuse in GPL-2.0-only codebases. Rejected in favor of dual-licensing, which preserves the patent protection while allowing GPL-2.0 projects to consume Tacit via the MIT option.
- **GPL / LGPL / MPL (copyleft).** Copyleft on a compiler is a well-known adoption blocker — depending on interpretation, it can infect compiled programs or runtime components. Incompatible with Tacit's goal of broad adoption. Rejected.
- **Proprietary / source-available.** Out of character for a research-oriented language project. Not seriously considered.

## Consequences

- All source files in the repository are covered by MIT OR Apache-2.0 from the start. No retroactive relicensing is ever needed.
- Contributions from any source (including corporate contributors) have a clear patent position via the Apache-2.0 option.
- GPL-2.0 projects can consume Tacit via the MIT option.
- **Phase 5 implication:** if a synthetic training corpus is built from public Rust code, license compatibility must be checked per-source. Rust code on crates.io is predominantly MIT / Apache-2.0 / dual and therefore compatible; GPL Rust code cannot be used as training input without careful analysis. This does not change Tacit's own license; it constrains the corpus pipeline.
- No additional legal review is expected to be required. The dual-license combination is well-trodden.
