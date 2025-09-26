import { begin } from "../helpers";

export const getAddress = async () => {
    const context = await begin();

    console.log("Getting relayer address...");
    const address = await context.relayer.address();
    console.log("Relayer address:", address);

    await context.end();
};

getAddress().then(() => console.log("get-address done"));