import { begin } from '../helpers';

export const getRelayer = async () => {
  const context = await begin();

  console.log('Getting relayer info...');
  const relayerInfo = await context.client.relayer.get(context.relayerInfo.id);
  console.log('Relayer info:', relayerInfo);

  await context.end();
};

getRelayer().then(() => console.log('get-relayer done'));
