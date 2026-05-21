# 0097 - macOS x86_64 toolchain release target

**Status:** Accepted
**Date:** 2026-05-21
**Phase:** Toolchain export target expansion
**Amends:** [ADR 0090](0090-toolchain-release-contract.md) additively.

## Context

ADR 0090 shipped the first exported Tacit toolchain as a Linux x86_64 binary
archive. The next practical target is Intel macOS because some Tacit users
still run Intel Macs, and local source-building LLVM on those machines is too
slow to be an acceptable release path.

The repository's current compiler already emits native LLVM objects through
LLVM's host target machine. Adding macOS x86_64 therefore does not require a
new backend if the release is built on a native Intel macOS runner.

The release path must avoid Homebrew source builds of LLVM. A missing LLVM
bottle should fail early in CI rather than spending hours compiling LLVM from
source.

No design, implementation, or validation work for this target expansion may
read, list, search, or otherwise depend on `corpus/sealed/`.

## Decision

Add a native macOS x86_64 release artifact named:

```text
tacit-<version>-x86_64-apple-darwin.tar.gz
```

The macOS artifact uses the same `tacit-toolchain-archive-v1` layout as the
Linux artifact:

```text
tacit-<version>-x86_64-apple-darwin/
  bin/tacit
  share/tacit/
```

The GitHub release workflow builds this artifact on an Intel macOS hosted
runner (`macos-15-intel`) instead of cross-compiling from Linux. The workflow
sets `MACOSX_DEPLOYMENT_TARGET=12.0` so the produced binary is intended to
remain compatible with Monterey-era systems. Monterey compatibility still
needs a real artifact smoke test because GitHub-hosted runners do not provide
Monterey.

LLVM 19 is installed through Homebrew with a bottle-only preflight:

```sh
brew fetch --formula --force-bottle --deps llvm@19
brew install --formula --force-bottle llvm@19
```

The `fetch` step catches missing bottles before installation. The
`--force-bottle` install flag prevents the workflow from taking a source-build
path for `llvm@19`. The workflow also sets
`HOMEBREW_NO_BOTTLE_SOURCE_FALLBACK=1` as a defensive guard for Homebrew
versions that honor that environment variable.

The macOS release script verifies:

- it is running on Darwin x86_64;
- LLVM reports major version 19 and provides static `libLLVM*.a` archives;
- the produced `tacit` binary does not dynamically link `libLLVM`;
- the produced binary has no Homebrew runtime library dependency paths;
- the staged `share/tacit/toolchain-release.json` matches the manifest embedded
  in the binary.

GitHub Release publication is moved to one final job that downloads both
platform artifacts. This avoids parallel platform jobs racing to create or
update the same release.

## Alternatives considered

### Cross-compile macOS x86_64 from Linux

Rejected. The compiler can emit LLVM objects for many triples, but linking a
macOS executable from Linux requires an Apple SDK/toolchain story. That is a
larger distribution project than this target expansion needs.

### Build LLVM from source on the macOS runner

Rejected. Source-building LLVM is exactly the failure mode this release target
is meant to avoid.

### Depend on user-installed Homebrew runtime libraries

Rejected. The release archive should remain usable on target machines without
requiring Homebrew or LLVM. If the binary links any Homebrew dylib, the release
script fails.

### Build only on Apple Silicon with Rosetta

Rejected for the initial x86_64 release. A native Intel runner gives simpler
target-machine behavior and avoids making the first macOS release depend on
Rosetta execution details.

## Consequences

- The release workflow produces Linux x86_64 and macOS x86_64 archives.
- macOS x86_64 support remains native-hosted, not a general cross-target
  abstraction.
- Monterey compatibility is intended through the deployment target but must be
  validated manually on a Monterey host before being advertised as fully
  proven.
- The release workflow fails fast if Homebrew cannot supply prebuilt LLVM 19
  bottles.
