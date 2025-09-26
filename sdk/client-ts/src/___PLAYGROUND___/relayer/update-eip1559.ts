import { begin } from "../helpers";

export const updateEip1559 = async () => {
    const context = await begin();

    console.log("Updating EIP1559 status to true...");
    await context.relayer.updateEIP1559Status(true);
    console.log("EIP1559 status updated to true");

    console.log("Updating EIP1559 status to false...");
    await context.relayer.updateEIP1559Status(false);
    console.log("EIP1559 status updated to false");

    await context.end();
};

updateEip1559().then(() => console.log("update-eip1559 done"));