# =============================================================================
# ANVIL BLOCKCHAIN
# =============================================================================

start-anvil:
	@echo "Resetting database schema..."
	@$(MAKE) reset-database
	@echo "Starting Anvil..."
	@anvil --host 0.0.0.0 --port 8545 --accounts 10 \
	   --mnemonic "test test test test test test test test test test test junk" \
	   --block-time 1 --gas-limit 30000000 --hardfork cancun & echo $$! > anvil.pid
	@sleep 2
	@echo "Anvil started (PID: $$(cat anvil.pid))"
	@$(MAKE) generate-gas-activity
	@echo "Anvil ready with realistic gas price history!"

stop-anvil:
	@if [ -f anvil.pid ]; then \
	   echo "Stopping Anvil (PID: $$(cat anvil.pid))..."; \
	   kill $$(cat anvil.pid) 2>/dev/null || true; \
	   rm -f anvil.pid; \
	   echo "Anvil stopped"; \
	else \
	   echo "No anvil.pid file found"; \
	fi

kill-anvil:
	@echo "Force killing any process on port 8545..."
	@lsof -ti:8545 | xargs kill -9 2>/dev/null || echo "No process found on port 8545"
	@rm -f anvil.pid

generate-gas-activity:
	@echo "Generating gas price activity with 50 transactions..."
	@for i in $$(seq 1 100); do \
		gas_multiplier=$$((1 + ($$i % 5))); \
		gas_price_hex=$$(printf "0x%x" $$((1000000000 * $$gas_multiplier))); \
		priority_fee_hex=$$(printf "0x%x" $$((500000000))); \
		nonce_hex=$$(printf "0x%x" $$(($$i - 1))); \
		echo "Sending transaction $$i/100 (Gas: $$(printf "%d" $$((1000000000 * $$gas_multiplier))) wei)..."; \
		curl -s -X POST http://127.0.0.1:8545 \
			-H "Content-Type: application/json" \
			-d "{\"jsonrpc\":\"2.0\",\"method\":\"eth_sendTransaction\",\"params\":[{\"from\":\"0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266\",\"to\":\"0x70997970C51812dc3A010C7d01b50e0d17dc79C8\",\"value\":\"0x1000000000000000\",\"maxFeePerGas\":\"$$gas_price_hex\",\"maxPriorityFeePerGas\":\"$$priority_fee_hex\",\"gas\":\"0x5208\",\"nonce\":\"$$nonce_hex\"}],\"id\":$$i}" \
			>/dev/null || echo "Transaction $$i failed"; \
		sleep 0.1; \
	done
	@echo "Generated 100 transactions with varying gas prices (1-5 gwei)"

# =============================================================================
# DATABASE MANAGEMENT
# =============================================================================

reset-database:
	@echo "Dropping all schemas and recreating database..."
	@PGPASSWORD=rrelayer psql -h localhost -p 5471 -U postgres -d postgres -c "SELECT 'DROP SCHEMA IF EXISTS \"' || schema_name || '\" CASCADE;' FROM information_schema.schemata WHERE schema_name NOT IN ('information_schema', 'pg_catalog', 'pg_toast');" | grep DROP | PGPASSWORD=rrelayer psql -h localhost -p 5471 -U postgres -d postgres
	@PGPASSWORD=rrelayer psql -h localhost -p 5471 -U postgres -d postgres -c "CREATE SCHEMA public;"
	@PGPASSWORD=rrelayer psql -h localhost -p 5471 -U postgres -d postgres -c "GRANT ALL ON SCHEMA public TO postgres;"
	@PGPASSWORD=rrelayer psql -h localhost -p 5471 -U postgres -d postgres -c "GRANT ALL ON SCHEMA public TO public;"
	@echo "Database completely reset - all schemas dropped and public schema recreated"