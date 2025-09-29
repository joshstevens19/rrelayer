import { begin } from '../helpers';

export const getNetwork = async () => {
  let context = await begin();

  let networks = await context.client.network.get(31337);
  console.log('networks', networks);

  await context.end();
};

getNetwork().then((_) => () => console.log('get-network done'));
