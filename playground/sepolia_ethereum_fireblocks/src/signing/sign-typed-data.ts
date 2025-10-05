import {client} from "../client";

export const signTypedData = async () => {
  const relayerClient = await client.getRelayerClient('5f8c51c2-949e-4711-80cf-0adcef872fbe');

  console.log('Signing typed data...');

  const domain = {
    name: 'Test App',
    version: '1',
    chainId: 11155111,
    verifyingContract:
      '0x1234567890123456789012345678901234567890' as `0x${string}`,
  };

  const types = {
    Person: [
      { name: 'name', type: 'string' },
      { name: 'wallet', type: 'address' },
    ],
  };

  const value = {
    name: 'Alice',
    wallet: await relayerClient.address(),
  };

  const typedData = {
    domain,
    types,
    primaryType: 'Person' as const,
    message: value,
  };

  const signature = await relayerClient.sign.typedData(typedData);

  console.log('Domain:', domain);
  console.log('Types:', types);
  console.log('Value:', value);
  console.log('Signature:', signature);
};

signTypedData().then(() => console.log('sign-typed-data done'));
