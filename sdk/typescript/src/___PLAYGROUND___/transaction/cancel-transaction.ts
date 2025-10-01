import { begin } from '../helpers';

export const cancelTransaction = async () => {
  const context = await begin();

  console.log('Cancel transaction...');

  const response = await context.relayer.transaction.cancel(
    'ebf8a8c1-9de5-4307-9810-8e842dad7bde'
  );
  console.log('Transaction sent:', response);

  if (!response.success) {
    console.log('Transaction failed:', response);
    return;
  }

  let receipt = await context.relayer.transaction.waitForTransactionReceiptById(
    response.cancelTransactionId
  );
  console.log('Transaction receipt:', receipt);

  await context.end();
};

cancelTransaction().then(() => console.log('cancel-transaction done'));
