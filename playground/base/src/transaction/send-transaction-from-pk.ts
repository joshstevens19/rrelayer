import {client} from "../client";
import { TransactionToSend }  from "rrelayer";

export const sendTransaction = async () => {
  const relayerClient = await client.getRelayerClient('2ba12e4f-ca5b-48ec-9bd5-a51179f504dc');

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
