# Phase 6 data layout and decode examples

These examples are the Stage 8 proof points from ADR 0086.

| Program | Description | Expected exit |
| --- | --- | --- |
| `cpu-state-record.tac` | Nested CPU-state record with fixed-width registers and flags. | 52 |
| `opcode-decode-record.tac` | Opcode nibble decode into an explicit tagged record. | 18 |

The examples intentionally use existing records, fixed-width integers, bit
operations, and `match`. Stage 8 does not add enum, packed-layout, or
ABI-stable record syntax.
