
import {client} from "../client";
import { TransactionToSend , createBlobFromString}  from "rrelayer";

export const sendBlobTransaction = async () => {
  const relayerClient = await client.getRelayerClient('1ddd52e1-c925-4fa0-86c7-dcc890ca94e1');

  const blobData = createBlobFromString('hello world');

  let request: TransactionToSend = {
    to: "0xafa06f7fb602f11275c2a2e9afa3a00c0f7c27d6",
    blobs: [blobData],
  }
  const transaction = await relayerClient.transaction.send(request);
  console.log('transaction', transaction);
};

sendBlobTransaction().then(() => console.log('send-transaction done'));
