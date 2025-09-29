import { begin } from '../helpers';
import { ethers } from 'ethers';

// run with:
// npm run playground::ethers::sign-typed-data
export const signTypedData = async () => {
  const context = await begin();

  const provider = new ethers.BrowserProvider(
    context.relayer.ethereumProvider()
  );
  const signer = await provider.getSigner();
  const network = await provider.getNetwork();

  console.log('Signing typed data...');

  const domain = {
    name: 'Test App',
    version: '1',
    chainId: Number(network.chainId),
    verifyingContract: '0x1234567890123456789012345678901234567890',
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

  const signature = await signer.signTypedData(domain, types, value);

  console.log('Domain:', domain);
  console.log('Types:', types);
  console.log('Value:', value);
  console.log('Signature:', signature);
  console.log('Signer address:', await signer.getAddress());

  await context.end();
};

signTypedData().then(() => console.log('sign-typed-data done'));
