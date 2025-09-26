import { begin } from "../helpers";
import { TransactionCountType } from "../../clients";

export const getTransactionCounts = async () => {
    const context = await begin();

    console.log("Getting transaction counts...");
    
    const pendingCount = await context.relayer.transaction.getCount(TransactionCountType.PENDING);
    console.log("Pending transactions:", pendingCount);

    const inmempoolCount = await context.relayer.transaction.getCount(TransactionCountType.INMEMPOOL);
    console.log("In mempool transactions:", inmempoolCount);

    await context.end();
};

getTransactionCounts().then(() => console.log("get-transaction-counts done"));