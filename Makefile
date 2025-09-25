# =============================================================================
# ANVIL BLOCKCHAIN
# =============================================================================

start-anvil: ## Start Anvil blockchain
	@echo "Starting Anvil..."
	@anvil --host 0.0.0.0 --port 8545 --accounts 10 \
	   --mnemonic "test test test test test test test test test test test junk" \
	   --block-time 1 --gas-limit 30000000 & echo $$! > anvil.pid
	@sleep 2
	@echo "Anvil started (PID: $$(cat anvil.pid))"

stop-anvil: ## Stop Anvil blockchain
	@if [ -f anvil.pid ]; then \
	   echo "Stopping Anvil (PID: $$(cat anvil.pid))..."; \
	   kill $$(cat anvil.pid) 2>/dev/null || true; \
	   rm -f anvil.pid; \
	   echo "Anvil stopped"; \
	else \
	   echo "No anvil.pid file found"; \
	fi

kill-anvil: ## Force kill any anvil process on port 8545
	@echo "Force killing any process on port 8545..."
	@lsof -ti:8545 | xargs kill -9 2>/dev/null || echo "No process found on port 8545"
	@rm -f anvil.pid