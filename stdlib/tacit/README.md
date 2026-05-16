# Tacit Source Stdlib

These directories are ordinary Phase 6 Tacit packages. They are resolved
through `tacit.toml`, `tacit.lock`, and exact BLAKE3 definition imports like
other packages; there is no implicit Stage 9 prelude or name-based `std`
resolver.

The `host/` namespace is reserved for Stage 10 host-backed capability wrapper
packages. It does not define networking, HTTP, arbitrary FFI, or dynamic
plugins in Stage 9.
