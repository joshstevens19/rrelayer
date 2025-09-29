import { begin } from '../helpers';

export const getAllowlist = async () => {
  const context = await begin();

  console.log('Getting relayer allowlist...');
  const allowlists = await context.relayer.allowlist.get();
  console.log('Relayer address:', allowlists);

  await context.end();
};

getAllowlist().then(() => console.log('get-allowlist done'));
