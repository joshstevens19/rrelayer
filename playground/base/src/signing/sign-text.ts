import {client} from "../client";


export const signText = async () => {
    const relayerClient = await client.getRelayerClient('1ddd52e1-c925-4fa0-86c7-dcc890ca94e1');

    console.log('Signing text message...');
    const message = `Hello from SDK test at ${new Date().toISOString()}`;
    const signature = await relayerClient.sign.text(message);

    console.log('Message:', message);
    console.log('Signature:', signature);
};

signText().then(() => console.log('sign-text done'));
