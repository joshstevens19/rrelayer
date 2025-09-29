import { begin } from '../helpers';

export const getAllNetworks = async () => {
  let context = await begin();

  let networks = await context.client.network.getAll();
  console.log('networks', networks);

  await context.end();
};

getAllNetworks().then((_) => () => console.log('get-all-networks done'));
