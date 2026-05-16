# Phase 6 Typed Mutable Memory Examples

These examples exercise the Stage 7 typed mutable-memory primitives
(ADR 0085): length-carrying typed vectors, bounds-checked element access, and
byte-bus typed loads and stores.

`memory-bus-u32.tac` writes a 32-bit little-endian value into a `u8vec`
"memory" and reads it back through the byte-bus helpers. The program's
result is the low byte of the round-tripped value, demonstrating that
multi-byte typed loads and stores compose with the Stage 6 cast primitives.

`register-file.tac` allocates a small `u32vec` register file, writes one
register, and reads it back to demonstrate the uniform per-width primitive
surface.
