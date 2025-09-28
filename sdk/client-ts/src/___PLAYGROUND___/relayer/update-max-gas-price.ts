import { begin } from '../helpers';

export const updateMaxGasPrice = async () => {
  const context = await begin();

  console.log('Setting max gas price to 2 gwei...');
  await context.relayer.updateMaxGasPrice('2000000000');
  console.log('Max gas price set to 2 gwei');

  console.log('Setting max gas price to 5 gwei...');
  await context.relayer.updateMaxGasPrice('5000000000');
  console.log('Max gas price set to 5 gwei');

  await context.end();
};

updateMaxGasPrice().then(() => console.log('update-max-gas-price done'));
