import {client} from "../client";


// created_result {
//   id: '5f8c51c2-949e-4711-80cf-0adcef872fbe',
//   address: '0xdb92af21bca0267598e45dad8176a2bdc3d5be9f'
// }

// created_result {
//   id: 'd2e59729-d6b0-4675-9042-9c9f886f7a7f',
//   address: '0xc0c684d4eb902437a5b32d8f6ab02ef0d65a9d06'
// }
export const createRelayer = async () => {
  let createRelayerResult = await client.relayer.create(11155111, 'fancy-relayer');
  console.log('created_result', createRelayerResult);
};

createRelayer().then(() => console.log('create-relayer done'));
