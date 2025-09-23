# RRelayer E2E Tests Makefile

.PHONY: help install-deps clean build \
        start-postgres stop-postgres reset-db \
        start-anvil stop-anvil \
        start-rrelayer start-rrelayer-debug stop-rrelayer \
        test-full test-quick run-tests run-tests-debug run-tests-debug-all \
        run-tests-debug-raw run-tests-debug-aws-secret-manager run-tests-debug-aws-kms \
        run-tests-debug-gcp-secret-manager run-tests-debug-privy run-tests-debug-turnkey \
        test-core test-comprehensive test-status-states test-allowlist test-config test-edge-cases \
        run-test run-test-debug \
        test-basic test-basic-debug test-transfer test-transfer-debug \
        test-contract test-contract-debug test-failed test-failed-debug \
        test-multi-provider test-multi-provider-debug test-specific-providers test-specific-providers-debug \
        test-multi-provider-single test-multi-provider-single-debug \
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

test-quick: ## Run tests assuming services are already running
	@echo "Running E2E tests (assuming services are running)..."
	@RUST_LOG=info,rrelayer_core=info cargo run --bin e2e-runner

run-tests:
	@echo "Running E2E tests with clean output..."
	@make reset-db
	@RUST_LOG=error cargo run --bin e2e-runner

run-tests-debug:
	@echo "Running E2E tests with debug output..."
	@make reset-db
	@RUST_LOG=info cargo run --bin e2e-runner

run-tests-debug-all:
	@echo "Running E2E tests for all providers with debug output..."
	@RRELAYER_MULTI_PROVIDER=1 RUST_LOG=info cargo run --bin e2e-runner

run-tests-debug-raw:
	@echo "Running E2E tests for raw provider with debug output..."
	@RRELAYER_MULTI_PROVIDER=1 RRELAYER_PROVIDERS="raw" RUST_LOG=info cargo run --bin e2e-runner

run-tests-debug-aws-secret-manager:
	@echo "Running E2E tests for aws_secret_manager provider with debug output..."
	@RRELAYER_MULTI_PROVIDER=1 RRELAYER_PROVIDERS="aws_secret_manager" RUST_LOG=info cargo run --bin e2e-runner

run-tests-debug-aws-kms:
	@echo "Running E2E tests for aws_kms provider with debug output..."
	@RRELAYER_MULTI_PROVIDER=1 RRELAYER_PROVIDERS="aws_kms" RUST_LOG=info cargo run --bin e2e-runner

run-tests-debug-gcp-secret-manager:
	@echo "Running E2E tests for gcp_secret_manager provider with debug output..."
	@RRELAYER_MULTI_PROVIDER=1 RRELAYER_PROVIDERS="gcp_secret_manager" RUST_LOG=info cargo run --bin e2e-runner

run-tests-debug-privy:
	@echo "Running E2E tests for privy provider with debug output..."
	@RRELAYER_MULTI_PROVIDER=1 RRELAYER_PROVIDERS="privy" RUST_LOG=info cargo run --bin e2e-runner

run-tests-debug-turnkey:
	@echo "Running E2E tests for turnkey provider with debug output..."
	@RRELAYER_MULTI_PROVIDER=1 RRELAYER_PROVIDERS="turnkey" RUST_LOG=info cargo run --bin e2e-runner

run-tests-debug-raw-and-aws-secret-manager:
	@echo "Running E2E tests for raw provider with debug output..."
	@RRELAYER_MULTI_PROVIDER=1 RRELAYER_PROVIDERS="raw,aws_secret_manager" RUST_LOG=error cargo run --bin e2e-runner

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