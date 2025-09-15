# RRelayer E2E Tests

This crate contains comprehensive end-to-end tests for the RRelayer system using Anvil as a local blockchain.

## Overview

The e2e tests verify the complete transaction lifecycle with realistic blockchain conditions:
1. **Relayer creation and automatic funding** - Creates relayers and funds them with ETH for testing
2. **Gas limit estimation** - Tests proper gas estimation and limit setting for all transaction types
3. **Transaction submission and queuing** - Full transaction pipeline testing
4. **Blockchain execution** - Real transaction execution on Anvil with proper gas mechanics
5. **Status tracking and receipt verification** - Complete lifecycle monitoring
6. **Error handling and edge cases** - Gas estimation failures, insufficient funds, etc.

## Prerequisites

### Required Dependencies

1. **Rust**: Latest stable Rust toolchain
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source ~/.cargo/env
   ```

2. **Foundry**: Required for Anvil blockchain
   ```bash
   curl -L https://foundry.paradigm.xyz | bash
   foundryup
   ```

3. **PostgreSQL**: Required for RRelayer (if running locally)
   ```bash
   # macOS with Homebrew
   brew install postgresql
   brew services start postgresql
   
   # Create database
   createdb rrelayer
   ```

### Environment Setup

1. **Environment Variables**: Create `.env` file in the core directory:
   ```bash
   DATABASE_URL=postgresql://postgres:password@localhost:5432/rrelayer
   RRELAYER_AUTH_USERNAME=your_username
   RRELAYER_AUTH_PASSWORD=your_password
   RAW_DANGEROUS_MNEMONIC="test test test test test test test test test test test junk"
   ```

2. **RRelayer Configuration**: Ensure you have a valid `rrelayer.yaml` configuration file

## Running Tests

### 🚀 Automated (Recommended)
The easiest way to run tests with full service management:

```bash
cd crates/e2e-tests

# Option 1: Full automated test suite
./run-tests.sh

# Option 2: Using Makefile
make test-full

# Option 3: Quick verification of setup
./verify-setup.sh
```

### ⚡ Quick Start (Manual)
If you prefer to manage services yourself:

```bash
# Terminal 1: Start Anvil
anvil --host 0.0.0.0 --port 8545 --accounts 10 \
  --mnemonic "test test test test test test test test test test test junk" \
  --block-time 2 --gas-limit 30000000

# Terminal 2: Start RRelayer (ensure .env is configured)
cd crates/core
cargo run

# Terminal 3: Run tests
cd crates/e2e-tests
cargo run --bin e2e-runner
```

### 🔧 Custom Configuration
```bash
# Set environment variables
export RRELAYER_BASE_URL="http://localhost:3001"
export ANVIL_PORT=8546
export TEST_TIMEOUT_SECONDS=60
export RUST_LOG=debug

# Use existing services
START_SERVICES=false ./run-tests.sh
```

### 🏗️ CI/CD Integration
```yaml
# GitHub Actions example
- name: Run E2E Tests
  env:
    DATABASE_URL: postgresql://postgres:postgres@localhost:5432/rrelayer_test
  run: |
    cd crates/e2e-tests
    make ci-test
```

## Test Scenarios

### 1. Basic Relayer Creation & Funding
- ✅ Create relayer via API
- ✅ Verify relayer has valid ID and address
- ✅ Automatically fund relayer with 10 ETH for testing
- ✅ Verify address format is valid Ethereum address

### 2. Simple ETH Transfer with Gas Estimation
- ✅ Estimate gas limit using temporary high-limit transaction
- ✅ Create final transaction with estimated gas + 20% buffer
- ✅ Send ETH transfer through relayer
- ✅ Verify transaction is submitted to blockchain with correct gas limit
- ✅ Verify transaction completes successfully

### 3. Contract Interaction
- ✅ Test gas estimation for contract calls
- ✅ Send contract call through relayer with proper gas limits
- ✅ Verify calldata is properly encoded
- ✅ Verify contract interaction completes

### 4. Transaction Status Tracking
- ✅ Monitor transaction through all states
- ✅ Verify status transitions (pending → inmempool → completed)
- ✅ Verify proper gas limit estimation in transaction details
- ✅ Verify receipt is available when completed

### 5. Failed Transaction Handling
- ✅ Submit transaction that should fail (invalid recipient)
- ✅ Test gas estimation for failing transactions
- ✅ Verify proper error handling
- ✅ Verify failed status is reported correctly

### 6. Gas Estimation & Limits
- ✅ Test two-phase gas estimation (temp tx → estimate → final tx)
- ✅ Verify reasonable gas usage for simple transfers (21,000 base + 20% buffer)
- ✅ Verify gas estimation for contract interactions
- ✅ Test both Legacy and EIP-1559 transaction types
- ✅ Test gas limit edge cases and error handling

### 7. Transaction Replacement & Gas Bumping
- ✅ Test transaction replacement with higher gas prices
- ✅ Verify nonce handling across replacements
- ✅ Verify gas price bumping mechanisms
- ✅ Test realistic blockchain conditions with varying gas prices

### 8. Batch Transactions
- ✅ Submit multiple transactions rapidly
- ✅ Verify proper gas estimation for each transaction
- ✅ Verify proper ordering and nonce management
- ✅ Verify all transactions complete with correct gas limits

### 9. Relayer Limits & Management
- ✅ Test pagination of transaction history
- ✅ Test pending transaction counts
- ✅ Test relayer capacity limits
- ✅ Verify relayer funding and balance management

## Configuration

The tests use `E2ETestConfig` with these defaults:

```rust
pub struct E2ETestConfig {
    pub anvil_port: u16,              // 8545
    pub rrelayer_base_url: String,    // "http://localhost:3000"
    pub test_timeout_seconds: u64,    // 30
    pub chain_id: u64,               // 31337 (Anvil default)
    // ... anvil accounts and keys
}
```

## Architecture

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   E2E Tests     │    │    RRelayer     │    │     Anvil       │
│                 │    │                 │    │                 │
│ ┌─────────────┐ │    │ ┌─────────────┐ │    │ ┌─────────────┐ │
│ │ Test Runner │◄┼────┤ │ HTTP API    │ │    │ │ Blockchain  │ │
│ └─────────────┘ │    │ └─────────────┘ │    │ └─────────────┘ │
│ ┌─────────────┐ │    │ ┌─────────────┐ │    │ ┌─────────────┐ │
│ │ Relayer     │◄┼────┤ │ TX Queue    │◄┼────┤ │ EVM + Gas   │ │
│ │ Funding     │ │    │ │ + Gas Est.  │ │    │ │ Mechanics   │ │
│ └─────────────┘ │    │ └─────────────┘ │    │ └─────────────┘ │
│ ┌─────────────┐ │    │ ┌─────────────┐ │    │ ┌─────────────┐ │
│ │ Anvil Mgr   │◄┼────┤ │ Gas Oracle  │◄┼────┤ │ EIP-1559    │ │
│ │ + Gas API   │ │    │ │ + Estimator │ │    │ │ Fee Market  │ │
│ └─────────────┘ │    │ └─────────────┘ │    │ └─────────────┘ │
│ ┌─────────────┐ │    │ ┌─────────────┐ │    │                 │
│ │ Contract    │◄┼────┤ │ Providers   │◄┼────┤                 │
│ │ Interactor  │ │    │ └─────────────┘ │    │                 │
│ └─────────────┘ │    └─────────────────┘    └─────────────────┘
└─────────────────┘
```

## Key Features

### 🔥 **Gas Limit Estimation**
- **Two-phase estimation**: Create temporary transaction with high gas limit → estimate actual usage → create final transaction
- **20% buffer**: Automatically adds safety margin to estimated gas limits
- **Multiple transaction types**: Supports Legacy, EIP-1559, and EIP-4844 blob transactions
- **Realistic testing**: Uses actual Anvil blockchain for accurate gas calculations

### 💰 **Automatic Relayer Funding**
- **Auto-funding**: Each relayer automatically receives 10 ETH for testing
- **Transaction confirmation**: Waits for funding transactions to be mined
- **Realistic balances**: Tests run with sufficient funds to avoid "insufficient funds" errors

### ⛓️ **Blockchain Simulation**
- **Anvil configuration**: Realistic 2-second block times, EIP-1559 fee market
- **Gas price variations**: Pre-mines blocks with varying gas prices (1-5 gwei)
- **Transaction history**: Creates realistic transaction history for gas estimation

## Debugging

### Enable Debug Logs
```bash
RUST_LOG=debug cargo run --bin e2e-runner
```

### Manual Testing
```bash
# Start Anvil manually
anvil --host 0.0.0.0 --port 8545

# Start RRelayer
cd crates/core && cargo run

# Run specific test
cd crates/e2e-tests
RUST_LOG=debug cargo run --bin e2e-runner
```

### Common Issues & Solutions

#### 1. **"Anvil not found"**
```bash
# Install Foundry
curl -L https://foundry.paradigm.xyz | bash
foundryup
# Verify installation
anvil --version
```

#### 2. **"Connection refused"** 
- ✅ Check RRelayer is running on port 3000: `curl http://localhost:3000/health`
- ✅ Check Anvil is running on port 8545: `curl -X POST -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","method":"eth_chainId","params":[],"id":1}' http://localhost:8545`
- ✅ Verify firewall settings allow local connections

#### 3. **"Gas limit not set" errors**
- ✅ **Fixed**: Gas limit estimation now works correctly
- ✅ The system uses two-phase estimation (temp tx → estimate → final tx)
- ✅ Both Legacy and EIP-1559 transactions supported

#### 4. **"Insufficient funds for gas" errors**
- ✅ **Fixed**: Relayers are automatically funded with 10 ETH
- ✅ Check if funding transaction completed: Look for "Successfully funded relayer" in logs
- ✅ Verify Anvil accounts have sufficient balance

#### 5. **"Transaction timeout"**
- ✅ Increase `TEST_TIMEOUT_SECONDS`: `export TEST_TIMEOUT_SECONDS=60`
- ✅ Check Anvil block time: Default is 2 seconds for realistic conditions
- ✅ Enable debug logging: `RUST_LOG=debug ./run-tests.sh`

#### 6. **"Database connection errors"**
```bash
# Start PostgreSQL
brew services start postgresql  # macOS
sudo systemctl start postgresql  # Linux

# Create database
createdb rrelayer

# Check connection
psql -d rrelayer -c "SELECT 1;"
```

#### 7. **"Compilation errors"**
```bash
# Clean and rebuild
cargo clean
cargo build --bin e2e-runner

# Check dependencies
./verify-setup.sh
```

#### 8. **"Authentication failed"**
- ✅ Ensure `.env` file has correct credentials
- ✅ Check `RRELAYER_AUTH_USERNAME` and `RRELAYER_AUTH_PASSWORD` are set
- ✅ Verify RRelayer is using the same credentials

## Adding New Tests

1. Add test function to `TestRunner` in `src/test_scenarios.rs`
2. Add test to the scenarios array in `run_all_tests()`
3. Follow the pattern: setup → execute → verify → cleanup

Example:
```rust
async fn test_my_new_scenario(&self) -> Result<()> {
    // Setup: Create and fund relayer automatically
    let relayer = self.create_and_fund_relayer("test-relayer").await?;
    let relayer_id = relayer["id"].as_str().context("Missing relayer ID")?;
    
    // Execute: Send transaction (gas estimation happens automatically)
    let tx = self.relayer_client.send_transaction(
        relayer_id,
        "0x70997970C51812dc3A010C7d01b50e0d17dc79C8", // recipient
        Some("1000000000000000000"), // 1 ETH
        None // no data
    ).await?;
    
    // Verify: Check transaction completion and gas usage
    self.wait_for_transaction_completion(&tx.id).await?;
    
    Ok(())
}
```

## 🎯 Expected Test Results

When all tests pass, you should see:

```
🧪 Running E2E test scenarios...
✅ basic_relayer_creation: PASSED
✅ simple_eth_transfer: PASSED  
✅ contract_interaction: PASSED
✅ transaction_status_tracking: PASSED
✅ failed_transaction_handling: PASSED
✅ gas_estimation: PASSED
✅ transaction_replacement: PASSED
✅ batch_transactions: PASSED
✅ relayer_limits: PASSED

📊 Test Results: 9 passed, 0 failed
🎉 All tests passed!
```

## 📋 Recent Updates

### ✅ Gas Limit Estimation (Fixed)
- Implemented two-phase gas estimation to resolve circular dependency
- Added temporary transaction creation with high gas limits for estimation
- Fixed both `send_transaction` and `add_transaction` code paths
- Added support for Legacy, EIP-1559, and EIP-4844 transaction types

### ✅ Relayer Funding (Implemented)
- Automatic funding of relayers with 10 ETH from Anvil accounts
- Transaction confirmation and mining verification
- Helper functions for create-and-fund workflow

### ✅ Realistic Blockchain Conditions
- Enhanced Anvil configuration with EIP-1559 fee market
- 2-second block times for realistic conditions  
- Pre-mining blocks with varying gas prices (1-5 gwei)
- Priority transaction ordering for proper gas bumping tests

---

**Your E2E testing suite provides comprehensive coverage of the entire RRelayer transaction lifecycle with realistic blockchain conditions!** 🚀
```