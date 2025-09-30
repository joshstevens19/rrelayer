
import {client} from "../client";
import { TransactionToSend , createBlobFromString}  from "rrelayer";

export const sendBlobTransaction = async () => {
  const relayerClient = await client.getRelayerClient('51207abe-b9e0-4dd0-b843-76522d3e47c6');

  const blobData = createBlobFromString('hello world');

  let request: TransactionToSend = {
    to: "0xafa06f7fb602f11275c2a2e9afa3a00c0f7c27d6",
    blobs: [blobData],
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

sendBlobTransaction().then(() => console.log('send-transaction done'));
