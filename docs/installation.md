# Installing Tacit

This doc covers installing a published Tacit toolchain archive and starting an
external project. For contributor source builds, see the bottom section.

## Supported platforms

Published release archives target:

- **Linux x86_64** (`x86_64-unknown-linux-gnu`)
- **macOS x86_64** (`x86_64-apple-darwin`)

The compiler binary links statically against LLVM 19 so no LLVM runtime is
required on the target host.

Linux runtime dependencies (dynamically linked on the target host): `libc`,
`libstdc++`, `libgcc_s`, `libm`, `libz`, `libzstd`, `libffi`. All are present
in a default Debian-bookworm / Ubuntu-22.04+ install. macOS release artifacts
must link only platform system libraries at runtime; the release script rejects
Homebrew dylib dependencies. There is no LLVM runtime dependency.

Published Linux artifacts are built on Ubuntu 22.04, which sets the glibc
compatibility floor at **glibc 2.35**. In practice that means the published
Linux `tacit` binary should run on Debian/Ubuntu hosts with glibc 2.35 or
newer, but not on older releases whose `libc.so.6` is below that baseline.

Published macOS x86_64 artifacts are built on a native Intel macOS GitHub
Actions runner with `MACOSX_DEPLOYMENT_TARGET=12.0`. They are intended for
Monterey-era systems and newer, but Monterey compatibility should be verified
with the produced artifact before treating that floor as proven.

Tacit programs themselves have no network access (no sockets, HTTP, TCP, or
UDP) at the language or stdlib level. The bundled `tacit.io` package provides
only stdin/stdout/stderr byte I/O. Embedders may add network capabilities via
host imports (see ADR 0088 / `tacit interface`), but no such capability ships
with the toolchain itself.

## Binary-archive layout

A complete Tacit toolchain release is a single directory:

```
tacit-<version>-<target-triple>/
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

Download the archive for your platform and its `.sha256` companion. For Linux:

```sh
sha256sum -c tacit-<version>-x86_64-unknown-linux-gnu.sha256
tar -xzf tacit-<version>-x86_64-unknown-linux-gnu.tar.gz
sudo cp -r tacit-<version>-x86_64-unknown-linux-gnu/bin/tacit /usr/local/bin/
sudo cp -r tacit-<version>-x86_64-unknown-linux-gnu/share/tacit /usr/local/share/
tacit version --format json
```

For macOS x86_64:

```sh
shasum -a 256 -c tacit-<version>-x86_64-apple-darwin.sha256
tar -xzf tacit-<version>-x86_64-apple-darwin.tar.gz
sudo cp -r tacit-<version>-x86_64-apple-darwin/bin/tacit /usr/local/bin/
sudo cp -r tacit-<version>-x86_64-apple-darwin/share/tacit /usr/local/share/
tacit version --format json
```

Per-user install (no root) into `~/.local`, substituting your target triple:

```sh
mkdir -p ~/.local/bin ~/.local/share
cp tacit-<version>-<target-triple>/bin/tacit ~/.local/bin/
cp -r tacit-<version>-<target-triple>/share/tacit ~/.local/share/
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
  - macOS: install a prebuilt Homebrew bottle with
    `brew fetch --force-bottle --deps llvm@19` followed by
    `brew install --force-bottle llvm@19`
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

On macOS x86_64, use:

```sh
scripts/build-release-macos-x86_64.sh
```

The Linux script verifies that the binary links statically against LLVM 19 (no
`libLLVM*.so` in `ldd`), checks that the binary does not require glibc newer
than 2.35, assembles the `share/tacit/` tree, regenerates
`templates/executable/` and `templates/library/` by invoking the freshly
built `tacit init`, and writes `release/tacit-<version>-x86_64-unknown-linux-gnu.tar.gz`
plus `.sha256` and `SHA256SUMS` files.

The macOS script performs the matching Darwin checks with `otool -L`, rejects
dynamic `libLLVM` or Homebrew runtime library dependencies, assembles the same
`share/tacit/` tree, and writes
`release/tacit-<version>-x86_64-apple-darwin.tar.gz` plus checksum files.

Note that local source builds inherit the host's glibc baseline. If you build
Tacit on a newer distro than Ubuntu 22.04, your locally produced binary may
require a newer glibc than the published GitHub release artifact.
