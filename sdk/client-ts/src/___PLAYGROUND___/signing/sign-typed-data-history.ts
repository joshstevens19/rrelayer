import { begin } from '../helpers';

export const signTypedDataHistory = async () => {
  const context = await begin();

  console.log('Getting signing text history...');
  const result = await context.relayer.sign.typedDataHistory({
    limit: 100,
    offset: 0,
  });

  console.log('result:', result);

  await context.end();
};

signTypedDataHistory().then(() => console.log('sign-typed-data-history done'));
