# Project Q&A

This file collects concise answers to presentation, blog, and project-direction
questions that come up after Phase 4.

## Is Phase 4 a natural stopping point to share the project?

Yes. Phase 4 is a natural public checkpoint.

The project now has a concrete artifact and a defensible research result:

- Tacit-Lite parses, typechecks, inspects, compiles, and executes the Phase 4
  language surface.
- Records, closures, callback effects, and `@map`/`@fold`/`@for-each` work end
  to end.
- The open-corpus evaluation reached `38/47` one-shot task passes and `47/47`
  final passes after repair.
- LLM-facing generated authoring output improved when primer cost is excluded.
- End-to-end primer-plus-generation density did not improve, because primer
  cost dominates the current metric.

That is a stronger story than pretending the project is simply unfinished. The
honest result is that Tacit improved model fluency and repair behavior, but
token density is more complicated than language syntax alone.

The right near-term move is publication and validation, not immediate feature
expansion:

- update public docs and presentation material
- show the compiler, views, diagnostics, and evaluation result
- frame the density miss as a finding
- define Phase 5 as a validation phase for maintenance/debugging workflows
- avoid adding more Phase 4 primitives or primer prose without a new metric ADR

The fair claim is:

> Tacit is publishable after Phase 4 as a research artifact. It demonstrates
> that a primer-taught AI-first language can be learned and repaired by frontier
> models, while also showing that end-to-end token efficiency depends on the
> language, tokenizer, primer, training distribution, and tooling loop together.

## Would a custom LLM trained on a more token-efficient language be required to beat popular languages on token cost?

Mostly yes, with one important correction: custom model familiarity is probably
necessary for beating popular languages end to end, but it is not sufficient by
itself.

For Tacit-like languages, token cost splits into at least four quantities:

- **Generated source tokens:** how many tokens the emitted program uses.
- **Primer/tooling context tokens:** what the model must be taught each call.
- **Repair tokens:** failed attempts, diagnostics, and retries.
- **Tokenizer fit:** whether the tokenizer has good merges for the language's
  syntax and idioms.

Popular languages win because they have all four advantages: massive
pretraining exposure, tokenizer familiarity, no primer tax, and strong repair
fluency.

The fair claim is:

> A new AI-first textual language is unlikely to beat popular languages on
> end-to-end LLM token cost unless the model and tokenizer are trained on it
> deeply enough that the language becomes model-native rather than
> primer-taught.

That does not strictly require a fully custom foundation model. Plausible
routes include:

- a custom tokenizer plus continued pretraining on a large Tacit corpus
- heavy fine-tuning if the base tokenizer is already adequate
- synthetic training corpus generation at large scale
- grammar-constrained or AST-native generation that bypasses ordinary text
- model/tool protocols where edits are structural deltas, not source re-emission

For the specific comparison "prompt a frontier model cold with a primer and ask
it to emit code," beating Python, Rust, or JavaScript token counts is probably
unrealistic. The Phase 4 result supports that: Tacit improved generated output
and repair behavior, but primer cost dominated the end-to-end metric.

Blog/presentation phrasing:

> The experiment suggests that primer-taught AI-first languages can become
> fluent and repairable, but probably cannot beat popular languages on total
> token cost until they are model-native. Token efficiency is not just a
> language-design property; it is a joint property of the language, tokenizer,
> training distribution, and tooling loop.

## Could C or Ghidra p-code be transpiled into Tacit to improve LLM bug analysis?

Yes, C and Rust make more sense than Python for Tacit's long-term interop and
comparison story. They are closer to Tacit's actual comparison class: compiled,
explicit data layout, explicit control flow, performance-sensitive, and
security-relevant.

C or Ghidra p-code to Tacit is plausible, and may be one of the stronger future
use cases. The value would not be making decompiled code prettier. The value
would be using Tacit as a canonical semantic intermediate representation:

- normalize messy C or decompiler output into a stable AST
- make control flow, effects, memory reads/writes, and aliasing explicit
- remove naming and style noise that distracts LLMs
- give the model a stable representation for repeated analysis
- support structural queries such as pointer escape, write reachability, guard
  conditions, and buffer access patterns
- support structural diffs between binary versions or decompiler revisions

This could provide an advantage over raw C or raw p-code, but only if Tacit
preserves the right low-level facts.

Raw C has real advantages:

- LLMs already know it well.
- Decompiled C is readable enough for many cases.
- Vulnerability patterns are well represented in training data.

Raw p-code also has real advantages:

- It is closer to actual binary semantics.
- It avoids some misleading high-level decompiler guesses.
- It already exposes low-level operations explicitly.

Tacit would need to beat both by being a better middle representation: higher
level and more structured than p-code, but less lossy and ambiguous than
decompiled C.

The main risk is semantic loss. If C or p-code is translated into Tacit-Lite and
the translation hides aliasing, integer width, undefined behavior, pointer
provenance, stack layout, or calling convention details, it could make analysis
worse. A security-analysis Tacit dialect would probably need explicit low-level
memory and effect constructs, not just the current Tacit-Lite surface.

The fair claim is:

> C or Ghidra p-code to Tacit could plausibly improve LLM-assisted vulnerability
> analysis, but only if Tacit is treated as a canonical semantic IR rather than
> a pretty high-level language. The advantage would come from normalized
> structure, explicit effects, stable identity, and queryable memory/control-flow
> facts. It would need to preserve low-level semantics aggressively, or raw
> p-code and decompiled C remain safer.

## Does the primer need updates for an AI workflow that uses all Tacit tooling and views?

Yes, but that should not be folded into the current authoring primer.

The current Tacit-Lite primer has one narrow job: teach a model to emit valid
authoring-view Tacit under a strict output contract. That should stay focused,
because every extra workflow instruction increases recurring prompt cost and
worsens the end-to-end token accounting that Phase 4 already exposed.

For a complete AI workflow, Tacit likely needs a separate workflow primer or
runbook. It should teach:

- which view to request for which task: authoring, inspection, and future
  data-flow or dependency views
- how to interpret structured diagnostics
- how to use `tacit view --as inspection --types --effects`
- how to move between `.tac`, `.tacd`, and transient `.taca`
- how to repair from compiler, typechecker, and test feedback
- how to use future `tacit-debug`, `tacit-diff`, or structural query tools
- when to ask for canonical, inspection, or authoring output
- how to avoid treating display names or comments as semantic identity

The right split is:

- **Language primer:** how to write Tacit.
- **Workflow primer:** how to use Tacit tooling to inspect, debug, repair, and
  maintain programs.
- **Tool schemas/help:** machine-readable command contracts and JSON output
  docs, ideally injected only when the tool is available or relevant.
- **Task prompt:** the actual user request.

This split matters because Phase 4 showed that primer cost dominates
end-to-end token accounting. A monolithic "everything primer" would likely make
the metric worse even if it improves agent behavior.

The fair claim is:

> A full AI development workflow requires instruction beyond the authoring-view
> primer, but that instruction should be modular and tool-facing, not folded
> into the core Tacit-Lite primer. The core primer should remain language-facing;
> workflow knowledge should be supplied only for maintenance or debugging tasks
> where the extra context is expected to pay for itself.

This should be validated in Phase 5: measure whether a workflow primer plus
existing tools improves larger-program repair enough to justify its token cost.

## Are there scenarios where progressive disclosure of the primer could work?

Yes. Progressive disclosure is plausible, especially in repositories that
already contain enough Tacit code to serve as local examples. It should be
treated as a workflow strategy, not as a replacement for the core language
primer or as evidence for a primer-only benchmark.

The useful shape is:

- a small always-present orientation primer: syntax, output contract, semantic
  invariants, and the few rules examples do not reliably teach
- retrieved local examples from the current repository, chosen by feature,
  library, module, error shape, or edit pattern
- tool help or schemas injected only when the agent is about to use that tool
- compiler/typechecker/test feedback used as the repair loop instead of adding
  more prose up front

A mature Tacit repository can make this work because the examples are not
abstract teaching material. They are in-dialect, project-specific, already
connected to local conventions, and often closer to the target task than a
general primer could be. For maintenance and extension tasks, "here are three
nearby modules that already do this" may be more valuable than another thousand
tokens of generic primer text.

But examples cannot carry the whole load. The base primer still needs to state
rules that are easy to miss from examples: canonical identity, sidecar/display
metadata, effect boundaries, output format, invalid capture cases, and the
difference between authoring and inspection views. Examples show what worked
somewhere; the primer defines what is legal and portable.

This also changes the metric. Progressive disclosure should be accounted for as
separate context classes:

- always-paid language primer tokens
- retrieved repository-example tokens
- workflow/tool-schema tokens
- repair feedback tokens
- generated Tacit output tokens

That separation matters because a repository-assisted workflow might be
economical even though the Phase 4 primer-plus-generation metric remains
negative. The two claims are different. Primer-only evaluation asks whether a
cold model can learn Tacit from a fixed document. Progressive disclosure asks
whether an agent can maintain Tacit efficiently when it has a real codebase,
retrieval, tools, and feedback.

The main constraints are:

- never retrieve sealed or held-out evaluation material
- only retrieve examples from the same language phase or a known-compatible
  dialect
- prefer compiling, tested examples over stale snippets
- keep retrieval narrow enough that it replaces prose instead of becoming a
  second large primer
- report retrieved-example context separately from core primer cost

The fair claim is:

> Progressive primer disclosure is likely useful for real Tacit repositories,
> especially once they contain enough idiomatic code to retrieve as examples.
> It does not remove the need for a compact core primer, and it should not be
> counted as primer-only fluency, but it may be the right production workflow
> for larger maintenance, extension, and repair tasks.

## Should Tacit have IDE support to help humans understand LLM-generated code?

Yes, as a stretch goal under the inspection/debugging track. Tacit is not meant
to optimize for humans typing code by hand, but humans still need to review,
debug, trust, and present what an LLM generated.

The right IDE goal is human comprehension, not human-first authoring. Useful
features would include:

- syntax highlighting for authoring, canonical, and inspection views
- a language server that surfaces parser, type, effect, and structured
  diagnostic output
- hover cards for inferred type, effect set, canonical hash, source view,
  binding depth, display-name metadata, and closure captures
- jump-to-definition and reference lookup based on structural identity, not
  only display names
- commands to render authoring, canonical, inspection, data-flow, or dependency
  views for the selected node
- inline links from diagnostics to the smallest relevant AST node
- integration with `tacit-diff`, `tacit-debug`, and future structural query
  tools
- a VS Code extension as the first practical packaging target

The IDE should consume the same structured APIs as the CLI tools. It should not
invent a second semantic model or treat sidecar display names as authoritative.
This keeps the source of truth in the canonical AST while giving humans better
ways to inspect it.

This is not urgent before Phase 5. First, measure which inspection/debugging
signals help maintenance. Then build IDE affordances around the proven signals.

The fair claim is:

> IDE support is valuable for Tacit, but primarily as a human-review and
> comprehension layer over LLM-generated code. The language server and editor
> extension should be thin clients over Tacit's canonical AST, structured
> diagnostics, views, diff, and debug APIs.

## Where do modules, packages, unit testing, libraries, and host ABI support fit?

They should be a first-class near-term phase, not a minor stretch item.
Module/package support is the bridge between the current single-program
research artifact and a real Tacit ecosystem.

The current design already points in this direction: definitions are
content-addressed atoms, imports should resolve to hashes, display names are
local aliases, and module boundaries carry explicit type/effect signatures. The
missing implementation layer is project/package composition.

The right ordering is:

- **Phase 5:** validate larger maintenance/debugging workflows.
- **Phase 6:** implement modules, packages, systems primitives, unit testing,
  source-library foundations, dependency caching, and the host-interface ABI.
- **Phase 7:** build full inspection/debugging/IDE tooling on top of real
  multi-module boundaries.

Phase 6 should include:

- module exports/imports with explicit type/effect signatures
- multiple `.tac`/`.tacd` units per project
- deterministic derived layout and a local hash index
- package manifests and lockfiles based on dependency hashes
- a local hash-indexed dependency cache
- unit tests with structured results
- fixed-width integer, bit operation, typed mutable memory, and data-layout
  primitives for systems-style projects
- source-level stdlib foundations for strings, collections, and file I/O
- host-backed curated libraries for network/HTTP-style capabilities

It should also include C/Rust embedding support, but as a constrained
host-interface ABI rather than general FFI. The intended model is:

> Tacit modules are deterministic, typed logic components. The host owns messy
> external capabilities. The module declares what it needs; the host provides
> it.

That means Phase 6 should support:

- stable C ABI for exported Tacit functions
- generated C headers
- generated Rust host bindings
- host-provided imports with explicit type/effect signatures
- ownership and lifetime rules for values crossing the boundary
- result/error ABI
- memory allocation boundary rules
- capability/effect declarations for host-backed operations
- a small C or Rust embedding demo

It should not support:

- arbitrary `extern "C"` declarations from Tacit source
- direct Tacit bindings to random ecosystem libraries
- untyped pointer escape hatches
- dynamic plugin loading
- HTTP as a built-in language primitive

The fair claim is:

> Module/package support is a high-priority missing bridge between Tacit as a
> research artifact and Tacit as a composable ecosystem. It should include a
> constrained C/Rust host-interface ABI so Tacit can run as a typed logic kernel
> inside a conventional host program, but it should not become general-purpose
> FFI.

## What language features would Tacit need for a video game emulator?

An emulator is plausible as a long-term Tacit-Lite benchmark, but not with the
Phase 4 language alone. The missing pieces are mostly systems-programming
primitives, not Tacit-Full research features.

An emulator would need:

- fixed-width integers: `u8`, `u16`, `u32`, `u64`, and signed variants
- explicit wrapping, checked, and saturating arithmetic
- bit operations: shifts, masks, bitwise operations, rotates, and sign extension
- byte-order helpers for decoding binary formats
- efficient byte-addressable mutable memory
- typed arrays/slices beyond today's `Buf` and `I64Vec`
- explicit bounds behavior for performance-critical memory access
- records or packed/ABI-stable layouts for CPU registers, flags, and device
  state
- enums or tagged-union-like decode shapes for instructions and addressing
  modes, if existing constructors and `match` are insufficient
- efficient dense dispatch for instruction decode loops
- module/package structure for CPU, memory bus, graphics/audio/input adapters,
  cartridge mappers, and tests
- a C/Rust host-interface ABI for windowing, audio, input, ROM loading, timing,
  and platform integration
- unit tests and golden-state fixtures for CPU and memory behavior
- optimization work for tight loops, helper inlining, typed-array access, and
  host-boundary overhead

Most of this belongs in two places:

- **Phase 6:** define the language/runtime surface needed to express
  emulator-style systems code: fixed-width integers, bit operations, typed
  mutable memory, data-layout/decode support, modules, tests, and host ABI.
- **Phase 8:** make those primitives fast enough: dense `match` lowering,
  jump-table-like dispatch, bounds-check strategy, typed-array lowering, and
  emulator-shaped benchmarks.

Windowing, audio, input, filesystem selection, and timing should remain
host-owned capabilities at first. Tacit should express the emulator logic
kernel; the host should own the messy platform integration.

The fair claim is:

> A video game emulator is a good long-term stress test for Tacit-Lite, but it
> requires systems primitives that Phase 4 does not yet have. The missing work
> is fixed-width arithmetic, bit operations, typed memory, efficient dispatch,
> package structure, host ABI, unit testing, and performance hardening, not
> refinement types or Tacit-Full effect machinery.
