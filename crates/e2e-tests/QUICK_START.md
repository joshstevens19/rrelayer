# 🚀 Quick Start Guide - RRelayer E2E Tests

Get your E2E tests running in under 5 minutes!

## ⚡ Super Quick Start

```bash
cd crates/e2e-tests
./run-tests.sh
```

That's it! The script handles everything automatically.

## 📋 What You Need

1. **Rust** (latest stable)
2. **Foundry** (for Anvil)
3. **PostgreSQL** (for RRelayer)

## 🛠️ Step-by-Step Setup

### 1. Install Dependencies
```bash
# Install Foundry
curl -L https://foundry.paradigm.xyz | bash
foundryup

# Verify installation
anvil --version
```

### 2. Verify Setup
```bash
cd crates/e2e-tests
./verify-setup.sh
```

### 3. Run Tests
```bash
# Automated (recommended)
./run-tests.sh

# Manual control
make test-full

# Development mode
RUST_LOG=debug ./run-tests.sh
```

## ✅ What Gets Tested

- ✅ **Relayer creation & funding** - Auto-funded with 10 ETH
- ✅ **Gas estimation** - Two-phase estimation for all tx types  
- ✅ **Transaction lifecycle** - Pending → InMempool → Completed
- ✅ **Error handling** - Failed transactions and edge cases
- ✅ **Gas mechanics** - EIP-1559, Legacy, and gas bumping
- ✅ **Batch processing** - Multiple concurrent transactions
- ✅ **API coverage** - Complete REST API testing

## 🎯 Expected Output

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

## 🔧 Options

| Command | Description |
|---------|-------------|
| `./run-tests.sh` | Full automated test |
| `make test-full` | Using Makefile |
| `make test-quick` | Assume services running |
| `./verify-setup.sh` | Check dependencies |
| `make status` | Check service status |
| `make logs` | View service logs |

## 🚨 Common Issues

**"Anvil not found"**
```bash
curl -L https://foundry.paradigm.xyz | bash && foundryup
```

**"Connection refused"**
- Check ports: RRelayer (3000), Anvil (8545)
- Run `make status` to check services

**"Gas limit not set"**
- ✅ **FIXED** - Automatic gas estimation now works

**"Insufficient funds"**
- ✅ **FIXED** - Relayers auto-funded with 10 ETH

## 📚 More Info

- **Full documentation**: [README.md](README.md)
- **Setup details**: [SETUP_COMPLETE.md](SETUP_COMPLETE.md)
- **Scripts**: `run-tests.sh`, `verify-setup.sh`
- **Makefile**: `make help` for all targets

---

**Need help?** Check the troubleshooting section in [README.md](README.md#common-issues--solutions) 🆘