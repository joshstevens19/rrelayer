import { begin } from "../helpers";

export const getBalance = async () => {
    const context = await begin();

    console.log("Getting relayer balance...");
    const balance = await context.relayer.getBalanceOf();
    console.log("Relayer balance:", balance, "ETH");

    await context.end();
};

getBalance().then(() => console.log("get-balance done"));