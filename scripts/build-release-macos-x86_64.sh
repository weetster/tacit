#!/usr/bin/env bash
# Builds a binary-archive distribution for macOS x86_64.
#
# This is intentionally separate from scripts/build-release.sh because the
# Linux release script depends on GNU userland checks (ldd, objdump, stat -c,
# sha256sum, tar --sort). This script uses Darwin-native checks.
#
# Output:
#   release/tacit-<version>-x86_64-apple-darwin/         (staged tree)
#   release/tacit-<version>-x86_64-apple-darwin.tar.gz   (archive)
#   release/tacit-<version>-x86_64-apple-darwin.sha256   (archive checksum)
#   release/tacit-<version>-x86_64-apple-darwin.SHA256SUMS
#
# Usage:
#   scripts/build-release-macos-x86_64.sh [--llvm-prefix /usr/local/opt/llvm@19] [--keep-stage]

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${REPO_ROOT}"

TARGET_TRIPLE="x86_64-apple-darwin"
LLVM_PREFIX=""
KEEP_STAGE=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --llvm-prefix)
            LLVM_PREFIX="$2"
            shift 2
            ;;
        --keep-stage)
            KEEP_STAGE=1
            shift
            ;;
        -h|--help)
            sed -n '2,15p' "$0"
            exit 0
            ;;
        *)
            echo "error: unknown argument: $1" >&2
            exit 2
            ;;
    esac
done

# --- Platform guard ----------------------------------------------------------
host_uname=$(uname -s)
host_arch=$(uname -m)
if [[ "${host_uname}" != "Darwin" || "${host_arch}" != "x86_64" ]]; then
    echo "error: this release script only targets ${TARGET_TRIPLE} (host: ${host_uname}/${host_arch})" >&2
    exit 1
fi

# --- LLVM discovery ----------------------------------------------------------
if [[ -z "${LLVM_PREFIX}" ]]; then
    if command -v llvm-config-19 >/dev/null 2>&1; then
        LLVM_PREFIX=$(llvm-config-19 --prefix)
    elif command -v brew >/dev/null 2>&1; then
        BREW_LLVM_PREFIX=$(brew --prefix llvm@19 2>/dev/null || true)
        if [[ -n "${BREW_LLVM_PREFIX}" && -x "${BREW_LLVM_PREFIX}/bin/llvm-config" ]]; then
            LLVM_PREFIX="${BREW_LLVM_PREFIX}"
        else
            echo "error: could not find LLVM 19; install a prebuilt llvm@19 bottle or pass --llvm-prefix" >&2
            echo "       CI should run: brew fetch --force-bottle --deps llvm@19 && brew install --force-bottle llvm@19" >&2
            exit 1
        fi
    else
        echo "error: could not find LLVM 19; install a prebuilt llvm@19 bottle or pass --llvm-prefix" >&2
        echo "       CI should run: brew fetch --force-bottle --deps llvm@19 && brew install --force-bottle llvm@19" >&2
        exit 1
    fi
fi

LLVM_VERSION=$("${LLVM_PREFIX}/bin/llvm-config" --version)
if [[ "${LLVM_VERSION%%.*}" != "19" ]]; then
    echo "error: ${LLVM_PREFIX}/bin/llvm-config reports ${LLVM_VERSION}; expected 19.x" >&2
    exit 1
fi

LLVM_LIBDIR=$("${LLVM_PREFIX}/bin/llvm-config" --libdir)
if ! ls "${LLVM_LIBDIR}"/libLLVM*.a >/dev/null 2>&1; then
    echo "error: ${LLVM_LIBDIR} does not contain libLLVM*.a static archives" >&2
    echo "       use a full LLVM 19 development install, not only dynamic libraries" >&2
    exit 1
fi
echo "==> using LLVM ${LLVM_VERSION} prefix: ${LLVM_PREFIX}"

# --- Toolchain version -------------------------------------------------------
VERSION=$(awk -F' *= *' '
    /^\[toolchain\]/ {in_toolchain=1; next}
    /^\[/             {in_toolchain=0}
    in_toolchain && $1=="version" {gsub(/"/, "", $2); print $2; exit}
' tacit-toolchain-release.toml)
if [[ -z "${VERSION}" ]]; then
    echo "error: could not parse toolchain version from tacit-toolchain-release.toml" >&2
    exit 1
fi
NAME="tacit-${VERSION}-${TARGET_TRIPLE}"
echo "==> packaging ${NAME}"

# --- Cargo build with static LLVM -------------------------------------------
export LLVM_SYS_191_PREFIX="${LLVM_PREFIX}"
unset LLVM_SYS_191_PREFER_DYNAMIC || true
export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-12.0}"

# LLVM@19 Homebrew bottles dynamically link zstd; override with a static-only
# shim directory so the distributed binary does not require Homebrew at runtime.
# When the linker resolves -lzstd, it finds only libzstd.a in the shim first.
if command -v brew >/dev/null 2>&1; then
    _ZSTD_PREFIX=$(brew --prefix zstd 2>/dev/null || true)
    if [[ -n "${_ZSTD_PREFIX}" && -f "${_ZSTD_PREFIX}/lib/libzstd.a" ]]; then
        _ZSTD_SHIM=$(mktemp -d)
        ln -s "${_ZSTD_PREFIX}/lib/libzstd.a" "${_ZSTD_SHIM}/libzstd.a"
        export RUSTFLAGS="${RUSTFLAGS:+${RUSTFLAGS} }-L native=${_ZSTD_SHIM}"
        echo "==> forcing static zstd from ${_ZSTD_PREFIX}"
    fi
fi

echo "==> MACOSX_DEPLOYMENT_TARGET=${MACOSX_DEPLOYMENT_TARGET}"
echo "==> cargo build --release --features tacit-cli/llvm19-1 -p tacit-cli"
cargo build --release --features tacit-cli/llvm19-1 -p tacit-cli

BIN_PATH="target/release/tacit"
if [[ ! -x "${BIN_PATH}" ]]; then
    echo "error: ${BIN_PATH} not produced by cargo build" >&2
    exit 1
fi

# --- Dynamic-link verification ----------------------------------------------
echo "==> otool -L $(basename "${BIN_PATH}")"
otool_out=$(otool -L "${BIN_PATH}" 2>&1)
echo "${otool_out}" | sed 's/^/    /'
if echo "${otool_out}" | grep -q 'libLLVM'; then
    echo "error: ${BIN_PATH} dynamically links libLLVM (expected static link)" >&2
    exit 1
fi
if echo "${otool_out}" | grep -E '/(usr/local|opt/homebrew)/(Cellar|opt)/' >/dev/null; then
    echo "error: ${BIN_PATH} has Homebrew runtime library dependencies" >&2
    echo "       macOS release artifacts must not require Homebrew on the target host" >&2
    exit 1
fi

# --- Stage layout ------------------------------------------------------------
STAGE_ROOT="release/${NAME}"
rm -rf "${STAGE_ROOT}"
mkdir -p "${STAGE_ROOT}/bin" "${STAGE_ROOT}/share/tacit"
install -m 755 "${BIN_PATH}" "${STAGE_ROOT}/bin/tacit"

# Find the most-recent build script OUT_DIR that contains a staged
# share/tacit/ tree. BSD stat does not support the Linux script's stat -c.
build_out=$(
    find "target/release/build" -type d -path '*/tacit-cli-*/out' -print 2>/dev/null \
        | while IFS= read -r dir; do
            if [[ -d "${dir}/share/tacit" ]]; then
                printf '%s\t%s\n' "$(stat -f '%m' "${dir}")" "${dir}"
            fi
        done \
        | sort -nr \
        | head -1 \
        | cut -f2-
)
if [[ -z "${build_out}" ]]; then
    echo "error: could not locate tacit-cli build script OUT_DIR with staged share/tacit/" >&2
    exit 1
fi
cp -R "${build_out}/share/tacit/." "${STAGE_ROOT}/share/tacit/"

# --- Verify embedded manifest matches staged manifest ------------------------
version_json=$(TACIT_TOOLCHAIN_ASSET_ROOT="${STAGE_ROOT}/share/tacit" \
    "${STAGE_ROOT}/bin/tacit" version --format json)
status=$(printf '%s' "${version_json}" \
    | python3 -c 'import json,sys;print(json.load(sys.stdin)["installed_manifest"]["status"])')
if [[ "${status}" != "matched" ]]; then
    echo "error: staged manifest does not match embedded manifest (status: ${status})" >&2
    echo "${version_json}" | sed 's/^/    /' >&2
    exit 1
fi
release_hash=$(printf '%s' "${version_json}" \
    | python3 -c 'import json,sys;print(json.load(sys.stdin)["release_hash"])')
echo "==> release hash: ${release_hash}"

# --- Stage templates by running the freshly built tacit ----------------------
TEMPLATES_DIR="${STAGE_ROOT}/share/tacit/templates"
mkdir -p "${TEMPLATES_DIR}"
tmp_templates=$(mktemp -d)
trap 'rm -rf "${tmp_templates}"' EXIT

TACIT_TOOLCHAIN_ASSET_ROOT="${STAGE_ROOT}/share/tacit" \
    "${STAGE_ROOT}/bin/tacit" init "${tmp_templates}/executable" >/dev/null
cp -R "${tmp_templates}/executable" "${TEMPLATES_DIR}/executable"

TACIT_TOOLCHAIN_ASSET_ROOT="${STAGE_ROOT}/share/tacit" \
    "${STAGE_ROOT}/bin/tacit" init --template library "${tmp_templates}/library" >/dev/null
cp -R "${tmp_templates}/library" "${TEMPLATES_DIR}/library"

# --- Archive + checksums -----------------------------------------------------
echo "==> creating tarball release/${NAME}.tar.gz"
COPYFILE_DISABLE=1 tar -czf "release/${NAME}.tar.gz" -C release "${NAME}"

(
    cd release
    shasum -a 256 "${NAME}.tar.gz" > "${NAME}.sha256"
    find "${NAME}" -type f | LC_ALL=C sort | while IFS= read -r path; do
        shasum -a 256 "${path}"
    done > "${NAME}.SHA256SUMS"
)

if [[ "${KEEP_STAGE}" -eq 0 ]]; then
    rm -rf "${STAGE_ROOT}"
fi

echo "==> done"
echo "    release/${NAME}.tar.gz"
echo "    release/${NAME}.sha256"
echo "    release/${NAME}.SHA256SUMS"
