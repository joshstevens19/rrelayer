import { Client, createClient, createRelayerClient } from '../clients/core';
import { AdminRelayerClient } from '../clients/admin';
import { RelayerClient } from '../clients/relayer';
import { TEST_CONFIG, skipE2E } from './setup';

describe('Client E2E Tests', () => {
  let client: Client;

  beforeAll(() => {
    if (skipE2E) return;

    client = createClient({
      serverUrl: TEST_CONFIG.SERVER_URL,
      auth: {
        username: TEST_CONFIG.USERNAME,
        password: TEST_CONFIG.PASSWORD,
      },
    });
  });

  describe('Relayer Management', () => {
    test('should create a new relayer', async () => {
      if (skipE2E) return;

      const chainId = parseInt(TEST_CONFIG.CHAIN_ID);
      const name = `test-relayer-${Date.now()}`;

      const result = await client.relayer.create(chainId, name);
      expect(result).toHaveProperty('id');
      expect(result).toHaveProperty('address');
      expect(typeof result.id).toBe('string');
      expect(typeof result.address).toBe('string');
    });

    test('should get all relayers', async () => {
      if (skipE2E) return;

      const relayers = await client.relayer.getAll();
      expect(relayers).toHaveProperty('data');
      expect(relayers).toHaveProperty('pagination');
      expect(Array.isArray(relayers.items)).toBe(true);
    });

    test('should get relayers with pagination', async () => {
      if (skipE2E) return;

      const relayers = await client.relayer.getAll({ offset: 0, limit: 5 });
      expect(relayers).toHaveProperty('items');
      expect(Array.isArray(relayers.items)).toBe(true);
      expect(relayers.items.length).toBeLessThanOrEqual(5);
    });

    test('should filter relayers by chain ID', async () => {
      if (skipE2E) return;

      const chainId = parseInt(TEST_CONFIG.CHAIN_ID);
      const relayers = await client.relayer.getAll(
        { offset: 0, limit: 10 },
        chainId
      );
      expect(relayers).toHaveProperty('items');
      expect(Array.isArray(relayers.items)).toBe(true);

      // All relayers should be for the specified chain
      relayers.items.forEach((relayer: any) => {
        expect(relayer.chainId).toBe(chainId);
      });
    });

    test('should get specific relayer', async () => {
      if (skipE2E) return;

      // Create a test relayer first to get
      const testRelayer = await client.relayer.create(
        parseInt(TEST_CONFIG.CHAIN_ID),
        `get-test-${Date.now()}`
      );
      const relayer = await client.relayer.get(testRelayer.id);
      if (relayer) {
        expect(relayer).toHaveProperty('relayer');
        expect(relayer.relayer).toHaveProperty('address');
        expect(relayer.relayer).toHaveProperty('chainId');
        expect(typeof relayer.relayer.address).toBe('string');
        expect(relayer.relayer.address).toMatch(/^0x[a-fA-F0-9]{40}$/);
      }
    });

    test('should handle non-existent relayer', async () => {
      if (skipE2E) return;

      const relayer = await client.relayer.get('non-existent-relayer-id');
      expect(relayer).toBeNull();
    });

    test('should delete a relayer', async () => {
      if (skipE2E) return;

      // First create a relayer to delete
      const chainId = parseInt(TEST_CONFIG.CHAIN_ID);
      const name = `delete-test-relayer-${Date.now()}`;
      const created = await client.relayer.create(chainId, name);

      // Then delete it
      await expect(client.relayer.delete(created.id)).resolves.not.toThrow();

      // Verify it's deleted
      const deleted = await client.relayer.get(created.id);
      expect(deleted).toBeNull();
    });
  });

  describe('Network Operations', () => {
    test('should get all networks', async () => {
      if (skipE2E) return;

      const networks = await client.network.getAll();
      expect(Array.isArray(networks)).toBe(true);

      if (networks.length > 0) {
        const network = networks[0];
        expect(network).toHaveProperty('chainId');
        expect(network).toHaveProperty('name');
        expect(typeof network.chainId).toBe('number');
        expect(typeof network.name).toBe('string');
      }
    });

    test('should get gas prices for a chain', async () => {
      if (skipE2E) return;

      const chainId = parseInt(TEST_CONFIG.CHAIN_ID);
      const gasEstimate = await client.network.getGasPrices(chainId);

      if (gasEstimate) {
        // Check if it has legacy gas price or EIP-1559 fields
        if ('gasPrice' in gasEstimate) {
          expect(typeof gasEstimate.gasPrice).toBe('string');
        }
        if ('maxFeePerGas' in gasEstimate) {
          expect(typeof gasEstimate.maxFeePerGas).toBe('string');
        }
        if ('maxPriorityFeePerGas' in gasEstimate) {
          expect(typeof gasEstimate.maxPriorityFeePerGas).toBe('string');
        }
      }
    });

    test('should handle invalid chain ID for gas prices', async () => {
      if (skipE2E) return;

      const gasEstimate = await client.network.getGasPrices(999999);
      expect(gasEstimate).toBeNull();
    });
  });

  describe('Transaction Operations', () => {
    test('should get transaction by ID', async () => {
      if (skipE2E) return;

      const transaction = await client.transaction.get('non-existent-tx-id');
      expect(transaction).toBeNull();
    });

    test('should get transaction status', async () => {
      if (skipE2E) return;

      const status = await client.transaction.getStatus('non-existent-tx-id');
      expect(status).toBeNull();
    });
  });

  describe('Admin Relayer Client Creation', () => {
    test('should create admin relayer client', async () => {
      if (skipE2E) return;

      // Create a test relayer first
      const testRelayer = await client.relayer.create(
        parseInt(TEST_CONFIG.CHAIN_ID),
        `admin-creation-test-${Date.now()}`
      );

      const adminClient = await client.getRelayerClient(testRelayer.id);
      expect(adminClient).toBeInstanceOf(AdminRelayerClient);
      expect(adminClient.id).toBe(testRelayer.id);

      // Test that the admin client can perform basic operations
      const info = await adminClient.getInfo();
      expect(info).toHaveProperty('address');

      // Clean up
      await client.relayer.delete(testRelayer.id);
    });

    test('should throw error for non-existent relayer when creating admin client', async () => {
      if (skipE2E) return;

      await expect(
        client.getRelayerClient('non-existent-relayer-id')
      ).rejects.toThrow('Relayer non-existent-relayer-id not found');
    });
  });

  describe('Error Handling', () => {
    test('should handle network errors', async () => {
      if (skipE2E) return;

      const invalidClient = createClient({
        serverUrl: 'http://invalid-server-url:9999',
        auth: {
          username: TEST_CONFIG.USERNAME,
          password: TEST_CONFIG.PASSWORD,
        },
      });

      await expect(invalidClient.relayer.getAll()).rejects.toThrow();
    });

    test('should handle authentication errors', async () => {
      if (skipE2E) return;

      const unauthorizedClient = createClient({
        serverUrl: TEST_CONFIG.SERVER_URL,
        auth: {
          username: 'invalid-username',
          password: 'invalid-password',
        },
      });

      await expect(unauthorizedClient.relayer.getAll()).rejects.toThrow();
    });
  });

  describe('Factory Functions', () => {
    test('createClient should return Client instance', () => {
      const testClient = createClient({
        serverUrl: TEST_CONFIG.SERVER_URL,
        auth: {
          username: TEST_CONFIG.USERNAME,
          password: TEST_CONFIG.PASSWORD,
        },
      });

      expect(testClient).toBeInstanceOf(Client);
    });

    test('createRelayerClient should return RelayerClient instance', async () => {
      // Create a test relayer first
      const testRelayer = await client.relayer.create(
        parseInt(TEST_CONFIG.CHAIN_ID),
        `factory-test-${Date.now()}`
      );

      const relayerClient = await createRelayerClient({
        serverUrl: TEST_CONFIG.SERVER_URL,
        providerUrl: TEST_CONFIG.PROVIDER_URL,
        relayerId: testRelayer.id,
        apiKey: 'test-api-key', // Use a test API key for now
      });

      expect(relayerClient).toBeInstanceOf(RelayerClient);
      expect(relayerClient.id).toBe(testRelayer.id);

      // Clean up
      await client.relayer.delete(testRelayer.id);
    });
  });

  describe('Client Integration', () => {
    test('should perform complete workflow: create relayer -> get info -> delete', async () => {
      if (skipE2E) return;

      // Create relayer
      const chainId = parseInt(TEST_CONFIG.CHAIN_ID);
      const name = `workflow-test-${Date.now()}`;
      const created = await client.relayer.create(chainId, name);

      expect(created).toHaveProperty('id');
      expect(created).toHaveProperty('address');

      // Get relayer info
      const relayer = await client.relayer.get(created.id);
      expect(relayer).not.toBeNull();
      expect(relayer!.relayer.chainId).toBe(chainId);

      // Create admin client for the relayer
      const adminClient = await client.getRelayerClient(created.id);
      expect(adminClient).toBeInstanceOf(AdminRelayerClient);

      // Test admin operations
      const info = await adminClient.getInfo();
      expect(info).toHaveProperty('address');
      expect(info.chainId).toBe(chainId);

      // Delete relayer
      await client.relayer.delete(created.id);

      // Verify deletion
      const deleted = await client.relayer.get(created.id);
      expect(deleted).toBeNull();
    });
  });
});
