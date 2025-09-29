import { begin } from '../helpers';

export const pauseUnpause = async () => {
  const context = await begin();

  console.log('Pausing relayer...');
  await context.relayer.pause();
  console.log('Relayer paused');

  console.log('Unpausing relayer...');
  await context.relayer.unpause();
  console.log('Relayer unpaused');

  await context.end();
};

pauseUnpause().then(() => console.log('pause-unpause done'));
