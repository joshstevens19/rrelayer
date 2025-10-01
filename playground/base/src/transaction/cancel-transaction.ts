import {client} from "../client";

export const sendTransaction = async () => {
  const relayerClient = await client.getRelayerClient('1ddd52e1-c925-4fa0-86c7-dcc890ca94e1');


  const transaction = await relayerClient.transaction.cancel('e8b63efa-c96d-4454-ac88-5cb3f50ec400');
  console.log('transaction', transaction);
  const startTime = Date.now();

  let receipt = await relayerClient.transaction.waitForTransactionReceiptById(transaction.id);
  console.log('receipt', receipt);
  const endTime = Date.now();
  const timeTaken = endTime - startTime;
  console.log(`⏱️  Time to receipt: ${timeTaken}ms (${(timeTaken / 1000).toFixed(2)}s)`);
};

sendTransaction().then(() => console.log('send-transaction done'));
