// Test environment setup
// Add any global test configuration here

export const TEST_CONFIG = {
  SERVER_URL: process.env.TEST_SERVER_URL || 'http://localhost:3000',
  PROVIDER_URL: process.env.TEST_PROVIDER_URL || 'http://localhost:8545',
  USERNAME: process.env.TEST_USERNAME || 'test-username',
  PASSWORD: process.env.TEST_PASSWORD || 'test-password',
  CHAIN_ID: process.env.TEST_CHAIN_ID || '31337',
};

// Helper to create a new relayer for testing
export const createTestRelayer = async (client: any, testName: string) => {
  const chainId = parseInt(TEST_CONFIG.CHAIN_ID);
  const name = `${testName}-${Date.now()}`;
  return await client.relayer.create(chainId, name);
};

// Skip tests if no test server URL is provided
const skipE2E = !process.env.TEST_SERVER_URL;

if (skipE2E) {
  console.warn(
    'E2E tests will be skipped. Set TEST_SERVER_URL to run against a real server.'
  );
}

export { skipE2E };
