import {client} from "../client";


// created_result  {
//   id: '51207abe-b9e0-4dd0-b843-76522d3e47c6',
//   address: '0x317b31b197c24bea5a64d67b6809a67203a3f43c'
// }
export const createRelayer = async () => {
  let created_result = await client.relayer.create(11155111, 'fancy-relayer');
  console.log('created_result', created_result);
};

createRelayer().then(() => console.log('create-relayer done'));
