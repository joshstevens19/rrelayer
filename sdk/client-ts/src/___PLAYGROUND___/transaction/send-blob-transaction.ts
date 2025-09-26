import { begin, getAnvilAccounts } from "../helpers";
import {createBlobFromString} from "../../clients";

export const sendBlobTransaction = async () => {
    const context = await begin("5", "hello", false);
    const accounts = getAnvilAccounts();

    const blobData = createBlobFromString('hello world');

    console.log("Sending transaction...");
    const txRequest = {
        to: accounts[1].address,
        blobs: [blobData]
    };

    const response = await context.relayer.transaction.send(txRequest);
    console.log("Transaction sent:", response);

    let receipt = await context.relayer.transaction.waitForTransactionReceiptById(response.id);
    console.log("Transaction receipt:", receipt);

    await context.end();
};

sendBlobTransaction().then(() => console.log("send-transaction done"));