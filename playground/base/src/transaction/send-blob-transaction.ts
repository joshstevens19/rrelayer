
import {client} from "../client";
import { TransactionToSend , createBlobFromString}  from "rrelayer";

export const sendBlobTransaction = async () => {
  const relayerClient = await client.getRelayerClient('d6dd6bcc-6a7d-4645-bf83-663da3bae8cd');

  const blobData = createBlobFromString('hello world');

  let request: TransactionToSend = {
    to: "0xafa06f7fb602f11275c2a2e9afa3a00c0f7c27d6",
    value: '1000',
    blobs: [blobData],
  }
  const transaction = await relayerClient.transaction.send(request);
  console.log('transaction', transaction);
};

sendBlobTransaction().then(() => console.log('send-transaction done'));
