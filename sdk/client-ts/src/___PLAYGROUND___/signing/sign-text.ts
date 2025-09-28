import { begin } from '../helpers';

export const signText = async () => {
  const context = await begin();

  console.log('Signing text message...');
  const message = `Hello from SDK test at ${new Date().toISOString()}`;
  const signature = await context.relayer.sign.text(message);

  console.log('Message:', message);
  console.log('Signature:', signature);

  await context.end();
};

signText().then(() => console.log('sign-text done'));
