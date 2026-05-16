# Phase 6 package test examples

These packages demonstrate the Stage 5 `[[tests]]` manifest surface.

- `pure/` contains a pure `Bool` test.
- `effectful/` shows a manifest entry that explicitly opts in to allocation
  and mutation effects for a runnable test target.

Run either package with:

```sh
tacit test examples/phase-6/package-tests/pure --format json
tacit test examples/phase-6/package-tests/effectful --format json
```
