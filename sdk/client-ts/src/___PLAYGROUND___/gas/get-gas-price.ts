import { begin } from "../helpers";

export const getGasPrice = async () => {
    const context = await begin();

    console.log("Getting gas price...");
    const gasPrice = await context.client.network.getGasPrices(31337);
    console.log("Gas price:", gasPrice);

    await context.end();
};

getGasPrice().then(() => console.log("get-gas-price done"));