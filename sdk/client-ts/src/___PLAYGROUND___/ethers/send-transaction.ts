import { begin, getAnvilAccounts } from '../helpers';
import { ethers } from 'ethers';

// run with:
// npm run playground::ethers::send-transaction
export const sendTransaction = async () => {
  const context = await begin();
  const accounts = getAnvilAccounts();

  console.log('Sending transaction...');

  const provider = new ethers.BrowserProvider(
    context.relayer.ethereumProvider()
  );
  const signer = await provider.getSigner();

  console.log('Sending to:', accounts[1].address);
  console.log('From address:', await signer.getAddress());

  const tx = await signer.sendTransaction({
    to: accounts[1].address,
    value: ethers.parseEther('0.001'),
  });

  console.log('Transaction hash:', tx.hash);
  console.log('Waiting for transaction receipt...');

  const receipt = await tx.wait();
  console.log('Transaction receipt:', receipt);

  await context.end();
};

sendTransaction().then(() => console.log('send-transaction done'));
