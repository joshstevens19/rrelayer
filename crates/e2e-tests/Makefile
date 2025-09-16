# RRelayer E2E Tests Makefile

.PHONY: help install-deps clean build \
        start-postgres stop-postgres reset-db \
        start-anvil stop-anvil \
        start-rrelayer start-rrelayer-debug stop-rrelayer \
        test-full test-quick run-tests run-tests-debug \
        test-core test-comprehensive test-status-states test-allowlist test-config test-edge-cases \
        run-test run-test-debug \
        test-basic test-basic-debug test-transfer test-transfer-debug \
        test-contract test-contract-debug test-failed test-failed-debug \
        logs logs-live logs-rrelayer logs-anvil logs-clear check-services

help: ## Show this help message
	@echo "🚀 RRelayer E2E Tests"
	@echo "Available targets:"
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  \033[36m%-25s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

# =============================================================================
# SETUP & DEPENDENCIES
# =============================================================================

install-deps: ## Install required dependencies (Foundry)
	@echo "Installing Foundry..."
	@curl -L https://foundry.paradigm.xyz | bash
	@foundryup

build: ## Build the E2E test binary
	@cargo build --bin e2e-runner

clean: stop-postgres stop-anvil stop-rrelayer ## Clean up all running processes and files
	@rm -f *.pid *.log
	@cargo clean
	@echo "Cleanup complete"

# =============================================================================
# DATABASE MANAGEMENT
# =============================================================================

start-postgres: ## Start PostgreSQL database
	@echo "Starting PostgreSQL database..."
	@cd ../.. && docker-compose up -d postgresql
	@echo "Waiting for PostgreSQL to be ready..."
	@for i in $$(seq 1 30); do \
		if PGPASSWORD=rrelayer psql -h localhost -p 5447 -U postgres -d postgres -c "SELECT 1;" >/dev/null 2>&1; then \
			echo "PostgreSQL is ready!"; \
			break; \
		fi; \
		echo "Attempt $$i/30 - PostgreSQL not ready yet..."; \
		sleep 1; \
	done

stop-postgres: ## Stop PostgreSQL database
	@echo "Stopping PostgreSQL database..."
	@cd ../.. && docker-compose down postgresql

reset-db: ## Reset database (drop all schemas and recreate)
	@echo "Resetting database..."
	@PGPASSWORD=rrelayer psql -h localhost -p 5447 -U postgres -d postgres -c "DROP SCHEMA IF EXISTS public CASCADE; CREATE SCHEMA public;" >/dev/null 2>&1 || true
	@PGPASSWORD=rrelayer psql -h localhost -p 5447 -U postgres -d postgres -c "DROP SCHEMA IF EXISTS authentication CASCADE;" >/dev/null 2>&1 || true
	@PGPASSWORD=rrelayer psql -h localhost -p 5447 -U postgres -d postgres -c "DROP SCHEMA IF EXISTS gas CASCADE;" >/dev/null 2>&1 || true
	@PGPASSWORD=rrelayer psql -h localhost -p 5447 -U postgres -d postgres -c "DROP SCHEMA IF EXISTS network CASCADE;" >/dev/null 2>&1 || true
	@PGPASSWORD=rrelayer psql -h localhost -p 5447 -U postgres -d postgres -c "DROP SCHEMA IF EXISTS relayers CASCADE;" >/dev/null 2>&1 || true
	@PGPASSWORD=rrelayer psql -h localhost -p 5447 -U postgres -d postgres -c "DROP SCHEMA IF EXISTS signing CASCADE;" >/dev/null 2>&1 || true
	@PGPASSWORD=rrelayer psql -h localhost -p 5447 -U postgres -d postgres -c "DROP SCHEMA IF EXISTS transactions CASCADE;" >/dev/null 2>&1 || true
	@PGPASSWORD=rrelayer psql -h localhost -p 5447 -U postgres -d postgres -c "DROP SCHEMA IF EXISTS user_rate_limiting CASCADE;" >/dev/null 2>&1 || true
	@PGPASSWORD=rrelayer psql -h localhost -p 5447 -U postgres -d postgres -c "DROP SCHEMA IF EXISTS webhooks CASCADE;" >/dev/null 2>&1 || true
	@echo "Database reset complete"

# =============================================================================
# ANVIL BLOCKCHAIN
# =============================================================================

start-anvil: ## Start Anvil blockchain
	@echo "Starting Anvil..."
	@anvil --host 0.0.0.0 --port 8545 --accounts 10 \
		--mnemonic "test test test test test test test test test test test junk" \
		--block-time 1 --gas-limit 30000000 > anvil.log 2>&1 &
	@echo $$! > anvil.pid
	@sleep 2
	@echo "Anvil started (PID: $$(cat anvil.pid))"

stop-anvil: ## Stop Anvil blockchain
	@if [ -f anvil.pid ]; then \
		echo "Stopping Anvil (PID: $$(cat anvil.pid))..."; \
		kill $$(cat anvil.pid) 2>/dev/null || true; \
		rm -f anvil.pid anvil.log; \
	fi

# =============================================================================
# RRELAYER SERVICE
# =============================================================================

start-rrelayer: ## Start RRelayer service (logs to file)
	@echo "Starting RRelayer (logs: rrelayer.log)..."
	@cd ../cli && RUST_LOG=warn,rrelayer_core=warn RUST_BACKTRACE=1 cargo run -- start --path $(CURDIR) > $(CURDIR)/rrelayer.log 2>&1 &
	@echo $$! > rrelayer.pid
	@sleep 5
	@echo "RRelayer started (PID: $$(cat rrelayer.pid))"
	@echo "Waiting for RRelayer to be ready..."
	@for i in $$(seq 1 10); do \
		if curl -s -f http://localhost:3000/health >/dev/null 2>&1; then \
			echo "RRelayer is ready!"; \
			break; \
		fi; \
		echo "Attempt $$i/10 - RRelayer not ready yet..."; \
		sleep 2; \
	done

start-rrelayer-debug: ## Start RRelayer with debug logs (logs to file)
	@echo "Starting RRelayer with debug logging (logs: rrelayer.log)..."
	@cd ../cli && RUST_LOG=debug,rrelayer_core=debug RUST_BACKTRACE=1 cargo run -- start --path $(CURDIR) > $(CURDIR)/rrelayer.log 2>&1 &
	@echo $$! > rrelayer.pid
	@sleep 5
	@echo "RRelayer started (PID: $$(cat rrelayer.pid))"
	@echo "Waiting for RRelayer to be ready..."
	@for i in $$(seq 1 10); do \
		if curl -s -f http://localhost:3000/health >/dev/null 2>&1; then \
			echo "RRelayer is ready!"; \
			break; \
		fi; \
		echo "Attempt $$i/10 - RRelayer not ready yet..."; \
		sleep 2; \
	done

stop-rrelayer: ## Stop RRelayer service
	@if [ -f rrelayer.pid ]; then \
		echo "Stopping RRelayer (PID: $$(cat rrelayer.pid))..."; \
		kill $$(cat rrelayer.pid) 2>/dev/null || true; \
		rm -f rrelayer.pid rrelayer.log; \
	fi

# =============================================================================
# TEST RUNNERS - MAIN TARGETS
# =============================================================================

test-full: start-postgres start-anvil start-rrelayer run-tests stop-rrelayer stop-anvil stop-postgres ## Run complete test suite with all services

test-quick: ## Run tests assuming services are already running
	@echo "Running E2E tests (assuming services are running)..."
	@RUST_LOG=info,rrelayer_core=info cargo run --bin e2e-runner

run-tests: ## Run all E2E tests with clean Jest-like output
	@echo "Running E2E tests with clean output..."
	@make reset-db
	@RUST_LOG=error cargo run --bin e2e-runner

run-tests-debug: ## Run all E2E tests with debug output
	@echo "Running E2E tests with debug output..."
	@make reset-db  
	@RUST_LOG=info cargo run --bin e2e-runner

# =============================================================================
# TEST CATEGORIES - GROUPED BY FUNCTIONALITY
# =============================================================================

test-core: ## Run core functionality tests
	@echo "Running core functionality tests..."
	@make run-test TEST=basic_relayer_creation
	@make run-test TEST=simple_eth_transfer
	@make run-test TEST=contract_interaction
	@make run-test TEST=gas_estimation
	@make run-test TEST=transaction_replacement

test-status-states: ## Run all transaction status state tests
	@echo "Running all transaction status state tests..."
	@make run-test TEST=transaction_status_pending
	@make run-test TEST=transaction_status_inmempool
	@make run-test TEST=transaction_status_mined
	@make run-test TEST=transaction_status_confirmed
	@make run-test TEST=transaction_status_failed
	@make run-test TEST=transaction_status_expired

test-allowlist: ## Run all allowlist tests
	@echo "Running all allowlist tests..."
	@make run-test TEST=allowlist_restrictions
	@make run-test TEST=allowlist_edge_cases
	@make run-test TEST=allowlist_management

test-config: ## Run all relayer configuration tests
	@echo "Running all relayer configuration tests..."
	@make run-test TEST=relayer_pause_unpause
	@make run-test TEST=relayer_gas_configuration
	@make run-test TEST=relayer_allowlist_toggle

test-edge-cases: ## Run all API edge cases and comprehensive tests
	@echo "Running all API edge cases..."
	@make run-test TEST=transaction_nonce_management
	@make run-test TEST=gas_price_bumping
	@make run-test TEST=concurrent_transactions
	@make run-test TEST=rate_limiting_enforcement
	@make run-test TEST=webhook_delivery_testing
	@make run-test TEST=blob_transaction_handling
	@make run-test TEST=balance_edge_cases

test-comprehensive: ## Run all comprehensive tests (status + allowlist + config + edge cases)
	@echo "Running comprehensive test suite..."
	@make test-status-states
	@make test-allowlist
	@make test-config
	@make test-edge-cases

# =============================================================================
# INDIVIDUAL TEST RUNNER - HELPER TARGET
# =============================================================================

run-test: start-postgres ## Internal target to run a single test with clean output (use TEST=test_name)
	@if [ -z "$(TEST)" ]; then \
		echo "❌ Error: TEST variable must be set. Usage: make run-test TEST=test_name"; \
		exit 1; \
	fi
	@echo "🧪 Running single test: $(TEST) (clean output)..."
	@make reset-db
	@RUST_LOG=error, RRELAYER_TEST_FILTER=$(TEST) cargo run --bin e2e-runner
	@make stop-postgres

run-test-debug: start-postgres ## Internal target to run a single test with debug output (use TEST=test_name)
	@if [ -z "$(TEST)" ]; then \
		echo "❌ Error: TEST variable must be set. Usage: make run-test-debug TEST=test_name"; \
		exit 1; \
	fi
	@echo "🧪 Running single test: $(TEST) (debug output)..."
	@make reset-db
	@RUST_LOG=info, RRELAYER_TEST_FILTER=$(TEST) cargo run --bin e2e-runner
	@make stop-postgres

# =============================================================================
# CONVENIENCE TARGETS FOR COMMON INDIVIDUAL TESTS
# =============================================================================

test-basic: ## Run basic relayer creation test (clean output)
	@make run-test TEST=basic_relayer_creation

test-basic-debug: ## Run basic relayer creation test (debug output)
	@make run-test-debug TEST=basic_relayer_creation

test-transfer: ## Run simple ETH transfer test (clean output)
	@make run-test TEST=simple_eth_transfer

test-transfer-debug: ## Run simple ETH transfer test (debug output)
	@make run-test-debug TEST=simple_eth_transfer

test-contract: ## Run smart contract interaction test (clean output)
	@make run-test TEST=contract_interaction

test-contract-debug: ## Run smart contract interaction test (debug output)
	@make run-test-debug TEST=contract_interaction

test-failed: ## Run failed transaction tests (clean output)
	@make run-test TEST=failed_transaction_handling_not_enough_funds
	@make run-test TEST=failed_transaction_handling_revert_execution

test-failed-debug: ## Run failed transaction tests (debug output)
	@make run-test-debug TEST=failed_transaction_handling_not_enough_funds
	@make run-test-debug TEST=failed_transaction_handling_revert_execution

# =============================================================================
# DEBUG AND DEVELOPMENT TARGETS
# =============================================================================

logs: ## Show recent logs from all services
	@echo "=== Recent Anvil Logs ==="
	@tail -n 20 anvil.log 2>/dev/null || echo "No anvil.log found"
	@echo ""
	@echo "=== Recent RRelayer Logs ==="
	@tail -n 20 rrelayer.log 2>/dev/null || echo "No rrelayer.log found"

logs-live: ## Follow live logs from all services
	@echo "Following live logs (Ctrl+C to stop)..."
	@echo "=== Live RRelayer Logs ==="
	@tail -f rrelayer.log 2>/dev/null || echo "No rrelayer.log found"

logs-rrelayer: ## Show recent RRelayer logs only
	@tail -n 50 rrelayer.log 2>/dev/null || echo "No rrelayer.log found"

logs-anvil: ## Show recent Anvil logs only  
	@tail -n 50 anvil.log 2>/dev/null || echo "No anvil.log found"

logs-clear: ## Clear all log files
	@rm -f *.log
	@echo "All log files cleared"

check-services: ## Check if all services are running
	@echo "Checking service status..."
	@if [ -f anvil.pid ] && kill -0 $$(cat anvil.pid) 2>/dev/null; then \
		echo "✅ Anvil is running (PID: $$(cat anvil.pid))"; \
	else \
		echo "❌ Anvil is not running"; \
	fi
	@if [ -f rrelayer.pid ] && kill -0 $$(cat rrelayer.pid) 2>/dev/null; then \
		echo "✅ RRelayer is running (PID: $$(cat rrelayer.pid))"; \
	else \
		echo "❌ RRelayer is not running"; \
	fi
	@if PGPASSWORD=rrelayer psql -h localhost -p 5447 -U postgres -d postgres -c "SELECT 1;" >/dev/null 2>&1; then \
		echo "✅ PostgreSQL is running"; \
	else \
		echo "❌ PostgreSQL is not running"; \
	fi