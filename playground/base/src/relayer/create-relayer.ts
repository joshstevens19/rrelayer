import {client} from "../client";


// created_result {
//   id: 'd6dd6bcc-6a7d-4645-bf83-663da3bae8cd',
//   address: '0x7a0f605c8366373764760673020b6b2d8574f3f2'
// }
export const createRelayer = async () => {
  let createRelayerResult = await client.relayer.create(8453, 'fancy-relayer');
  console.log('created_result', createRelayerResult);
};

createRelayer().then(() => console.log('create-relayer done'));
