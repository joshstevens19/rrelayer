import { begin, getAnvilAccounts } from '../helpers';
import { parseEther } from '../../index';

export const sendTransaction = async () => {
  const context = await begin();
  const accounts = getAnvilAccounts();

  console.log('Sending transaction...');
  const txRequest = {
    to: accounts[1].address,
    value: parseEther('1'),
  };

  const response = await context.client.transaction.sendRandom(1, txRequest);
  console.log('Transaction sent:', response);

  let receipt = await context.relayer.transaction.waitForTransactionReceiptById(
    response.id
  );
  console.log('Transaction receipt:', receipt);

  await context.end();
};

sendTransaction().then(() => console.log('send-transaction done'));
