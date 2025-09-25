import { RelayerClient } from '../clients/relayer';
import { Client, createClient } from '../clients/core';
import { TEST_CONFIG, skipE2E, createTestRelayer } from './setup';

describe('RelayerClient E2E Tests', () => {
  let adminClient: Client;
  let client: RelayerClient;
  let testRelayer: any;

  beforeAll(async () => {
    if (skipE2E) return;
    
    // Create admin client to create relayers
    adminClient = createClient({
      serverUrl: TEST_CONFIG.SERVER_URL,
      auth: {
        username: TEST_CONFIG.USERNAME,
        password: TEST_CONFIG.PASSWORD,
      },
    });

    // Create a test relayer
    testRelayer = await createTestRelayer(adminClient, 'relayer-client-test');
    
    // Create relayer client with the new relayer
    client = new RelayerClient({
      serverUrl: TEST_CONFIG.SERVER_URL,
      providerUrl: TEST_CONFIG.PROVIDER_URL,
      relayerId: testRelayer.relayerId,
      auth: {
        username: TEST_CONFIG.USERNAME,
        password: TEST_CONFIG.PASSWORD,
      },
    });
  });

  afterAll(async () => {
    if (skipE2E || !testRelayer) return;
    
    // Clean up the test relayer
    try {
      await adminClient.relayer.delete(testRelayer.relayerId);
    } catch (error) {
      console.warn('Failed to cleanup test relayer:', error);
    }
  });

  describe('Basic Relayer Operations', () => {
    test('should get relayer address', async () => {
      if (skipE2E) return;
      
      const address = await client.address();
      expect(typeof address).toBe('string');
      expect(address).toMatch(/^0x[a-fA-F0-9]{40}$/);
    });

    test('should get relayer information', async () => {
      if (skipE2E) return;
      
      const info = await client.getInfo();
      expect(info).toHaveProperty('address');
      expect(info).toHaveProperty('chainId');
      expect(typeof info.address).toBe('string');
    });

    test('should get relayer balance', async () => {
      if (skipE2E) return;
      
      const balance = await client.getBalanceOf();
      expect(typeof balance).toBe('string');
      expect(parseFloat(balance)).toBeGreaterThanOrEqual(0);
    });

    test('should get ethereum provider', () => {
      if (skipE2E) return;
      
      const provider = client.ethereumProvider();
      expect(provider).toBeDefined();
    });
  });

  describe('Allowlist Operations', () => {
    test('should get allowlist addresses', async () => {
      if (skipE2E) return;
      
      const allowlist = await client.allowlist.get();
      expect(allowlist).toHaveProperty('data');
      expect(Array.isArray(allowlist.items)).toBe(true);
    });

    test('should get allowlist addresses with pagination', async () => {
      if (skipE2E) return;
      
      const allowlist = await client.allowlist.get({ offset: 1, limit: 10 });
      expect(allowlist).toHaveProperty('data');
      expect(allowlist).toHaveProperty('pagination');
      expect(allowlist.items.length).toBeLessThanOrEqual(10);
    });
  });

  describe('Transaction Operations', () => {
    test('should get all transactions', async () => {
      if (skipE2E) return;
      
      const transactions = await client.transaction.getAll();
      expect(transactions).toHaveProperty('data');
      expect(Array.isArray(transactions.items)).toBe(true);
    });

    test('should get all transactions with pagination', async () => {
      if (skipE2E) return;
      
      const transactions = await client.transaction.getAll({ offset: 1, limit: 5 });
      expect(transactions).toHaveProperty('data');
      expect(transactions).toHaveProperty('pagination');
      expect(transactions.items.length).toBeLessThanOrEqual(5);
    });

    test('should handle getting non-existent transaction', async () => {
      if (skipE2E) return;
      
      const transaction = await client.transaction.get('non-existent-tx-id');
      expect(transaction).toBeNull();
    });

    test('should handle getting status of non-existent transaction', async () => {
      if (skipE2E) return;
      
      const status = await client.transaction.getStatus('non-existent-tx-id');
      expect(status).toBeNull();
    });

    // Note: Testing actual transaction sending would require a valid transaction and ETH balance
    // This would be better suited for integration tests with a test network
  });

  describe('Signing Operations', () => {
    test('should sign text message', async () => {
      if (skipE2E) return;
      
      const message = 'Hello, World!';
      const result = await client.sign.text(message);
      expect(result).toHaveProperty('signature');
      expect(typeof result.signature).toBe('string');
      expect(result.signature).toMatch(/^0x[a-fA-F0-9]{130}$/);
    });

    test('should sign text message with rate limit key', async () => {
      if (skipE2E) return;
      
      const message = 'Hello with rate limit!';
      const rateLimitKey = 'test-rate-limit-key';
      const result = await client.sign.text(message, rateLimitKey);
      expect(result).toHaveProperty('signature');
      expect(typeof result.signature).toBe('string');
    });

    test('should sign typed data', async () => {
      if (skipE2E) return;
      
      const typedData = {
        domain: {
          name: 'Test App',
          version: '1',
          chainId: parseInt(TEST_CONFIG.CHAIN_ID),
        },
        types: {
          Person: [
            { name: 'name', type: 'string' },
            { name: 'wallet', type: 'address' },
          ],
        },
        primaryType: 'Person',
        message: {
          name: 'Alice',
          wallet: '0x1234567890123456789012345678901234567890',
        },
      };

      const result = await client.sign.typedData(typedData);
      expect(result).toHaveProperty('signature');
      expect(typeof result.signature).toBe('string');
      expect(result.signature).toMatch(/^0x[a-fA-F0-9]{130}$/);
    });
  });

  describe('Error Handling', () => {
    test('should handle network errors gracefully', async () => {
      if (skipE2E) return;
      
      // Create client with invalid server URL
      const invalidClient = new RelayerClient({
        serverUrl: 'http://invalid-server-url:9999',
        providerUrl: TEST_CONFIG.PROVIDER_URL,
        relayerId: 'some-relayer-id',
        auth: {
          username: TEST_CONFIG.USERNAME,
          password: TEST_CONFIG.PASSWORD,
        },
      });

      await expect(invalidClient.getInfo()).rejects.toThrow();
    });

    test('should handle invalid relayer ID', async () => {
      if (skipE2E) return;
      
      const invalidClient = new RelayerClient({
        serverUrl: TEST_CONFIG.SERVER_URL,
        providerUrl: TEST_CONFIG.PROVIDER_URL,
        relayerId: 'invalid-relayer-id',
        auth: {
          username: TEST_CONFIG.USERNAME,
          password: TEST_CONFIG.PASSWORD,
        },
      });

      await expect(invalidClient.getInfo()).rejects.toThrow();
    });
  });
});