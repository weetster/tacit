# Installing Tacit

This doc covers installing a published Tacit toolchain archive and starting an
external project. For contributor source builds, see the bottom section.

## Supported platforms

The first export targets **Linux x86_64** only. The compiler binary links
statically against LLVM 19 so no LLVM runtime is required on the target host.

Other platforms (macOS, ARM Linux, Windows) are not built or tested by the
release script in this stage. Each adds its own static-LLVM toolchain and
prebuilt-archive story and is out of scope until later.

Runtime dependencies (dynamically linked on the target host): `libc`,
`libstdc++`, `libgcc_s`, `libm`, `libz`, `libzstd`, `libffi`. All are present
in a default Debian-bookworm / Ubuntu-22.04+ install. There is no LLVM runtime
dependency.

Published GitHub release artifacts are built on Ubuntu 22.04, which sets the
glibc compatibility floor at **glibc 2.35**. In practice that means the
published `tacit` binary should run on Debian/Ubuntu hosts with glibc 2.35 or
newer, but not on older releases whose `libc.so.6` is below that baseline.

Tacit programs themselves have no network access (no sockets, HTTP, TCP, or
UDP) at the language or stdlib level. The bundled `tacit.io` package provides
only stdin/stdout/stderr byte I/O. Embedders may add network capabilities via
host imports (see ADR 0088 / `tacit interface`), but no such capability ships
with the toolchain itself.

## Binary-archive layout

A complete Tacit toolchain release is a single directory:

```
tacit-<version>-x86_64-unknown-linux-gnu/
  bin/tacit
  share/tacit/
    toolchain-release.json
    primer/tacit-lite.md
    primer/tacit-lite.toml
    workflow/agent-workflow.md
    stdlib-src/tacit/{core,bytes,array,text,collections,io}/
    stdlib-cache/objects/{defs,units,sidecars}/
    stdlib-cache/packages/<package-hash>/
    templates/executable/
    templates/library/
```

`bin/tacit` looks for `share/tacit/` at install time via two rules:

1. If `TACIT_TOOLCHAIN_ASSET_ROOT` is set, it must point at the
   `share/tacit/` directory.
2. Otherwise the binary looks for `../share/tacit/` relative to the resolved
   `bin/tacit` path.

If you keep the archive layout intact when installing, no environment variable
is needed.

## Install

Download `tacit-<version>-x86_64-unknown-linux-gnu.tar.gz` and its
`.sha256` companion. Then:

```sh
sha256sum -c tacit-<version>-x86_64-unknown-linux-gnu.sha256
tar -xzf tacit-<version>-x86_64-unknown-linux-gnu.tar.gz
sudo cp -r tacit-<version>-x86_64-unknown-linux-gnu/bin/tacit /usr/local/bin/
sudo cp -r tacit-<version>-x86_64-unknown-linux-gnu/share/tacit /usr/local/share/
tacit version --format json
```

Per-user install (no root) into `~/.local`:

```sh
mkdir -p ~/.local/bin ~/.local/share
cp tacit-<version>-x86_64-unknown-linux-gnu/bin/tacit ~/.local/bin/
cp -r tacit-<version>-x86_64-unknown-linux-gnu/share/tacit ~/.local/share/
~/.local/bin/tacit version --format json
```

`tacit version --format json` should report
`installed_manifest.status: "matched"`. Any other status means the
`share/tacit/toolchain-release.json` next to your binary is missing, edited,
or from a different toolchain — re-extract the archive.

## Start an external project

In a directory outside of any other Tacit repository:

```sh
tacit init my-project
cd my-project
tacit check .
tacit test .
tacit compile .
./target/release/... # see derived/bin under .tacit/derived
```

`tacit init` writes `tacit-toolchain.toml` pinning your project to this
toolchain's version, release hash, primer hash, and bundled-stdlib package
hashes. Package-aware commands (`check`, `compile`, `test`, `interface`,
`lock`) verify that pin against the installed toolchain on every run. A
mismatch is a hard error with a `toolchain-pin-*` diagnostic; a missing pin is
a warning.

For a library project that publishes hash-pinned dependencies on the bundled
stdlib:

```sh
tacit init my-lib --template library --with-stdlib
cd my-lib
tacit interface . --emit-library
```

To regenerate the project lockfile after editing `tacit.toml` dependencies:

```sh
tacit lock
```

## Where the language primer and workflow live

Agents and humans can discover the language primer and the workflow doc
without leaving the toolchain:

```sh
tacit primer                        # prints the primer markdown
tacit primer --format json          # primer id/version/hash/tokens
tacit primer --search rec           # find matching primer lines and section ids
tacit primer --list-sections        # list selectable primer sections
tacit primer --section primitive-surface
tacit primer --check primer.md      # verify a copy matches the installed bytes
cat $(dirname $(which tacit))/../share/tacit/workflow/agent-workflow.md
```

Generated `AGENTS.md` and `CLAUDE.md` files inside a Tacit project instruct
agents to fetch these via the toolchain rather than copying prose from another
source — the bytes are pinned per ADR 0090.

## Building from source (contributors)

Source builds are not a supported release channel but are the contributor
workflow. Requirements:

- Rust toolchain (stable; see `rust-toolchain.toml` if present)
- LLVM 19 development install
  - Debian bookworm / Ubuntu 24.04+: `sudo apt install llvm-19-dev libpolly-19-dev`
  - Ubuntu 22.04: add apt.llvm.org's Jammy repo for LLVM 19, then install
    `llvm-19-dev libpolly-19-dev`
- A C linker on `PATH` (`cc`, `clang`, or `gcc`) and `ar`/`llvm-ar`

Then:

```sh
cargo build --features tacit-cli/llvm19-1 -p tacit-cli
./target/debug/tacit version --format json
```

To produce a release archive identical to the published one:

```sh
scripts/build-release.sh
```

The script verifies that the binary links statically against LLVM 19 (no
`libLLVM*.so` in `ldd`), checks that the binary does not require glibc newer
than 2.35, assembles the `share/tacit/` tree, regenerates
`templates/executable/` and `templates/library/` by invoking the freshly
built `tacit init`, and writes `release/tacit-<version>-x86_64-unknown-linux-gnu.tar.gz`
plus `.sha256` and `SHA256SUMS` files.

Note that local source builds inherit the host's glibc baseline. If you build
Tacit on a newer distro than Ubuntu 22.04, your locally produced binary may
require a newer glibc than the published GitHub release artifact.
