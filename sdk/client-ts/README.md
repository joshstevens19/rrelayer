# RRelayer SDK

The official SDK for interacting with RRelayer services.

## Installation

```bash
npm install rrelayer-sdk
```

## Usage

### Basic Client (Admin Operations)

```typescript
import { createClient } from 'rrelayer-sdk';

const client = createClient({
  serverUrl: 'https://your-rrelayer-server.com',
  auth: {
    username: 'your-username',
    password: 'your-password',
  },
});

// Create a new relayer
const relayer = await client.relayer.create(1, 'My Relayer');

// Get all relayers
const relayers = await client.relayer.getAll();

// Get networks
const networks = await client.network.getAll();
```

### Relayer Client (Relayer-specific Operations)

```typescript
import { RelayerClient } from 'rrelayer-sdk';

const relayerClient = new RelayerClient({
  serverUrl: 'https://your-rrelayer-server.com',
  providerUrl: 'https://your-ethereum-provider.com',
  relayerId: 'your-relayer-id',
  auth: {
    username: 'your-username',
    password: 'your-password',
  },
});

// Get relayer address
const address = await relayerClient.address();

// Send a transaction
const tx = await relayerClient.transaction.send({
  to: '0x...',
  data: '0x...',
  value: '0',
});

// Sign a message
const signature = await relayerClient.sign.text('Hello, World!');
```

### Admin Relayer Client (Admin + Relayer Operations)

```typescript
import { AdminRelayerClient } from 'rrelayer-sdk';

const adminClient = new AdminRelayerClient({
  serverUrl: 'https://your-rrelayer-server.com',
  providerUrl: 'https://your-ethereum-provider.com',
  relayerId: 'your-relayer-id',
  auth: {
    username: 'your-admin-username',
    password: 'your-admin-password',
  },
});

// Pause/unpause relayer
await adminClient.pause();
await adminClient.unpause();

// Update gas settings
await adminClient.updateMaxGasPrice('1000000000');
await adminClient.updateEIP1559Status(true);

// Get transaction counts
const pendingCount = await adminClient.transaction.getCount('PENDING');
```

## Testing

### Running Tests

The SDK includes comprehensive E2E tests. To run them:

1. Copy the environment file and configure your test server:
   ```bash
   cp .env.example .env
   # Edit .env with your test server details
   ```

2. Run all tests:
   ```bash
   npm test
   ```

3. Run specific test suites:
   ```bash
   npm run test:client      # Core client tests
   npm run test:relayer     # Relayer client tests
   npm run test:admin       # Admin client tests
   ```

4. Run tests in watch mode:
   ```bash
   npm run test:watch
   ```

5. Generate coverage report:
   ```bash
   npm run test:coverage
   ```

### Test Environment Variables

- `TEST_SERVER_URL`: Your RRelayer server URL
- `TEST_PROVIDER_URL`: Ethereum provider URL
- `TEST_USERNAME`: Test account username
- `TEST_PASSWORD`: Test account password
- `TEST_CHAIN_ID`: Chain ID to test with (default: 1)

### Test Behavior

- Each test creates its own relayer to ensure isolation
- Tests clean up created relayers automatically
- Tests are skipped if `TEST_SERVER_URL` is not provided
- All tests use basic authentication (username/password)

## Development

### Build

```bash
npm run build
```

### Format Code

```bash
npm run format
```

### Check Formatting

```bash
npm run format:check
```

## API Reference

### Client

- `relayer.create(chainId, name)` - Create a new relayer
- `relayer.getAll(pagination?, chainId?)` - Get all relayers
- `relayer.get(relayerId)` - Get specific relayer
- `relayer.delete(relayerId)` - Delete a relayer
- `network.getAll()` - Get all supported networks
- `network.getGasPrices(chainId)` - Get gas prices for a chain
- `transaction.get(txId)` - Get transaction by ID
- `transaction.getStatus(txId)` - Get transaction status
- `getRelayerClient(relayerId)` - Create admin relayer client

### RelayerClient

- `address()` - Get relayer address
- `getInfo()` - Get relayer information
- `getBalanceOf()` - Get relayer balance
- `allowlist.get(pagination?)` - Get allowlist addresses
- `sign.text(message, rateLimitKey?)` - Sign text message
- `sign.typedData(typedData, rateLimitKey?)` - Sign typed data
- `transaction.get(txId)` - Get transaction
- `transaction.getStatus(txId)` - Get transaction status
- `transaction.getAll(pagination?)` - Get all transactions
- `transaction.send(txData, rateLimitKey?)` - Send transaction
- `transaction.replace(txId, newTx, rateLimitKey?)` - Replace transaction
- `transaction.cancel(txId, rateLimitKey?)` - Cancel transaction
- `transaction.waitForTransactionReceiptById(txId)` - Wait for receipt

### AdminRelayerClient

Inherits all RelayerClient methods plus:

- `pause()` - Pause relayer
- `unpause()` - Unpause relayer
- `updateEIP1559Status(enabled)` - Update EIP1559 status
- `updateMaxGasPrice(gasPrice)` - Set max gas price
- `removeMaxGasPrice()` - Remove gas price limit
- `transaction.getCount(type)` - Get transaction counts ('PENDING' | 'INMEMPOOL')