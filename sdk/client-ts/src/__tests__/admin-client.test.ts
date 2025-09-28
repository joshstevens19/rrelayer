import { AdminRelayerClient } from '../clients/admin';
import { TransactionCountType } from '../clients/types';
import { Client, createClient } from '../clients/core';
import { TEST_CONFIG, skipE2E, createTestRelayer } from './setup';

describe('AdminRelayerClient E2E Tests', () => {
  let client: Client;
  let adminClient: AdminRelayerClient;
  let testRelayer: any;

  beforeAll(async () => {
    if (skipE2E) return;

    // Create admin client to create relayers
    client = createClient({
      serverUrl: TEST_CONFIG.SERVER_URL,
      auth: {
        username: TEST_CONFIG.USERNAME,
        password: TEST_CONFIG.PASSWORD,
      },
    });

    // Create a test relayer
    testRelayer = await createTestRelayer(client, 'admin-client-test');

    // Create admin relayer client
    adminClient = new AdminRelayerClient({
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
      await client.relayer.delete(testRelayer.relayerId);
    } catch (error) {
      console.warn('Failed to cleanup test relayer:', error);
    }
  });

  describe('Admin Operations', () => {
    test('should pause relayer', async () => {
      if (skipE2E) return;

      await expect(adminClient.pause()).resolves.not.toThrow();
    });

    test('should unpause relayer', async () => {
      if (skipE2E) return;

      await expect(adminClient.unpause()).resolves.not.toThrow();
    });

    test('should update EIP1559 status', async () => {
      if (skipE2E) return;

      await expect(
        adminClient.updateEIP1559Status(true)
      ).resolves.not.toThrow();
      await expect(
        adminClient.updateEIP1559Status(false)
      ).resolves.not.toThrow();
    });

    test('should update max gas price', async () => {
      if (skipE2E) return;

      const maxGasPrice = '1000000000'; // 1 gwei
      await expect(
        adminClient.updateMaxGasPrice(maxGasPrice)
      ).resolves.not.toThrow();
    });

    test('should remove max gas price', async () => {
      if (skipE2E) return;

      await expect(adminClient.removeMaxGasPrice()).resolves.not.toThrow();
    });
  });

  describe('Transaction Count Operations', () => {
    test('should get pending transaction count', async () => {
      if (skipE2E) return;

      const count = await adminClient.transaction.getCount(
        TransactionCountType.PENDING
      );
      expect(typeof count).toBe('number');
      expect(count).toBeGreaterThanOrEqual(0);
    });

    test('should get inmempool transaction count', async () => {
      if (skipE2E) return;

      const count = await adminClient.transaction.getCount(
        TransactionCountType.INMEMPOOL
      );
      expect(typeof count).toBe('number');
      expect(count).toBeGreaterThanOrEqual(0);
    });

    test('should throw error for invalid transaction count type', async () => {
      if (skipE2E) return;

      const invalidType = 'INVALID' as TransactionCountType;
      await expect(
        adminClient.transaction.getCount(invalidType)
      ).rejects.toThrow('Invalid transaction count type');
    });
  });

  describe('Inherited Transaction Operations', () => {
    test('should inherit base transaction methods', async () => {
      if (skipE2E) return;

      // Test that admin client has all the base transaction methods
      expect(adminClient.transaction.get).toBeDefined();
      expect(adminClient.transaction.getStatus).toBeDefined();
      expect(adminClient.transaction.getAll).toBeDefined();
      expect(adminClient.transaction.send).toBeDefined();
      expect(adminClient.transaction.replace).toBeDefined();
      expect(adminClient.transaction.cancel).toBeDefined();
      expect(
        adminClient.transaction.waitForTransactionReceiptById
      ).toBeDefined();

      // Test that it has the admin-specific method
      expect(adminClient.transaction.getCount).toBeDefined();
    });

    test('should get all transactions (inherited)', async () => {
      if (skipE2E) return;

      const transactions = await adminClient.transaction.getAll();
      expect(transactions).toHaveProperty('data');
      expect(Array.isArray(transactions.next)).toBe(true);
    });

    test('should handle getting non-existent transaction (inherited)', async () => {
      if (skipE2E) return;

      const transaction =
        await adminClient.transaction.get('non-existent-tx-id');
      expect(transaction).toBeNull();
    });
  });

  describe('Inherited Base Methods', () => {
    test('should get relayer address (inherited)', async () => {
      if (skipE2E) return;

      const address = await adminClient.address();
      expect(typeof address).toBe('string');
      expect(address).toMatch(/^0x[a-fA-F0-9]{40}$/);
    });

    test('should get relayer information (inherited)', async () => {
      if (skipE2E) return;

      const info = await adminClient.getInfo();
      expect(info).toHaveProperty('address');
      expect(info).toHaveProperty('chainId');
    });

    test('should get relayer balance (inherited)', async () => {
      if (skipE2E) return;

      const balance = await adminClient.getBalanceOf();
      expect(typeof balance).toBe('string');
      expect(parseFloat(balance)).toBeGreaterThanOrEqual(0);
    });

    test('should get allowlist (inherited)', async () => {
      if (skipE2E) return;

      const allowlist = await adminClient.allowlist.get();
      expect(allowlist).toHaveProperty('data');
      expect(Array.isArray(allowlist.next)).toBe(true);
    });

    test('should sign text message (inherited)', async () => {
      if (skipE2E) return;

      const message = 'Admin signed message';
      const result = await adminClient.sign.text(message);
      expect(result).toHaveProperty('signature');
      expect(typeof result.signature).toBe('string');
      expect(result.signature).toMatch(/^0x[a-fA-F0-9]{130}$/);
    });
  });

  describe('Authentication and Error Handling', () => {
    test('should handle invalid credentials', async () => {
      if (skipE2E) return;

      const invalidAdminClient = new AdminRelayerClient({
        serverUrl: TEST_CONFIG.SERVER_URL,
        providerUrl: TEST_CONFIG.PROVIDER_URL,
        relayerId: 'invalid-relayer-id',
        auth: {
          username: 'invalid-username',
          password: 'invalid-password',
        },
      });

      await expect(invalidAdminClient.getInfo()).rejects.toThrow();
    });

    test('should handle unauthorized admin operations', async () => {
      if (skipE2E) return;

      // Create a regular relayer client (not admin) and try admin operations
      const regularClient = new AdminRelayerClient({
        serverUrl: TEST_CONFIG.SERVER_URL,
        providerUrl: TEST_CONFIG.PROVIDER_URL,
        relayerId: 'some-relayer-id',
        auth: {
          username: 'regular-user',
          password: 'regular-password',
        },
      });

      // This should fail if the user doesn't have admin privileges
      await expect(regularClient.pause()).rejects.toThrow();
    });

    test('should handle network errors in admin operations', async () => {
      if (skipE2E) return;

      const networkErrorClient = new AdminRelayerClient({
        serverUrl: 'http://invalid-server-url:9999',
        providerUrl: TEST_CONFIG.PROVIDER_URL,
        relayerId: 'some-relayer-id',
        auth: {
          username: TEST_CONFIG.USERNAME,
          password: TEST_CONFIG.PASSWORD,
        },
      });

      await expect(networkErrorClient.pause()).rejects.toThrow();
    });
  });

  describe('Gas Price Management', () => {
    test('should handle gas price operations sequence', async () => {
      if (skipE2E) return;

      // Set a max gas price
      const maxGasPrice = '2000000000'; // 2 gwei
      await expect(
        adminClient.updateMaxGasPrice(maxGasPrice)
      ).resolves.not.toThrow();

      // Remove the max gas price
      await expect(adminClient.removeMaxGasPrice()).resolves.not.toThrow();
    });
  });
});
