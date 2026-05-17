# Phase 6 Stage 11 — embedding demo

A small Tacit kernel exported through the constrained host ABI from
[ADR 0088](../../../decisions/0088-phase-6-host-interface-abi.md), linked into
a Rust host that owns platform-shaped state and satisfies a host import.

## Layout

- `kernel/` — a Tacit package. Public exports are scalar-typed entry points
  for a tiny register-machine CPU; tests verify the pure behaviour via
  `tacit test`.
- `host/` — a Rust crate whose `build.rs` invokes the typecheck and codegen
  APIs to emit a static library plus C header and Rust bindings under
  `$OUT_DIR`, then links the host binary against it.
- `../../../tools/stage11_demo_gen/` — one-shot generator that constructs the
  kernel `.tac` file, hashes each definition, and writes the matching
  `tacit.toml`. Run `cargo run -p stage11-demo-gen` after editing kernel
  definitions to refresh both files in lockstep.

## Kernel surface

The kernel declares one host import and three public exports:

| Symbol | Tacit type | Notes |
| --- | --- | --- |
| `host demo.log / write-byte` | `Int -> Int / {IO}` | Host import; the host appends every byte it sees to its own log and echoes it back. |
| `decode-op` | `Int -> Int` | Returns the low nibble of an instruction byte (`byte mod 16`). |
| `step-cpu` | `Int -> Int -> Int -> Int` | One register-machine step: `(acc, op_nibble, operand) -> new_acc`. Implements NOP, ADD-mod-256, SUB-mod-256, MUL-mod-256, LOAD, ZERO; unknown opcodes are NOPs. |
| `log-acc` | `Int -> Int / {IO}` | Calls the host import to log a byte and returns the byte. |

Internally the package also defines four `Bool` test entries; they exercise
`decode-op` and `step-cpu` through `tacit test`, demonstrating that pure
kernel definitions can be checked without involving the host at all.

## Running the demo

```sh
# Package-level tests (no host involved).
cargo run -p tacit-cli --features llvm19-1 -- \
    test examples/phase-6/embedding-demo/kernel

# Generate the interface metadata, C header, Rust bindings, and a static
# library under the package's derived directory.
cargo run -p tacit-cli --features llvm19-1 -- \
    interface examples/phase-6/embedding-demo/kernel --emit-library

# Build and run the host: it links the static library, registers the host
# import callback, and exercises each public export.
cargo run -p tacit-embedding-demo-host
```

The host prints a small transcript and asserts the round-tripped log:

```
tacit embedding demo (Phase 6 Stage 11)
  package hash: blake3:...
  symbol prefix: tacit_p_...
decode_op(0xAB) = 11
cpu program ran to acc = 40
log_acc(40) -> 40
host_log: [40]
ok
```

## What this proves

- A multi-definition Tacit package can be **checked**, **tested**, **compiled
  to a linkable artifact** with ABI-conforming wrappers, and **consumed by a
  Rust host** through the Stage 10 constrained ABI — without exposing
  arbitrary FFI from the Tacit side.
- Host-provided capabilities are content-addressed (`demo.log / write-byte`
  hashes into the package identity through the unit's import table); the
  host implements the callback as an ordinary Rust function whose pointer is
  threaded through the generated callbacks struct.
- The kernel's runtime trap on a missing or null callback uses `llvm.trap`,
  matching the Stage 7 bounds-trap policy: ABI status values describe
  boundary failures only, source errors remain ordinary Tacit return values.

## Known limits to revisit later

- The host boundary in this demo uses scalar types only. ADR 0088 allows
  records and borrowed typed vectors, but the Stage 11 codegen rejects them
  with `abi-library-unsupported-type` so the kernel and the wrappers stay
  small. Phase 7 or a later bounded ADR can extend the codegen.
- Performance hardening (closure allocation in private bodies, lowering
  patterns for tight CPU steps, optional optimizer pipeline) remains a Phase
  8 concern.
