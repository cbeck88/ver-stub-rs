#!/bin/bash
# Test running a Linux binary that was cross-compiled on macOS
# This script runs on Linux and executes the artifact with QEMU
set -e

cd "$(dirname "$0")"

echo "=== Testing macOS-cross-compiled Linux binary ==="
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

# The binary path is passed as an argument
LINUX_BIN="$1"

if [[ -z "$LINUX_BIN" ]]; then
    fail "Usage: $0 <path-to-linux-binary>"
fi

if [[ ! -f "$LINUX_BIN" ]]; then
    fail "Binary not found: $LINUX_BIN"
fi

echo "Binary: $LINUX_BIN"
file "$LINUX_BIN"
echo

# Make sure it's executable
chmod +x "$LINUX_BIN"

# Check if we need QEMU (running on x86_64 but binary is aarch64)
HOST_ARCH=$(uname -m)
BIN_ARCH=$(file "$LINUX_BIN" | grep -o 'ARM aarch64\|x86-64' || echo "unknown")

echo "Host architecture: $HOST_ARCH"
echo "Binary architecture: $BIN_ARCH"
echo

if [[ "$BIN_ARCH" == "ARM aarch64" && "$HOST_ARCH" == "x86_64" ]]; then
    echo "--- Running aarch64 binary with QEMU ---"
    if ! command -v qemu-aarch64-static &> /dev/null && ! command -v qemu-aarch64 &> /dev/null; then
        fail "qemu-aarch64 not found. Install with: sudo apt-get install qemu-user-static"
    fi
    # Use static version if available (works better with musl binaries)
    if command -v qemu-aarch64-static &> /dev/null; then
        QEMU_CMD="qemu-aarch64-static"
    else
        QEMU_CMD="qemu-aarch64"
    fi
    OUTPUT=$($QEMU_CMD "$LINUX_BIN" 2>&1)
else
    echo "--- Running binary natively ---"
    OUTPUT=$("$LINUX_BIN" 2>&1)
fi

echo "$OUTPUT"
echo

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

echo
echo -e "${GREEN}=== macOS-cross-compiled artifact test passed ===${NC}"
