# CLI

## Design

- relayer init - creates a new relayer project
   - rrelayer project name - required
   - project description - optional
   - docker support out of the box? [yes, no]
 creates you a new project with a rrelayer.yaml file in it + .env file with stuff in

- relayer start - starts the relayer
- relayer stop - stops the relayer

- relayer network add - adds a network to the relayer
   - network name - required
   - provider url - required > add another?
   - gas provider - [infura, tenderly, built in]
- relayer network list [--filter=enabled|disabled] - lists all the networks setup for the relayer
- relayer network <network_name> gas - gets the gas prices for the network
- relayer network <network_name> enable - enables the network
- relayer network <network_name> disable - disables the network

- relayer list [--networks=optional] - lists all the relayers setup for the relayer project
- relayer get <relayer_id> - gets the relayer by id
- relayer pause <relayer_id> - pauses the relayer by id
- relayer unpause <relayer_id> - unpauses the relayer by id
- relayer update_eip1559_status <relayer_id> <status> - updates the EIP1559 status for the relayer by id
- relayer update_max_gas_price <relayer_id> <cap> - updates the max gas price for the relayer by id
- 
- relayer api_key add <relayer_id> - adds an API key for the relayer by id
- relayer api_key list <relayer_id> - lists all the API keys for the relayer by id
- relayer api_key delete <relayer_id> <api_key> - deletes an API key for the relayer by id
- 
- relayer allowlist add <relayer_id> <address> - adds an allowlist address for the relayer by id
- relayer allowlist list <relayer_id> - lists all the allowlist addresses for the relayer by id
- relayer allowlist delete <relayer_id> <address> - deletes an allowlist address for the relayer by id

- relayer create - creates a new relayer client
  - relayer name - required
  - network name - required

- relayer sign text <relayer_id> - signs a message 
- relayer sign typed_data <relayer_id> - signs typed data

- relayer tx get <tx_id> - gets a transaction by id
- relayer tx status <tx_id> - gets a transaction status by id
- relayer tx list <relayer_id> - lists all the transactions for the relayer (maybe be able to filter by status)
- relayer tx queue <relayer_id> - lists all the pending and inmempool transactions for the relayer
- relayer tx cancel <tx_id> - cancels a transaction by id
- relayer tx replace <tx_id> - replaces a transaction by id
- relayer tx send <relayer_id> - sends a transaction

- relayer user list - lists all the users for the relayer
- relayer user edit <user_address> <role> - edits a user for the relayer
- relayer user add <user_address> <role> - adds a user for the relayer
- relayer user delete <user_address> - deletes a user for the relayer



