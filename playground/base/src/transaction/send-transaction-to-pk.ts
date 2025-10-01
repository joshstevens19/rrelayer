import {client} from "../client";
import { TransactionToSend }  from "rrelayer";

export const sendTransaction = async () => {
  const relayerClient = await client.getRelayerClient('a08fb802-b2b0-42e1-a271-3e4a21115058');

  let request: TransactionToSend = {
    to: "0x12DA2589E40855EC80E017215D8B4B92CF21C8AC",
    value: '1000000000000000'
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
