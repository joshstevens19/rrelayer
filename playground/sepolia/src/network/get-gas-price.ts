import {client} from "../client";

export const getGasPrice = async () => {

  console.log('Getting gas price...');
  const gasPrice = await client.network.getGasPrices(11155111);
  console.log('Gas price:', gasPrice);
};

getGasPrice().then(() => console.log('get-gas-price done'));
