import { begin } from "../helpers";

export const getAllTransactions = async () => {
    const context = await begin();

    console.log("Getting all transactions...");
    const transactions = await context.relayer.transaction.getAll();
    console.log("All transactions:", transactions);

    await context.end();
};

getAllTransactions().then(() => console.log("get-all-transactions done"));