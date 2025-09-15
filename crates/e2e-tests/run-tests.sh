#!/bin/bash
set -e

echo "🚀 Starting RRelayer E2E Test Suite"

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Cleanup function
cleanup() {
    print_status "Cleaning up processes..."
    
    if [ -f anvil.pid ]; then
        kill $(cat anvil.pid) 2>/dev/null || true
        rm -f anvil.pid anvil.log
        print_status "Stopped Anvil"
    fi
    
    if [ -f rrelayer.pid ]; then
        kill $(cat rrelayer.pid) 2>/dev/null || true
        rm -f rrelayer.pid rrelayer.log
        print_status "Stopped RRelayer"
    fi
}

# Set trap for cleanup on exit
trap cleanup EXIT

# Check dependencies
print_status "Checking dependencies..."

if ! command -v anvil &> /dev/null; then
    print_error "anvil not found. Please install Foundry:"
    print_error "curl -L https://foundry.paradigm.xyz | bash && foundryup"
    exit 1
fi

if ! command -v cargo &> /dev/null; then
    print_error "cargo not found. Please install Rust."
    exit 1
fi

# Build the e2e tests
print_status "Building E2E tests..."
cargo build --bin e2e-runner

# Check if we should start services or assume they're running
START_SERVICES=${START_SERVICES:-true}

if [ "$START_SERVICES" = "true" ]; then
    # Start Anvil
    print_status "Starting Anvil blockchain..."
    anvil --host 0.0.0.0 --port 8545 --accounts 10 \
        --mnemonic "test test test test test test test test test test test junk" \
        --block-time 1 --gas-limit 30000000 > anvil.log 2>&1 &
    echo $! > anvil.pid
    print_status "Anvil started (PID: $(cat anvil.pid))"
    
    # Wait for Anvil to be ready
    print_status "Waiting for Anvil to be ready..."
    for i in {1..10}; do
        if curl -s -X POST -H "Content-Type: application/json" \
           -d '{"jsonrpc":"2.0","method":"eth_chainId","params":[],"id":1}' \
           http://localhost:8545 >/dev/null 2>&1; then
            print_status "Anvil is ready!"
            break
        fi
        if [ $i -eq 10 ]; then
            print_error "Anvil failed to start"
            exit 1
        fi
        sleep 1
    done
    
    # Start RRelayer (if available)
    if [ -f "../core/Cargo.toml" ]; then
        print_status "Starting RRelayer service..."
        cd ../core
        cargo run > ../e2e-tests/rrelayer.log 2>&1 &
        echo $! > ../e2e-tests/rrelayer.pid
        cd ../e2e-tests
        print_status "RRelayer started (PID: $(cat rrelayer.pid))"
        
        # Wait for RRelayer to be ready
        print_status "Waiting for RRelayer to be ready..."
        for i in {1..15}; do
            if curl -s -f http://localhost:3000/health >/dev/null 2>&1; then
                print_status "RRelayer is ready!"
                break
            fi
            if [ $i -eq 15 ]; then
                print_warning "RRelayer health check failed, but continuing with tests..."
                break
            fi
            sleep 2
        done
    else
        print_warning "RRelayer core not found, assuming it's running elsewhere..."
    fi
else
    print_status "Using existing services (START_SERVICES=false)"
fi

# Run the tests
print_status "Running E2E tests..."
export RUST_LOG=${RUST_LOG:-info}
export RRELAYER_BASE_URL=${RRELAYER_BASE_URL:-http://localhost:3000}
export TEST_TIMEOUT_SECONDS=${TEST_TIMEOUT_SECONDS:-30}

if cargo run --bin e2e-runner; then
    print_status "🎉 All E2E tests passed!"
    exit 0
else
    print_error "❌ E2E tests failed"
    
    # Show logs if tests failed
    if [ -f anvil.log ]; then
        print_status "Anvil logs:"
        tail -n 20 anvil.log
    fi
    
    if [ -f rrelayer.log ]; then
        print_status "RRelayer logs:"
        tail -n 20 rrelayer.log
    fi
    
    exit 1
fi