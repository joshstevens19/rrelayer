import { begin } from "../helpers";

export const getAllRelayers = async () => {
    const context = await begin();

    console.log("Getting all relayers...");
    const relayers = await context.client.relayer.getAll();
    console.log("All relayers:", relayers);

    await context.end();
};

getAllRelayers().then(() => console.log("get-all-relayers done"));