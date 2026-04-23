# 0018 — Stage 5 repository scaffolding frozen

**Status:** Accepted
**Date:** 2026-04-23
**Phase:** 0, Stage 5 (exit)
**Supersedes:** None

## Context

Stage 5 of Phase 0 ([phase-0-plan.md § Stage 5](../plans/phase-0-plan.md)) is the repo-scaffolding stage. Its listed deliverables in the plan are:

> - Cargo workspace layout, CI (fmt / clippy / test), issue templates
> - Decision log (`decisions/NNNN-title.md` ADR format)

Stage 5 is labeled "parallel, low-effort" and is the smallest remaining item before Phase 0 exits to Phase 1. The Stage 3 freeze ([ADR 0017](0017-stage-3-frozen.md)) left Stage 4 (the evaluation corpus) as the only other remaining Phase 0 item, and Stage 4 is the longer stretch — closing Stage 5 first lets corpus authoring happen against a stable CI and issue-reporting baseline.

### What was actually needed

Two of the four Stage 5 items landed earlier than planned and don't need re-doing at the exit:

- **Decision log.** Established in Stage 1 (ADRs 0001–0004) and grown continuously since; by the time Stage 5 began, 17 ADRs existed and `decisions/README.md` indexed them. No additional work here beyond adding 0018 itself.
- **Cargo workspace layout.** Explicitly deferred to Phase 1 by [ADR 0016](0016-rust-ast-enum-location.md) — "No Cargo workspace or `crates/tacit-ast` in Phase 0." The "Cargo workspace layout" phrase in the Stage 5 deliverable list is superseded by ADR 0016. See the Alternatives section for why re-doing this early would violate [CLAUDE.md § Ground rules](../CLAUDE.md).

That left two items to execute:

- **CI** — `fmt / clippy / test` for Rust, `pytest` for Python, running on push and PR.
- **Issue templates** — bug / spec-ambiguity / ADR-proposal, matching the actual issue kinds Phase 0 produces.

### Work done for this freeze

**CI** landed at [`.github/workflows/ci.yml`](../.github/workflows/ci.yml) — two jobs on `ubuntu-latest`:

- `py-canonicalizer` — installs `uv`, runs `uv sync --extra dev` and `uv run pytest` from `impls/py-canonicalizer/`. 54 tests.
- `rs-canonicalizer` — installs the Rust stable toolchain with `rustfmt` + `clippy`, caches cargo artifacts keyed on `Cargo.lock`, runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` from `impls/rs-canonicalizer/`. 14 tests.

Cross-impl byte-equivalence is verified transitively: both jobs run their test suites against the shared fixture directory [`plans/test-vectors/`](../plans/test-vectors/), so if either impl diverges from the committed fixture bytes, CI fails. This preserves the [ADR 0013](0013-canonical-text-format-frozen.md) byte-equivalence gate going forward.

**Minor code changes were required to make the Rust CI checks pass:**

- `cargo fmt` applied its default layout to `impls/rs-canonicalizer/src/ast.rs`, `impls/rs-canonicalizer/src/lex.rs`, `impls/rs-canonicalizer/src/parse.rs`, `impls/rs-canonicalizer/tests/emission_rules.rs`, and `impls/rs-canonicalizer/tests/vectors.rs`. Struct-variant expansion grew `ast.rs` from ~34 lines to ~90. [ADR 0016](0016-rust-ast-enum-location.md)'s context snippet was updated to describe the enum textually instead of inlining the code, so it no longer claims a line count that will drift.
- One `cargo clippy` warning on `parse.rs` (new `manual_is_multiple_of` lint in Rust 1.93) was fixed: `args.len() % 2 != 0` → `!args.len().is_multiple_of(2)`. This ties the canonicalizer's MSRV to Rust 1.93+, which is acceptable because Phase 0 has no public release and the Phase 1 workspace promotion will re-visit MSRV.

**Issue templates** landed at `.github/ISSUE_TEMPLATE/`:

- `bug.yml` — canonicalizer or tool misbehavior (required: component, minimal input, expected, actual, commit).
- `spec-ambiguity.yml` — spec under- or contradictorily-specified (required: doc section, ambiguity, impact; optional suggested resolution).
- `adr-proposal.yml` — new decision needed (required: question, context, options considered; optional recommendation; required target phase).
- `config.yml` — points at `decisions/README.md` for the ADR index, blank issues still enabled.

## Decision

**Stage 5 is frozen.** The Phase 0 repository scaffolding is complete:

1. **CI at [`.github/workflows/ci.yml`](../.github/workflows/ci.yml)** enforces `fmt / clippy / test` for Rust and `pytest` for Python on every push to main and on every pull request. The cross-impl byte-equivalence gate from [ADR 0013](0013-canonical-text-format-frozen.md) is preserved transitively through the shared fixture directory.
2. **Issue templates at [`.github/ISSUE_TEMPLATE/`](../.github/ISSUE_TEMPLATE/)** structure incoming reports into the three kinds that Phase 0 actually produces (bug, spec ambiguity, ADR proposal).
3. **Decision log at [`decisions/`](README.md)** with the ADR format documented in [`decisions/README.md`](README.md) is the authoritative record of design choices and is now indexing 18 ADRs including this one.
4. **Cargo workspace is deferred to Phase 1** per [ADR 0016](0016-rust-ast-enum-location.md); this deferral remains in force.

Changes to CI, issue templates, or the decision-log format after this freeze are not governed by ADR discipline — they are ordinary repo maintenance. Only changes that would affect Stage 2/3 freeze invariants (e.g., disabling the fixture-based equivalence test) would warrant an ADR.

## Alternatives considered

- **Stand up the Cargo workspace now, ahead of Phase 1.** Rejected for the same reasons [ADR 0016](0016-rust-ast-enum-location.md) rejected it: Phase 0 has one consumer of the AST (the Rust canonicalizer itself), and a workspace + shared crate + publish/versioning story are scaffolding for consumers that don't exist yet. [CLAUDE.md § Ground rules](../CLAUDE.md) ("Don't add compiler scaffolding to save time later") applies directly. Phase 1 will spin up the workspace with full knowledge of what the parser and emitter want.

- **Add a root-level `Makefile` / `justfile` for convenience.** Rejected. Two canonicalizers with their native build tools (`uv run pytest`, `cargo test`) is simple enough that a thin wrapper mostly adds a second thing to remember. Can be added in Phase 1 when more tools enter the picture.

- **Matrix CI across Linux/macOS/Windows.** Rejected for Phase 0. Canonicalizer output is byte-exact by spec, not platform-dependent, so single-platform (ubuntu-latest) suffices. The Python 3.14+ requirement and Rust 1.93+ requirement would make Windows runners marginally annoying to maintain without any Phase-0 benefit.

- **Ship CI without `clippy -D warnings`.** Rejected. A single warning fix (the `manual_is_multiple_of` lint) was required to turn it on, and enforcing from the start costs nothing extra. Relaxing to `-W warnings` later would be a one-line edit if some future lint is deemed too aggressive.

- **Make CI run the cross-impl byte-equivalence check explicitly** (parse-and-compare script in a third job). Rejected. The transitive check — both impls against the same fixtures — has identical semantics and avoids a third job's maintenance surface. If Phase 1 adds fixture generation to either impl, the explicit cross-check can be added then.

## Consequences

- **Every commit to main and every PR exercises the Stage 2 byte-equivalence gate.** Regressions that break the fixture contract fail CI before they land, matching the intent of [ADR 0013](0013-canonical-text-format-frozen.md).

- **Rust code must be `cargo fmt --check` + `clippy --all-targets -- -D warnings` clean.** First-time contributors adding a file hit this immediately; the rule is in CI, not a reviewer concern.

- **Phase 0 exit is now gated only on Stage 4** (evaluation corpus). With Stage 5 frozen, corpus authoring can proceed against a stable baseline — CI catches canonicalizer regressions as the corpus grows.

- **The canonicalizer's minimum Rust version is pinned at 1.93+** via the `is_multiple_of` clippy-suggested fix. Downgrade requires a `#[allow]` attribute or reverting to the `%` form. Phase 1 workspace setup will make MSRV an explicit `Cargo.toml` field.

- **ADR 0016's inlined enum snippet was removed** in favor of a textual description plus a pointer at the source file, so future `cargo fmt` runs don't silently stale the ADR.

- **Issue-template YAML parses client-side at issue-creation time**, so subtle syntax errors won't surface until an issue is filed. An informal test: confirm each template renders in GitHub's new-issue UI after the first push to main. Not a blocker for freeze — if any template fails to render, it's a trivial fix.

- **Phase 0 has fully-formed scaffolding without any Phase 1 code.** The directory layout is compatible with later workspace promotion (e.g., moving `impls/rs-canonicalizer/` under `crates/` alongside new crates) without touching CI semantics.

## Related decisions

- [ADR 0013](0013-canonical-text-format-frozen.md) — the byte-equivalence gate that CI now enforces continuously.
- [ADR 0016](0016-rust-ast-enum-location.md) — Cargo workspace deferral, referenced here.
- [ADR 0017](0017-stage-3-frozen.md) — Stage 3 freeze that preceded this one; Stage 5 closure leaves Stage 4 as the last Phase 0 item.
- [phase-0-plan.md § Stage 5](../plans/phase-0-plan.md) — deliverable list this ADR closes; Stage 5 status flips to **Frozen** concurrently.
- [CLAUDE.md § Ground rules](../CLAUDE.md) — "No Phase 1 work," which constrains this ADR's scope away from workspace promotion.
