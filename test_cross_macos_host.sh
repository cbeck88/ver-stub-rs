#!/bin/bash
# Test cross-compilation from macOS to Linux (aarch64)
# Uses alvm to run the Linux binary on macOS via Hypervisor.framework
set -e

cd "$(dirname "$0")"

echo "=== Testing ver-stub-rs cross-compilation: macOS -> Linux (aarch64) ==="
echo

# Verify we're on macOS
if [[ "$(uname)" != "Darwin" ]]; then
    echo "This test is meant to be run on macOS, not $(uname)"
    exit 1
fi

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

pass() {
    echo -e "${GREEN}PASS${NC}: $1"
}

fail() {
    echo -e "${RED}FAIL${NC}: $1"
    exit 1
}

warn() {
    echo -e "${YELLOW}WARN${NC}: $1"
}

# Build the ver-stub CLI tool first (on macOS host)
echo "--- Building ver-stub CLI tool ---"
cargo build -p ver-stub-tool 2>&1
VER_STUB="$(pwd)/target/debug/ver-stub"
echo

# Test: Cross-compilation to Linux (aarch64-unknown-linux-musl)
echo "--- Test: Cross-compilation to Linux aarch64 (musl) ---"

# Requires musl cross-linker. On macOS with Homebrew:
#   brew install filosottile/musl-cross/musl-cross

# Add the Linux musl target
rustup target add aarch64-unknown-linux-musl 2>&1

# Clean any previous cross-compiled artifacts
rm -rf ver-stub-example/target/aarch64-unknown-linux-musl 2>/dev/null || true

# Build for Linux musl
(cd ver-stub-example && cargo build --target aarch64-unknown-linux-musl 2>&1)
LINUX_BIN="ver-stub-example/target/aarch64-unknown-linux-musl/debug/ver-stub-example"

if [[ ! -f "$LINUX_BIN" ]]; then
    fail "Linux binary was not created"
fi

echo
echo "--- Checking section in cross-compiled binary ---"

# Try to get section info with ver-stub tool
echo "Attempting to get section info with ver-stub tool..."
OUTPUT=$($VER_STUB get-section-info "$LINUX_BIN" 2>&1) || true
echo "$OUTPUT"
echo

if ! echo "$OUTPUT" | grep -q "size:"; then
    echo
    echo "=== ISSUE CONFIRMED ==="
    echo "ver-stub-build (compiled on macOS) cannot find the section in the Linux ELF binary."
    echo "This is because it's looking for '__TEXT,ver_stub' (Mach-O format)"
    echo "but the ELF binary has 'ver_stub' section."
    echo
    echo "The fix needs to determine the section name based on the TARGET format,"
    echo "not the HOST format."
    fail "Section not found due to format mismatch (this confirms issue #2)"
fi

pass "ver-stub found the section in the ELF binary"

# Patch the Linux binary with ver-stub
echo
echo "--- Patching Linux binary ---"
$VER_STUB --all-git --all-build-time patch "$LINUX_BIN" 2>&1

LINUX_BIN_PATCHED="${LINUX_BIN}.bin"
if [[ ! -f "$LINUX_BIN_PATCHED" ]]; then
    fail "patched Linux binary was not created"
fi

# Verify section still exists after patching
OUTPUT=$($VER_STUB get-section-info "$LINUX_BIN_PATCHED" 2>&1) || true
echo "$OUTPUT"
if echo "$OUTPUT" | grep -q "size:"; then
    pass "patched Linux binary has ver_stub section"
else
    fail "patched Linux binary should have ver_stub section"
fi

# Run the patched binary with alvm
echo
echo "--- Test: Run Linux binary with alvm ---"
if ! command -v alvm &> /dev/null; then
    warn "alvm not found - skipping runtime test"
    warn "Install with: cargo install alvm"
else
    OUTPUT=$(alvm -- "$LINUX_BIN_PATCHED" 2>&1)
    echo "$OUTPUT"
    if echo "$OUTPUT" | grep -q "git sha:" && ! echo "$OUTPUT" | grep -q "git sha:.*not set"; then
        pass "Linux binary runs and shows git sha"
    else
        fail "Linux binary should show git sha"
    fi
    if echo "$OUTPUT" | grep -q "build timestamp:" && ! echo "$OUTPUT" | grep -q "build timestamp:.*not set"; then
        pass "Linux binary shows build timestamp"
    else
        fail "Linux binary should show build timestamp"
    fi
fi

echo
echo -e "${GREEN}=== macOS -> Linux cross-compilation tests passed ===${NC}"
