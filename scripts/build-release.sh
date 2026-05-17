#!/usr/bin/env bash
# Stage 6 release packaging for the Tacit toolchain.
#
# Builds a reproducible binary-archive distribution for Linux x86_64 that
# links statically against the system LLVM 19 install (apt: llvm-19-dev).
# Source-builds of LLVM are out of scope; the script fails fast if the
# expected static archives are not present.
#
# Output:
#   release/tacit-<version>-x86_64-unknown-linux-gnu/         (staged tree)
#   release/tacit-<version>-x86_64-unknown-linux-gnu.tar.gz   (archive)
#   release/SHA256SUMS                                        (checksums)
#
# Usage:
#   scripts/build-release.sh [--llvm-prefix /usr/lib/llvm-19] [--keep-stage]

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${REPO_ROOT}"

TARGET_TRIPLE="x86_64-unknown-linux-gnu"
GLIBC_FLOOR="2.35"
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
            sed -n '2,16p' "$0"
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
if [[ "${host_uname}" != "Linux" || "${host_arch}" != "x86_64" ]]; then
    echo "error: this release script only targets ${TARGET_TRIPLE} (host: ${host_uname}/${host_arch})" >&2
    exit 1
fi

# --- LLVM discovery ----------------------------------------------------------
if [[ -z "${LLVM_PREFIX}" ]]; then
    if command -v llvm-config-19 >/dev/null 2>&1; then
        LLVM_PREFIX=$(llvm-config-19 --prefix)
    elif [[ -d /usr/lib/llvm-19 ]]; then
        LLVM_PREFIX=/usr/lib/llvm-19
    else
        echo "error: could not find LLVM 19 (no llvm-config-19, no /usr/lib/llvm-19); install llvm-19-dev" >&2
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
    echo "       install llvm-19-dev (and libpolly-19-dev) to provide them" >&2
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

# --- Cargo build with static LLVM --------------------------------------------
export LLVM_SYS_191_PREFIX="${LLVM_PREFIX}"
# llvm-sys honours --link-static when libLLVM*.a are present in libdir, which
# we already verified above. We unset PREFER_DYNAMIC defensively.
unset LLVM_SYS_191_PREFER_DYNAMIC || true

echo "==> cargo build --release --features tacit-cli/llvm19-1 -p tacit-cli"
cargo build --release --features tacit-cli/llvm19-1 -p tacit-cli

BIN_PATH="target/release/tacit"
if [[ ! -x "${BIN_PATH}" ]]; then
    echo "error: ${BIN_PATH} not produced by cargo build" >&2
    exit 1
fi

# --- Static-link verification ------------------------------------------------
echo "==> ldd $(basename "${BIN_PATH}")"
ldd_out=$(ldd "${BIN_PATH}" 2>&1 || true)
echo "${ldd_out}" | sed 's/^/    /'
if echo "${ldd_out}" | grep -q 'libLLVM'; then
    echo "error: ${BIN_PATH} dynamically links libLLVM (expected static link)" >&2
    exit 1
fi

echo "==> checking glibc symbol floor (<= ${GLIBC_FLOOR})"
required_glibc=$(
    objdump -T "${BIN_PATH}" 2>/dev/null \
        | grep -o 'GLIBC_[0-9]\+\.[0-9]\+' \
        | sed 's/^GLIBC_//' \
        | sort -Vu
)
if [[ -z "${required_glibc}" ]]; then
    echo "error: could not determine required GLIBC symbol versions from ${BIN_PATH}" >&2
    exit 1
fi
max_glibc=$(printf '%s\n' "${required_glibc}" | tail -1)
echo "${required_glibc}" | sed 's/^/    GLIBC_/'
if [[ "$(printf '%s\n%s\n' "${GLIBC_FLOOR}" "${max_glibc}" | sort -V | tail -1)" != "${GLIBC_FLOOR}" ]]; then
    echo "error: ${BIN_PATH} requires GLIBC_${max_glibc}, above release floor GLIBC_${GLIBC_FLOOR}" >&2
    exit 1
fi

# --- Stage layout ------------------------------------------------------------
STAGE_ROOT="release/${NAME}"
rm -rf "${STAGE_ROOT}"
mkdir -p "${STAGE_ROOT}/bin" "${STAGE_ROOT}/share/tacit"
install -m 755 "${BIN_PATH}" "${STAGE_ROOT}/bin/tacit"

# Find the most-recent build script OUT_DIR that contains a staged
# share/tacit/ tree, and copy it into the stage. The build script run above
# is guaranteed to have refreshed exactly one directory.
build_out=$(find "target/release/build" -maxdepth 3 -type d -path '*/tacit-cli-*/out' \
    -exec test -d '{}/share/tacit' \; -print 2>/dev/null \
    | xargs -r -I{} stat -c '%Y {}' {} \
    | sort -nr | head -1 | cut -d' ' -f2-)
if [[ -z "${build_out}" ]]; then
    echo "error: could not locate tacit-cli build script OUT_DIR with staged share/tacit/" >&2
    exit 1
fi
cp -r "${build_out}/share/tacit/." "${STAGE_ROOT}/share/tacit/"

# --- Verify embedded manifest matches staged manifest ------------------------
# Use the binary's own verifier: with TACIT_TOOLCHAIN_ASSET_ROOT pointing at
# the staged share/tacit, `tacit version --format json` reports
# installed_manifest.status, which compares embedded bytes to the staged file
# byte-for-byte (release.rs::installed_manifest).
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
cp -r "${tmp_templates}/executable" "${TEMPLATES_DIR}/executable"

TACIT_TOOLCHAIN_ASSET_ROOT="${STAGE_ROOT}/share/tacit" \
    "${STAGE_ROOT}/bin/tacit" init --template library "${tmp_templates}/library" >/dev/null
cp -r "${tmp_templates}/library" "${TEMPLATES_DIR}/library"

# --- Archive + checksums -----------------------------------------------------
echo "==> creating tarball release/${NAME}.tar.gz"
tar --sort=name \
    --owner=0 --group=0 --numeric-owner \
    --mtime="@$(git -C "${REPO_ROOT}" log -1 --format=%ct 2>/dev/null || echo 0)" \
    -czf "release/${NAME}.tar.gz" -C release "${NAME}"

(
    cd release
    sha256sum "${NAME}.tar.gz" > "${NAME}.sha256"
    # Aggregate file (idempotent rewrite).
    {
        find "${NAME}" -type f -print0 | sort -z | xargs -0 sha256sum
    } > "${NAME}.SHA256SUMS"
)

if [[ "${KEEP_STAGE}" -eq 0 ]]; then
    rm -rf "${STAGE_ROOT}"
fi

echo "==> done"
echo "    release/${NAME}.tar.gz"
echo "    release/${NAME}.sha256"
echo "    release/${NAME}.SHA256SUMS"
