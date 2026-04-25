# GPT-5.4 Review of Tacit

Originally written 2026-04-24 after reviewing the repository as it existed then. Updated 2026-04-25 after Phase 1 Stage 2 reached a natural stopping point. For the update I re-read the affected plans, ADRs, and `crates/tacit-views/`, and reran `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `uv run corpus-run`, and `uv run corpus-verify-sealed`.

## Overall Take

This is one of the more credible AI-first language repositories I have seen at this stage. The most convincing thing here is not the syntax experiment by itself. It is the discipline around it: frozen canonical bytes, two independent canonicalizers, a decision log that actually carries design weight, and a sealed evaluation corpus with runnable references. That makes Tacit feel like a falsifiable research artifact rather than a vague "LLM-native language" concept.

My strongest independent opinion is that the durable idea here is `AST-first + multiple lossless views + content-addressed identity`, not "the exact densest token surface for today's tokenizer mix." The former can benefit GPT, Claude, Gemini, Llama, Qwen, and future models because it removes representational ambiguity. The latter is useful, but more likely to drift as tokenizers and model priors change.

Compared to the 2026-04-24 snapshot, the important change is that the view layer is now materially real. `tacit-views` is in the workspace, root `cargo test` exercises it, and the authoring parser/emitter are no longer speculative. That shifts my main caution from "the views are outside the repo's truth surface" to "the written Stage 2 contract, CI cleanliness, and docs now need to catch up to the implementation that exists."

## Findings

1. High: Stage 2 is now real code, but the current "done" marker overstates the round-trip surface.

   The previous highest-severity issue is resolved in substance: `crates/tacit-views` is now a workspace member (`Cargo.toml:1-3`), `src/authoring/emit.rs` exists, and root `cargo test` exercises the view crate. The remaining truthfulness gap is the written Stage 2 exit gate. `plans/phase-1-plan.md:79-84` says the authoring-view round-trip property should hold on every Phase 0 test vector, but `crates/tacit-views/tests/round_trip.rs:35-54,72-74` deliberately skips 14 fixtures covering symbol-edge field names, holes, open terms, and `module`. Some of those exclusions are also codified in the spec itself: `plans/candidates/authoring-bpe-compact.md:160,170,177-179` explicitly defers `module` syntax and accepts lossy hole round-trip. My read is that Stage 2 now exists, but the written contract and the implemented/accepted subset are not yet aligned.

2. High: the workspace currently fails its own Rust formatting gate.

   CI is configured to run `cargo fmt --check` from the repo root (`.github/workflows/ci.yml:52-59`), but `cargo fmt --check` currently reports diffs in `crates/tacit-views/src/authoring/emit.rs:42-45,58-61`, `crates/tacit-views/src/authoring/lex.rs:68-80`, `crates/tacit-views/src/authoring/parse.rs:40-43`, `crates/tacit-views/src/sidecar.rs:50-52`, and `crates/tacit-views/tests/round_trip.rs:76-80`. `cargo test` and `cargo clippy` both pass, so this is not a semantic blocker, but on a repo that treats CI as a truth surface, missing the already-declared formatting gate is still a real inconsistency.

3. Medium: documentation drift is narrower than in the previous review, but it is not gone.

   The good news is that one stale-reference example is fixed: `plans/candidates/reference-ast.md` no longer points at a missing scoring script because `tools/q1-scoring/score_claude.py` now exists. The remaining drift is still worth fixing. `plans/tacit-plan.md:53-57` says Tacit-Lite deliberately excludes `Exn`, while `docs/effect-system.md:15-23` still includes `Exn` in the Lite lattice. And active Phase 1 docs still point at removed Phase 0 Rust paths in `decisions/0023-hole-node-recovery-deferred.md:13-19` and `plans/phase-1-plan.md:278-279`.

4. Medium: the evaluation corpus is real, but the repository still overstates what CI enforces around it.

   The artifact quality remains strong: `corpus/MANIFEST.md:3-4,104-109` says the corpus is 60/60 implemented, `uv run corpus-run` still passes `638/638` checks across 47 open tasks, and `uv run corpus-verify-sealed` still confirms the sealed manifest. The inconsistency now is mostly about enforcement language. `corpus/README.md:105-106` says CI runs `corpus-verify-sealed` on every push, but `.github/workflows/ci.yml:1-59` still has no corpus job. The same README then softens that at `corpus/README.md:137-139` by saying the workflow is only "expected to gain" those checks. The repo needs one story here.

5. Medium: the architecture is more model-neutral than the repo's framing, but the framing is still noticeably Anthropic-shaped.

   The load-bearing pieces are broadly useful: canonical AST storage, deterministic projections, DeBruijn normalization, typed holes, and content addressing are not vendor-specific ideas. But the public framing still leans Claude/Anthropic in important places. `plans/tacit-plan.md:62-63,223-225,377` uses Sonnet/Haiku/Opus as the main capability yardsticks, and `plans/candidates/reference-ast.md:154,326` plus `plans/candidates/authoring-bpe-compact.md:81` still talk as if Claude is the decisive production target. If the claim is truly "benefits all AI models," the repo should start expressing success in more model-family-neutral terms.

6. Medium: the current corpus still proves "small task solving" better than it proves the larger AI-first language thesis.

   Per `corpus/README.md:59-63`, every task is a stdin/stdout program, and the manifest is dominated by short arithmetic, string, collection, algorithm, and I/O exercises. That remains a good Phase 0 choice for falsifiable evaluation. But it still does not test the strongest Tacit-specific claim: that canonical AST identity plus lossless views improves long-horizon editing, repair, explanation, and multi-step manipulation. I would treat the current corpus as strong evidence for language-shape experiments, not yet as strong evidence for full workflow superiority.

## Resolved Since Previous Pass

- The old "`tacit-views` is outside the build" finding is resolved. `Cargo.toml:1-3` now includes `crates/tacit-views`, `crates/tacit-views/src/authoring/emit.rs` exists, and root `cargo test` exercises the view crate.
- The view layer is no longer merely planned. `crates/tacit-views/tests/round_trip.rs` now contains real canonical <-> authoring regression tests.
- The missing scoring-script example is resolved. `tools/q1-scoring/score_claude.py` now exists, so that specific stale-reference complaint no longer stands.

## What You Have Built Well

- The canonical layer is still the repo's strongest asset.
  `plans/canonical-text-format.md`, `decisions/0005` through `0013`, `crates/tacit-canonical/`, and `impls/py-canonicalizer/` together form a credible spec-plus-conformance story. Two independent implementations plus shared vectors is the right way to freeze a representation.

- Stage 2 is now a live artifact, not just a plan.
  `Cargo.toml`, `crates/tacit-views/`, and the round-trip tests mean the projection story is no longer only aspirational. Even with the remaining contract mismatch, this is a meaningful increase in credibility over the previous snapshot.

- The ADR discipline is unusually good.
  Most speculative language repos have ideas. This repo has decision boundaries. That matters a lot when using strong models to help drive design, because otherwise the project turns into stylistic drift.

- The split between canonical, authoring, and inspection views is still the right abstraction.
  I think this is the most durable part of Tacit. Even if the exact authoring syntax changes later, the separation itself is load-bearing and worth defending.

- The corpus work is serious.
  The sealed/open split, reference implementations in two languages, token-count harness, and explicit anti-contamination rules make the evaluation plan far more believable than "we'll see if the model feels better at it."

## My Independent Opinion

If I strip away the repo's current Anthropic/Opus origin story, I think the core project is already broader than that story. The generalizable insight is not "Claude likes this syntax." The generalizable insight is "models do better when program identity, binding structure, and projection rules are explicit, deterministic, and machine-friendly."

That is a real idea. It should help GPT-class models too. It should probably help open-weight models even more, because weaker models tend to suffer more from representational ambiguity and syntax noise.

Where I am more skeptical is the degree of emphasis on token-density as the central win condition. I do think density matters. I do not think it is the deepest moat here. If Tacit eventually matters, I expect it to be because it makes program manipulation more reliable, diffable, canonical, and repairable, not because it wins a tokenizer footrace forever. Tokenizer-specific optimization is worth doing, but I would treat it as one optimization layer on top of the more durable semantic design.

I also think the repo is right to be harsh about scope. The fastest way to lose this project would be to chase Tacit-Full research prestige before Tacit-Lite has a boring, correct, round-tripping end-to-end toolchain.

## What I Would Do Next

1. Align the Stage 2 contract with the supported subset.
   Either make the round-trip surface cover the skipped vectors, or explicitly narrow the Stage 2 exit gate and related prose to exclude holes, open terms, `module`, and authoring-ident syntax edge cases. The bad state is not the current subset; it is the mismatch between "done" and what "done" means.

2. Make the workspace actually CI-clean.
   Run rustfmt on `tacit-views` and make sure the root Rust workflow passes as written.

3. Finish the doc cleanup.
   Fix the `Exn` contradiction and remove the remaining `impls/rs-canonicalizer` references from active Phase 1 docs and ADRs.

4. Make the corpus enforcement story true.
   Either add `corpus-run` and `corpus-verify-sealed` to CI, or rewrite the docs so they describe the current state rather than the intended state.

5. Define "benefits all models" with a cross-model evaluation matrix.
   I would add explicit evaluation targets for at least one GPT-family model, one Claude-family model, and one strong open-weight model. Measure more than token count: compile success, repair success after errors, round-trip stability, and explanation/debug quality.

6. Add a small second evaluation track for maintenance tasks.
   Keep the current corpus, but add a handful of edit/repair/refactor tasks on slightly larger programs. That is where Tacit's canonicalization thesis can separate itself from "just another compact syntax."

## Verification Notes

For the 2026-04-25 update, I did not just read the plans. I also reran the current validation surface that exists today:

- `cargo test` at repo root: passed, now including `tacit-views` tests.
- `cargo clippy --all-targets -- -D warnings` at repo root: passed.
- `cargo fmt --check` at repo root: failed with formatting diffs in `crates/tacit-views/`.
- `uv run corpus-run` in `corpus/harness`: `638/638` passed across 47 open tasks.
- `uv run corpus-verify-sealed`: sealed tree matched the manifest.
- For this update I did not re-run `uv run pytest` in `impls/py-canonicalizer` or `uv run corpus-tokens`; those were part of the 2026-04-24 pass.
- I did not inspect the held-out task contents under `corpus/sealed/`; I only verified the manifest and tooling around them.

## Bottom Line

My opinion remains positive. You have built the beginnings of a real research artifact, not a slogan. The repository still contains one genuinely strong contribution: a disciplined, test-backed, canonical representation strategy for AI-authored code.

The important update is that the implementation surface is now more real than it was in the previous pass. The caution has shifted accordingly. The main problem is no longer "the views are not integrated." It is that the declared Stage 2 contract, CI cleanliness, and remaining docs now need to catch up to the implementation that exists. If you keep the repo honest at those boundaries, I still think Tacit has a real chance to matter beyond the Opus/Anthropic context that helped start it.
