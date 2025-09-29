import { begin } from '../helpers';

export const testAuth = async () => {
  const context = await begin();

  console.log('Testing authentication...');

  // Test auth by getting networks (requires valid auth)
  try {
    const networks = await context.client.network.getAll();
    console.log('Authentication successful - got networks:', networks.length);
  } catch (error) {
    console.error('Authentication failed:', error);
  }

  await context.end();
};

testAuth().then(() => console.log('test-auth done'));
