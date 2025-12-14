#!/bin/bash
set -e

cd "$(dirname "$0")"

echo "=== Testing ver-stub-rs cross-compilation ==="
echo

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m' # No Color

pass() {
    echo -e "${GREEN}PASS${NC}: $1"
}

fail() {
    echo -e "${RED}FAIL${NC}: $1"
    exit 1
}

# Build the ver-stub CLI tool first
echo "--- Building ver-stub CLI tool ---"
cargo build -p ver-stub-tool 2>&1
VER_STUB="$(pwd)/target/debug/ver-stub"
echo

# Test: Cross-compilation to ARM64 (aarch64-unknown-linux-gnu)
# We can't run the binary, but we can verify the section is correctly patched
echo "--- Test: Cross-compilation to ARM64 ---"

# Add the ARM64 target
rustup target add aarch64-unknown-linux-gnu 2>&1

# Clean any previous cross-compiled artifacts
rm -rf ver-stub-example/target/aarch64-unknown-linux-gnu 2>/dev/null || true

# Build for ARM64
(cd ver-stub-example && cargo build --target aarch64-unknown-linux-gnu 2>&1)

ARM_BIN="ver-stub-example/target/aarch64-unknown-linux-gnu/debug/ver-stub-example"
if [[ ! -f "$ARM_BIN" ]]; then
    fail "ARM64 binary was not created"
fi

# Verify section exists in cross-compiled binary
OUTPUT=$($VER_STUB get-section-info "$ARM_BIN" 2>&1)
echo "$OUTPUT"
if echo "$OUTPUT" | grep -q "size:"; then
    pass "cross-compiled ARM64 binary has .ver_stub section"
else
    fail "cross-compiled ARM64 binary should have .ver_stub section"
fi

# Patch the ARM binary with ver-stub
$VER_STUB --all-git --all-build-time patch "$ARM_BIN" 2>&1

ARM_BIN_PATCHED="ver-stub-example/target/aarch64-unknown-linux-gnu/debug/ver-stub-example.bin"
if [[ ! -f "$ARM_BIN_PATCHED" ]]; then
    fail "patched ARM64 binary was not created"
fi

# Verify section still exists after patching
OUTPUT=$($VER_STUB get-section-info "$ARM_BIN_PATCHED" 2>&1)
echo "$OUTPUT"
if echo "$OUTPUT" | grep -q "size:"; then
    pass "patched ARM64 binary has .ver_stub section"
else
    fail "patched ARM64 binary should have .ver_stub section"
fi

# Verify section is read-only
if echo "$OUTPUT" | grep -q "is_writable: false"; then
    pass "ARM64 section is read-only"
else
    fail "ARM64 section should be read-only"
fi

# Test: Run the patched ARM64 binary with QEMU user-mode emulation
echo
echo "--- Test: Run ARM64 binary with QEMU ---"
if ! command -v qemu-aarch64 &> /dev/null; then
    fail "qemu-aarch64 not found. Install with: sudo apt-get install qemu-user"
fi

OUTPUT=$(qemu-aarch64 -L /usr/aarch64-linux-gnu "$ARM_BIN_PATCHED" 2>&1)
echo "$OUTPUT"
if echo "$OUTPUT" | grep -q "git sha:" && ! echo "$OUTPUT" | grep -q "git sha:.*not set"; then
    pass "ARM64 binary runs and shows git sha"
else
    fail "ARM64 binary should show git sha"
fi
if echo "$OUTPUT" | grep -q "build timestamp:" && ! echo "$OUTPUT" | grep -q "build timestamp:.*not set"; then
    pass "ARM64 binary shows build timestamp"
else
    fail "ARM64 binary should show build timestamp"
fi

echo
echo -e "${GREEN}=== Cross-compilation tests passed ===${NC}"
