import {client} from "../client";


export const signText = async () => {
    const relayerClient = await client.getRelayerClient('5f8c51c2-949e-4711-80cf-0adcef872fbe');

    console.log('Signing text message...');
    const message = `Hello from SDK test at ${new Date().toISOString()}`;
    const signature = await relayerClient.sign.text(message);

    console.log('Message:', message);
    console.log('Signature:', signature);
};

signText().then(() => console.log('sign-text done'));
