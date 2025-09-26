import { begin } from "../helpers";

export const createRelayer = async () => {
    const context = await begin();

    console.log("Creating new relayer...");
    const relayer = await context.client.relayer.create(31337, `test-relayer-${Date.now()}`);
    console.log("Created relayer:", relayer);

    // Clean up - delete the test relayer
    await context.client.relayer.delete(relayer.id);
    console.log("Test relayer cleaned up");

    await context.end();
};

createRelayer().then(() => console.log("create-relayer done"));