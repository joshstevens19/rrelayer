import { begin } from '../helpers';

export const signTypedData = async () => {
  const context = await begin();

  console.log('Signing typed data...');

  const domain = {
    name: 'Test App',
    version: '1',
    chainId: 31337,
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
    wallet: context.relayerInfo.address,
  };

  const typedData = {
    domain,
    types,
    primaryType: 'Person' as const,
    message: value,
  };

  const signature = await context.relayer.sign.typedData(typedData);

  console.log('Domain:', domain);
  console.log('Types:', types);
  console.log('Value:', value);
  console.log('Signature:', signature);

  await context.end();
};

signTypedData().then(() => console.log('sign-typed-data done'));
