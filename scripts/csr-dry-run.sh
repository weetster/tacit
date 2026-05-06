#!/usr/bin/env bash
# Hash-stability dry-run for the canonical storage reconciliation.
# For each Mode A + Mode B .tac file:
#   1. canonicalize authoring text  → canonical bytes
#   2. render canonical → authoring text
#   3. re-canonicalize that authoring text → canonical bytes2
#   4. assert bytes == bytes2
#
# Usage: scripts/csr-dry-run.sh [--verbose]

set -euo pipefail

TACIT=${TACIT:-$(cargo build -p tacit-cli --message-format=short 2>/dev/null; echo target/debug/tacit)}
if ! [[ -x "$TACIT" ]]; then
    echo "error: tacit binary not found at $TACIT" >&2
    exit 1
fi

VERBOSE=0
[[ "${1:-}" == "--verbose" ]] && VERBOSE=1

PASS=0
FAIL=0
ERRORS=()

check_file() {
    local src="$1"
    local tmpdir
    tmpdir=$(mktemp -d)
    trap "rm -rf '$tmpdir'" RETURN

    # Step 1: canonicalize authoring → canonical
    cp "$src" "$tmpdir/prog.taca"
    if ! "$TACIT" canonicalize "$tmpdir/prog.taca" 2>"$tmpdir/err1.txt"; then
        ERRORS+=("PARSE_FAIL $src: $(cat "$tmpdir/err1.txt")")
        ((FAIL++)) || true
        return
    fi

    # Step 2: render canonical → authoring text
    if ! "$TACIT" render "$tmpdir/prog.tac" -o "$tmpdir/prog2.taca" 2>"$tmpdir/err2.txt"; then
        ERRORS+=("RENDER_FAIL $src: $(cat "$tmpdir/err2.txt")")
        ((FAIL++)) || true
        return
    fi

    # Step 3: re-canonicalize
    if ! "$TACIT" canonicalize "$tmpdir/prog2.taca" -o "$tmpdir/prog2.tac" 2>"$tmpdir/err3.txt"; then
        ERRORS+=("REPARSE_FAIL $src: $(cat "$tmpdir/err3.txt")")
        ((FAIL++)) || true
        return
    fi

    # Step 4: compare
    if ! cmp -s "$tmpdir/prog.tac" "$tmpdir/prog2.tac"; then
        local b1 b2
        b1=$(cat "$tmpdir/prog.tac")
        b2=$(cat "$tmpdir/prog2.tac")
        ERRORS+=("HASH_DRIFT $src
  canonical1: $b1
  canonical2: $b2")
        ((FAIL++)) || true
        return
    fi

    [[ $VERBOSE -eq 1 ]] && echo "  ok  $src"
    ((PASS++)) || true
}

# Build fresh binary
echo "Building tacit..."
cargo build -p tacit-cli -q
TACIT=target/debug/tacit

echo "Running hash-stability dry-run on Mode A + Mode B files..."
echo ""

# Mode A: corpus reference files
while IFS= read -r f; do
    check_file "$f"
done < <(find corpus -name "reference.tac" -o -name "reference.stdlib.tac" | sort)

# Mode A: examples/phase-3
for f in examples/phase-3/*.tac; do
    check_file "$f"
done

# Mode B: examples/smoke
for f in examples/smoke/*.tac; do
    check_file "$f"
done

# Mode B: plans/test-vectors (if any .tac files exist)
if find plans/test-vectors -name "*.tac" 2>/dev/null | grep -q .; then
    while IFS= read -r f; do
        check_file "$f"
    done < <(find plans/test-vectors -name "*.tac" | sort)
fi

echo "Results: $PASS passed, $FAIL failed"
echo ""

if [[ ${#ERRORS[@]} -gt 0 ]]; then
    echo "Failures:"
    for e in "${ERRORS[@]}"; do
        echo "  $e"
    done
    exit 1
fi

echo "All files round-trip hash-stable. Safe to proceed with repository conversion."
