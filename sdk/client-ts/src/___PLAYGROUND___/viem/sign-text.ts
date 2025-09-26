import { begin } from "../helpers";
import {createWalletClient, custom} from "viem";

export const signText = async () => {
    const context = await begin();

    const walletClient = createWalletClient({
        chain: await context.relayer.getViemChain(),
        transport: custom(context.relayer.ethereumProvider()),
    });

    const [account] = await walletClient.getAddresses();

    console.log("Signing text message...");
    const message = `Hello from SDK using viem test at ${new Date().toISOString()}`;
    const signature = await walletClient.signMessage({
        account,
        message,
    });

    console.log("Message:", message);
    console.log("Signature:", signature);

    await context.end();
};

signText().then(() => console.log("sign-text done"));