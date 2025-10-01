import { begin } from '../helpers';

export const cloneRelayer = async () => {
  const context = await begin();

  console.log('Creating new relayer...');
  const relayer = await context.client.relayer.clone(
    '94afb207-bb47-4392-9229-ba87e4d783cb',
    31337,
    `test-relayer-${Date.now()}`
  );
  console.log('Created relayer:', relayer);

  // Clean up - delete the test relayer
  await context.client.relayer.delete(relayer.id);
  console.log('Test relayer cleaned up');

  await context.end();
};

cloneRelayer().then(() => console.log('clone-relayer done'));
