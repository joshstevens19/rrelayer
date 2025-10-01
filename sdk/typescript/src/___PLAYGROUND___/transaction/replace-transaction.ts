import { begin, getAnvilAccounts } from '../helpers';

export const replaceTransaction = async () => {
  const context = await begin();
  const accounts = getAnvilAccounts();

  console.log('Replacing transaction...');
  const txRequest = {
    to: accounts[1].address,
    value: '1000000000000000000',
  };

  const response = await context.relayer.transaction.replace(
    'ebf8a8c1-9de5-4307-9810-8e842dad7bde',
    txRequest
  );
  console.log('Replaced transaction sent:', response);

  if (!response.success) {
    console.log('Replaced transaction failed:', response);
    return;
  }

  let receipt = await context.relayer.transaction.waitForTransactionReceiptById(
    response.replaceTransactionId
  );
  console.log('Replaced transaction receipt:', receipt);

  await context.end();
};

replaceTransaction().then(() => console.log('replaced-transaction done'));
