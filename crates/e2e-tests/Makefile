# RRelayer E2E Tests Makefile

.PHONY: help install-deps start-postgres stop-postgres reset-db start-anvil stop-anvil start-rrelayer stop-rrelayer run-tests clean

help: ## Show this help message
	@echo "RRelayer E2E Tests"
	@echo "Available targets:"
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

install-deps: ## Install required dependencies (Foundry)
	@echo "Installing Foundry..."
	@curl -L https://foundry.paradigm.xyz | bash
	@foundryup

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

start-rrelayer: ## Start RRelayer service
	@echo "Starting RRelayer..."
	@cd ../cli && RUST_LOG=info,rrelayer_core=info RUST_BACKTRACE=1 cargo run -- start --path $(CURDIR) > $(CURDIR)/rrelayer.log 2>&1 &
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

run-tests: ## Run E2E tests
	@echo "Running E2E tests..."
	@make reset-db
	@RUST_LOG=info,rrelayer_core=info cargo run --bin e2e-runner

test-full: start-postgres start-anvil start-rrelayer run-tests stop-rrelayer stop-anvil stop-postgres ## Run complete test suite

test-quick: ## Run tests assuming services are already running
	@echo "Running E2E tests (assuming services are running)..."
	@RUST_LOG=info,rrelayer_core=info cargo run --bin e2e-runner

build: ## Build the E2E test binary
	@cargo build --bin e2e-runner

# Individual Test Scenarios
test-basic-relayer: ## Run basic relayer creation test
	@echo "Running basic relayer creation test..."
	@make reset-db
	@RUST_LOG=info,rrelayer_core=info RRELAYER_TEST_FILTER=basic_relayer_creation cargo run --bin e2e-runner

test-gas-price: ## Run gas price API test
	@echo "Running gas price API test..."
	@make reset-db
	@RUST_LOG=info,rrelayer_core=info RRELAYER_TEST_FILTER=gas_price_api cargo run --bin e2e-runner

test-network-mgmt: ## Run network management test
	@echo "Running network management test..."
	@make reset-db
	@RUST_LOG=info,rrelayer_core=info RRELAYER_TEST_FILTER=network_management cargo run --bin e2e-runner

test-allowlist-mgmt: ## Run allowlist management test
	@echo "Running allowlist management test..."
	@make reset-db
	@RUST_LOG=info,rrelayer_core=info RRELAYER_TEST_FILTER=allowlist_management cargo run --bin e2e-runner

test-signing-text: ## Run text signing test
	@echo "Running text signing test..."
	@make reset-db
	@RUST_LOG=info,rrelayer_core=info RRELAYER_TEST_FILTER=signing_text cargo run --bin e2e-runner

test-signing-typed: ## Run typed data signing test
	@echo "Running typed data signing test..."
	@make reset-db
	@RUST_LOG=info,rrelayer_core=info RRELAYER_TEST_FILTER=signing_typed_data cargo run --bin e2e-runner

test-transaction-ops: ## Run transaction operations test
	@echo "Running transaction operations test..."
	@make reset-db
	@RUST_LOG=info,rrelayer_core=info RRELAYER_TEST_FILTER=transaction_operations cargo run --bin e2e-runner

test-transaction-status: ## Run transaction status test
	@echo "Running transaction status test..."
	@make reset-db
	@RUST_LOG=info,rrelayer_core=info RRELAYER_TEST_FILTER=transaction_status_operations cargo run --bin e2e-runner

test-transaction-counts: ## Run transaction counts test
	@echo "Running transaction counts test..."
	@make reset-db
	@RUST_LOG=info,rrelayer_core=info RRELAYER_TEST_FILTER=transaction_counts cargo run --bin e2e-runner

# Missing test targets
test-simple-eth-transfer: ## Run simple ETH transfer test
	@echo "Running simple ETH transfer test..."
	@make reset-db
	RUST_LOG=info,rrelayer_core=info RRELAYER_TEST_FILTER=simple_eth_transfer cargo run --bin e2e-runner

test-contract-interaction: ## Run contract interaction test
	@echo "Running contract interaction test..."
	@make reset-db
	@RUST_LOG=info,rrelayer_core=info RRELAYER_TEST_FILTER=contract_interaction cargo run --bin e2e-runner

test-transaction-status-tracking: ## Run transaction status tracking test
	@echo "Running transaction status tracking test..."
	@make reset-db
	@RUST_LOG=info,rrelayer_core=info RRELAYER_TEST_FILTER=transaction_status_tracking cargo run --bin e2e-runner

test-failed-transaction-handling-not-enough-funds:
	@echo "Running failed transaction handling test (insufficient funds)..."
	@make reset-db
	@RUST_LOG=info,rrelayer_core=info RRELAYER_TEST_FILTER=failed_transaction_handling_not_enough_funds cargo run --bin e2e-runner

test-failed-transaction-handling-revert-execution:
	@echo "Running failed transaction handling test (contract revert)..."
	@make reset-db
	@RUST_LOG=info,rrelayer_core=info RRELAYER_TEST_FILTER=failed_transaction_handling_revert_execution cargo run --bin e2e-runner

test-failed-transaction-handling:
	@echo "Running all failed transaction handling tests..."
	@make test-failed-transaction-handling-not-enough-funds
	@make test-failed-transaction-handling-revert-execution

test-gas-estimation: ## Run gas estimation test
	@echo "Running gas estimation test..."
	@make reset-db
	@RUST_LOG=info,rrelayer_core=info RRELAYER_TEST_FILTER=gas_estimation cargo run --bin e2e-runner

test-transaction-replacement: ## Run transaction replacement test
	@echo "Running transaction replacement test..."
	@make reset-db
	@RUST_LOG=info,rrelayer_core=info RRELAYER_TEST_FILTER=transaction_replacement cargo run --bin e2e-runner

test-batch-transactions: ## Run batch transactions test
	@echo "Running batch transactions test..."
	@make reset-db
	@RUST_LOG=info,rrelayer_core=info RRELAYER_TEST_FILTER=batch_transactions cargo run --bin e2e-runner

test-relayer-limits: ## Run relayer limits test
	@echo "Running relayer limits test..."
	@make reset-db
	@RUST_LOG=info,rrelayer_core=info RRELAYER_TEST_FILTER=relayer_limits cargo run --bin e2e-runner

clean: stop-postgres stop-anvil stop-rrelayer ## Clean up all running processes and files
	@rm -f *.pid *.log
	@cargo clean
	@echo "Cleanup complete"

status: ## Show status of services
	@echo "=== Service Status ==="
	@if PGPASSWORD=rrelayer psql -h localhost -p 5447 -U postgres -d postgres -c "SELECT 1;" >/dev/null 2>&1; then \
		echo "✅ PostgreSQL running"; \
	else \
		echo "❌ PostgreSQL not running"; \
	fi
	@if [ -f anvil.pid ] && kill -0 $$(cat anvil.pid) 2>/dev/null; then \
		echo "✅ Anvil running (PID: $$(cat anvil.pid))"; \
	else \
		echo "❌ Anvil not running"; \
	fi
	@if [ -f rrelayer.pid ] && kill -0 $$(cat rrelayer.pid) 2>/dev/null; then \
		echo "✅ RRelayer running (PID: $$(cat rrelayer.pid))"; \
	else \
		echo "❌ RRelayer not running"; \
	fi
	@if curl -s -f http://localhost:3000/health >/dev/null 2>&1; then \
		echo "✅ RRelayer API responding"; \
	else \
		echo "❌ RRelayer API not responding"; \
	fi

logs: ## Show logs from services
	@echo "=== Anvil Logs ==="
	@if [ -f anvil.log ]; then tail -n 20 anvil.log; else echo "No anvil logs"; fi
	@echo ""
	@echo "=== RRelayer Logs ==="
	@if [ -f rrelayer.log ]; then tail -n 20 rrelayer.log; else echo "No rrelayer logs"; fi

logs-follow: ## Follow logs from services in real-time
	@echo "Following RRelayer logs (Ctrl+C to exit)..."
	@if [ -f rrelayer.log ]; then tail -f rrelayer.log; else echo "No rrelayer logs"; fi

test-with-logs: ## Run tests and show both e2e and rrelayer logs
	@echo "Running E2E tests with combined logging..."
	@make reset-db
	@(tail -f rrelayer.log 2>/dev/null | sed 's/^/[RRELAYER] /' &) ; \
	RUST_LOG=info,rrelayer_core=info cargo run --bin e2e-runner

test-embedded: ## Run E2E tests with embedded RRelayer server (unified logs)
	@echo "Running E2E tests with embedded RRelayer server..."
	@make reset-db
	@RUST_LOG=info,rrelayer_core=info cargo run --bin e2e-runner

test-full-embedded: start-postgres start-anvil test-embedded stop-anvil stop-postgres ## Run complete test suite with embedded RRelayer

test-quick-embedded: ## Run embedded tests assuming Anvil and PostgreSQL are running
	@echo "Running E2E tests with embedded RRelayer (assuming services are running)..."
	@RUST_LOG=info,rrelayer_core=info cargo run --bin e2e-runner

# Development targets
dev-setup: install-deps build ## Setup development environment

dev-start: start-postgres start-anvil start-rrelayer ## Start all services for development

dev-test: ## Run tests in development mode with detailed logging
	@RUST_LOG=info,rrelayer_core=info cargo run --bin e2e-runner

# CI targets  
ci-test: ## Run tests in CI mode
	@echo "Running CI E2E tests..."
	@timeout 300 make test-full || (make clean && exit 1)