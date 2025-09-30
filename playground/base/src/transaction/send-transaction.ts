import {client} from "../client";
import { TransactionToSend }  from "rrelayer";

export const sendTransaction = async () => {
  const relayerClient = await client.getRelayerClient('d6dd6bcc-6a7d-4645-bf83-663da3bae8cd');

  let request: TransactionToSend = {
    to: "0xafa06f7fb602f11275c2a2e9afa3a00c0f7c27d6",
    value: '1000'
  }
  const transaction = await relayerClient.transaction.send(request);
  console.log('transaction', transaction);

  let receipt = await relayerClient.transaction.waitForTransactionReceiptById(transaction.id);
  console.log('receipt', receipt);
};

sendTransaction().then(() => console.log('send-transaction done'));
