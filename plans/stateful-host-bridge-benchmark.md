# Stateful Host-Bridge Benchmark Shape

**Status:** Draft benchmark shape
**Scope:** Open benchmark and measurement plan for the stateful host-bridge
track. This document defines what to measure; it does not contain benchmark
programs yet.

## Purpose

The benchmark exists to keep stateful host-bridge work honest. It should prove
that Tacit can own long-running state while the host only bridges platform
capabilities, and it should identify whether bottlenecks are in language
semantics, ABI lowering, codegen, generated bindings, or host-library glue.

No benchmark task, fixture, validation run, or result analysis may read, list,
search, or otherwise depend on `corpus/sealed/`.

## Workload Classes

| ID | Workload | Purpose |
| --- | --- | --- |
| SHB-B1 | Host-call overhead | Measure scalar and non-scalar generated ABI call cost for no-op and small-return exports. |
| SHB-B2 | Rich boundary bulk transfer | Measure borrowed `u8vec`/`i16vec` parameters for ROM-sized input, framebuffer output, and audio-buffer output. |
| SHB-B3 | CPU-step loop | Measure a Tacit-owned instruction loop over fixed-width registers and byte-addressed memory. |
| SHB-B4 | Framebuffer production | Measure production and host transfer of a 160x144 frame without one ABI call per pixel. |
| SHB-B5 | Audio-buffer production | Measure production and host transfer of sample buffers without one ABI call per sample. |
| SHB-B6 | Instance lifecycle | Measure create, reset, run, and destroy for Tacit-owned package instances. |
| SHB-B7 | Host responsibility audit | Verify that host code bridges libraries but does not own domain state or emulator semantics. |

## Required Measurements

Every benchmark run should record:

- toolchain version, release hash, compiler git revision, LLVM feature, and
  target triple;
- package hash and public export hashes under test;
- host compiler version and optimization mode;
- CPU model, operating system, and whether the run used debug or release host
  code;
- median, p95, and best-of-N timing for each workload;
- ABI calls per frame, per audio buffer, and per instruction batch;
- bytes transferred per frame/audio/ROM workload;
- instructions or synthetic opcodes executed per second for CPU-loop
  workloads;
- whether execution is bounded-stack by construction, by inspection, or only
  by observed behavior;
- whether host code contains any domain state such as CPU registers, memory
  maps, PPU state, timers, or instruction semantics.

## Gates vs Descriptive Results

Stage 0 does not set numeric throughput gates. The first complete benchmark
runs should produce baselines before a later ADR decides calibrated thresholds.

Structural gates:

- The benchmark must be open and must not depend on `corpus/sealed/`.
- Correctness checks must pass before throughput numbers are interpreted.
- Bulk video and audio paths must not require one ABI call per pixel or
  sample.
- Long-running loop workloads must have a bounded-stack lowering contract
  before they can satisfy the loop stage exit criteria.
- Instance lifecycle workloads must support multiple independent instances
  before they can satisfy the package-instance stage exit criteria.
- The host responsibility audit must fail if the host owns emulator-domain
  semantics rather than bridge code.

Descriptive results:

- scalar host-call nanoseconds per call;
- borrowed-vector call nanoseconds per call;
- framebuffer bytes per second;
- audio-buffer bytes per second;
- synthetic instructions per second;
- create/destroy cost per package instance;
- generated binding size and amount of handwritten host glue.

## Benchmark Artifacts

When implemented, benchmark artifacts should live under an open path such as
`plans/stateful-host-bridge-benchmark/` or `examples/`, not under the corpus
sealed tree.

Each benchmark should include:

- the Tacit package source;
- the host harness source;
- the exact command line used to build and run;
- a machine-readable JSON result record;
- a short README explaining the workload and what domain responsibility, if
  any, the host is allowed to own.

## Host Responsibility Rules

Allowed host responsibilities:

- create windows, audio devices, timers, files, and input handles;
- copy bytes between third-party libraries and generated Tacit ABI buffers;
- satisfy generated callback tables;
- own process lifecycle and error reporting around the Tacit package.

Disallowed host responsibilities for stateful emulator benchmarks:

- CPU register storage;
- memory-map dispatch;
- instruction decode or execution;
- PPU mode/state transitions;
- timer, interrupt, or DMA semantics;
- audio channel state or sample synthesis semantics.

The benchmark may include smaller microbenchmarks where those concepts do not
exist. For emulator-shaped vertical slices, the split above is load-bearing.

## Initial Reporting Format

A future implementation should emit a JSON result with at least:

```json
{
  "format": "tacit-stateful-host-bridge-benchmark-v1",
  "toolchain": {
    "version": "0.0.0",
    "release_hash": "blake3:..."
  },
  "host": {
    "language": "rust",
    "profile": "release",
    "target": "x86_64-unknown-linux-gnu"
  },
  "results": [
    {
      "id": "SHB-B1",
      "name": "host-call-overhead",
      "outcome": "pass",
      "median_ns": 0,
      "p95_ns": 0,
      "details": {}
    }
  ]
}
```

The schema is intentionally small until real runs show which fields are
stable. A later metrics ADR should freeze the schema before using benchmark
numbers as release gates.
