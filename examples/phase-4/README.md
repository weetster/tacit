# Phase 4 examples

Durable examples for the Phase 4 language surface. These are checked in as
canonical `.tac` files with `.tacd` display sidecars; authoring `.taca` files
are transient and should not be committed here.

| File | Surface | Expected exit | Stdout |
| --- | --- | ---: | --- |
| `record-accumulator.tac` | Named-field records and projection for structured accumulator state. | 9 | empty |
| `closure-pipeline.tac` | Returned capturing closure applied through a local function value. | 42 | empty |
| `stored-callback-record.tac` | Function values stored in and projected from a record. | 41 | empty |
| `vector-combinators.tac` | `@map`, `@fold`, `@for-each`, captured callback state, and callback effects. | 18 | `A\nB\n` |

These examples are hand-authored and are not drawn from `corpus/`.
