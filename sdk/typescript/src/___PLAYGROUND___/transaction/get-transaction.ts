import { begin } from '../helpers';

export const getTransaction = async () => {
  const context = await begin();

  const transaction = await context.relayer.transaction.get(
    'ebf8a8c1-9de5-4307-9810-8e842dad7bde'
  );
  console.log('transaction', transaction);
  await context.end();
};

getTransaction().then(() => console.log('get-transaction done'));
