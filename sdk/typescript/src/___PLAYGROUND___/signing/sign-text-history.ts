import { begin } from '../helpers';

export const signTextHistory = async () => {
  const context = await begin();

  console.log('Getting signing text history...');
  const result = await context.relayer.sign.textHistory({
    limit: 100,
    offset: 0,
  });

  console.log('result:', result);

  await context.end();
};

signTextHistory().then(() => console.log('sign-text-history done'));
