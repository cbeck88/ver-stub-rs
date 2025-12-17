#!/bin/bash
# Test building for iOS and verify the section name is correct
set -e

cd "$(dirname "$0")"

echo "=== Testing ver-stub iOS build ==="
echo

# Verify we're on macOS
if [[ "$(uname)" != "Darwin" ]]; then
    echo "This test is meant to be run on macOS, not $(uname)"
    exit 1
fi

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

# Build the ver-stub CLI tool first (on macOS host)
echo "--- Building ver-stub CLI tool ---"
cargo build -p ver-stub-tool 2>&1
VER_STUB="$(pwd)/target/debug/ver-stub"
echo

# Test: Build for iOS (aarch64-apple-ios)
echo "--- Test: Build for iOS (aarch64-apple-ios) ---"

# Add the iOS target
rustup target add aarch64-apple-ios 2>&1

# Clean any previous cross-compiled artifacts
rm -rf ver-stub-example/target/aarch64-apple-ios 2>/dev/null || true

# Build for iOS
(cd ver-stub-example && cargo build --target aarch64-apple-ios 2>&1)
IOS_BIN="ver-stub-example/target/aarch64-apple-ios/debug/ver-stub-example"

if [[ ! -f "$IOS_BIN" ]]; then
    fail "iOS binary was not created"
fi

pass "iOS binary built successfully"

echo
echo "--- Checking section in iOS binary ---"

# Get section info with ver-stub tool
OUTPUT=$($VER_STUB get-section-info "$IOS_BIN" 2>&1) || true
echo "$OUTPUT"

# Verify it's detected as Mach-O
if ! echo "$OUTPUT" | grep -q "format: MachO"; then
    fail "iOS binary should be detected as Mach-O format"
fi
pass "iOS binary detected as Mach-O"

# Verify the section name is __TEXT,ver_stub (Mach-O format)
if ! echo "$OUTPUT" | grep -q "section: __TEXT,ver_stub"; then
    fail "iOS binary should have section '__TEXT,ver_stub'"
fi
pass "iOS binary has correct section name '__TEXT,ver_stub'"

# Verify section exists and has correct size
if ! echo "$OUTPUT" | grep -q "size:"; then
    fail "iOS binary should have ver_stub section with size info"
fi
pass "iOS binary has ver_stub section"

# Verify section is read-only
if echo "$OUTPUT" | grep -q "is_writable: false"; then
    pass "iOS section is read-only"
else
    fail "iOS section should be read-only"
fi

echo
echo -e "${GREEN}=== iOS build tests passed ===${NC}"
