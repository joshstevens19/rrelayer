import { begin } from '../helpers';
import { ethers } from 'ethers';

// run with:
// npm run playground::ethers::sign-text
export const signText = async () => {
  const context = await begin();

  const provider = new ethers.BrowserProvider(
    context.relayer.ethereumProvider()
  );
  const signer = await provider.getSigner();

  console.log('Signing text message...');
  const message = `Hello from SDK using ethers test at ${new Date().toISOString()}`;
  const signature = await signer.signMessage(message);

  console.log('Message:', message);
  console.log('Signature:', signature);
  console.log('Signer address:', await signer.getAddress());

  await context.end();
};

signText().then(() => console.log('sign-text done'));
