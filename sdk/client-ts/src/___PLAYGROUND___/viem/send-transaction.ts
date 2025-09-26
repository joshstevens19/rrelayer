import { begin, getAnvilAccounts } from "../helpers";
import {createPublicClient, createWalletClient, custom, parseEther} from "viem";

export const sendTransaction = async () => {
    const context = await begin();
    const accounts = getAnvilAccounts();

    console.log("Sending transaction...");

    let chain = await context.relayer.getViemChain();

    const walletClient = createWalletClient({
        account: await context.relayer.address(),
        chain,
        transport: custom(context.relayer.ethereumProvider()),
    });
    const publicClient = createPublicClient({
        chain,
        transport: await context.client.getViemHttp(chain.id)
    });

    const hash = await walletClient.sendTransaction({
        to: accounts[1].address,
        value: parseEther('0.001')
    });

    const receipt = await publicClient.waitForTransactionReceipt({
        hash
    });

    console.log("Transaction receipt:", receipt);

    await context.end();
};

sendTransaction().then(() => console.log("send-transaction done"));