#!/bin/bash
set -e

echo "🔧 Verifying E2E Test Setup"

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

print_check() {
    echo -e "${GREEN}✅${NC} $1"
}

print_fail() {
    echo -e "${RED}❌${NC} $1"
}

print_info() {
    echo -e "${YELLOW}ℹ️${NC} $1"
}

# Check 1: Rust toolchain
print_info "Checking Rust toolchain..."
if cargo --version >/dev/null 2>&1; then
    print_check "Cargo found: $(cargo --version)"
else
    print_fail "Cargo not found"
    exit 1
fi

# Check 2: Foundry (Anvil)
print_info "Checking Foundry installation..."
if command -v anvil >/dev/null 2>&1; then
    print_check "Anvil found: $(anvil --version | head -n1)"
else
    print_fail "Anvil not found. Install with: curl -L https://foundry.paradigm.xyz | bash"
    exit 1
fi

# Check 3: Project structure
print_info "Checking project structure..."
if [ -f "Cargo.toml" ]; then
    print_check "Cargo.toml exists"
else
    print_fail "Cargo.toml missing"
    exit 1
fi

if [ -f "src/main.rs" ]; then
    print_check "main.rs exists"
else
    print_fail "main.rs missing"
    exit 1
fi

# Check 4: Dependencies compilation
print_info "Checking dependencies compilation..."
if cargo check >/dev/null 2>&1; then
    print_check "All dependencies compile successfully"
else
    print_fail "Compilation errors found"
    cargo check
    exit 1
fi

# Check 5: E2E binary build
print_info "Building E2E test binary..."
if cargo build --bin e2e-runner >/dev/null 2>&1; then
    print_check "E2E test binary builds successfully"
else
    print_fail "Failed to build E2E test binary"
    exit 1
fi

# Check 6: Core crate availability
print_info "Checking core crate availability..."
if [ -d "../core" ]; then
    print_check "Core crate found"
    cd ../core
    if cargo check >/dev/null 2>&1; then
        print_check "Core crate compiles successfully"
    else
        print_fail "Core crate has compilation errors"
    fi
    cd ../e2e-tests
else
    print_fail "Core crate not found (this is OK if running standalone)"
fi

# Check 7: Test structure
print_info "Checking test modules..."
for module in anvil_manager relayer_client contract_interactions test_scenarios; do
    if [ -f "src/${module}.rs" ]; then
        print_check "${module}.rs exists"
    else
        print_fail "${module}.rs missing"
        exit 1
    fi
done

print_info "Setup verification complete!"
echo ""
echo "🚀 Ready to run E2E tests!"
echo ""
echo "Quick start:"
echo "  ./run-tests.sh                    # Full test with service startup"
echo "  START_SERVICES=false ./run-tests.sh  # Use existing services"
echo "  make test-full                    # Using Makefile"
echo ""
echo "Manual testing:"
echo "  1. Start Anvil: anvil --port 8545"
echo "  2. Start RRelayer: cd ../core && cargo run"
echo "  3. Run tests: cargo run --bin e2e-runner"