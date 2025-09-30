import {client} from "../client";
import { TransactionToSend }  from "rrelayer";

export const sendTransaction = async () => {
  const relayerClient = await client.getRelayerClient('51207abe-b9e0-4dd0-b843-76522d3e47c6');

  let request: TransactionToSend = {
    to: "0xb744dd8f994b65ab7da7bc5a92b93c8209be9908",
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
