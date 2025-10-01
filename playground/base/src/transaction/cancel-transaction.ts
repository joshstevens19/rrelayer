import {client} from "../client";
import { TransactionToSend }  from "rrelayer";

export const sendTransaction = async () => {
  const relayerClient = await client.getRelayerClient('1ddd52e1-c925-4fa0-86c7-dcc890ca94e1');

  let request: TransactionToSend = {
    to: "0xafa06f7fb602f11275c2a2e9afa3a00c0f7c27d6",
    value: '1000'
  }
  const transaction = await relayerClient.transaction.send(request);
  console.log('transaction', transaction);
  const startTime = Date.now();

  let receipt = await relayerClient.transaction.waitForTransactionReceiptById(transaction.id);
  console.log('receipt', receipt);
  const endTime = Date.now();
  const timeTaken = endTime - startTime;
  console.log(`⏱️  Time to receipt: ${timeTaken}ms (${(timeTaken / 1000).toFixed(2)}s)`);
};

sendTransaction().then(() => console.log('send-transaction done'));
