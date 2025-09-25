// import { createWalletClient, custom } from 'viem';
// import { createClient } from '../clients';
// import { createRelayerClient } from "../clients/core";
//
// export const integration = async () => {
//   // BASIC AUTH USER FLOW
//   const client = createClient({
//     serverUrl: 'http://127.0.0.1:8000',
//     basicAuthUsername: "USERNAME",
//     basicAuthPassword: "PASSWORD"
//   });
//
//   client.relayer.create
//   client.relayer.delete
//   client.relayer.get
//   client.relayer.getAll
//   client.network.getAll
//   client.network.getGasPrices
//   client.transaction.get
//   client.transaction.getStatus
//
//   const relayer_main = await client.getRelayerClient('b12be8a1-7e95-4a5c-b46a-fbd124ee5771')
//   relayer_main.pause()
//   relayer_main.unpause()
//   relayer_main.removeMaxGasPrice()
//
//   // API USER FLOW
//
//   const relayer_api = await createRelayerClient({
//     serverUrl: 'http://127.0.0.1:8000',
//     relayerId: 'b12be8a1-7e95-4a5c-b46a-fbd124ee5771',
//     apiKey: 'hmKQ1Q9svKBsif3zQlwjPojYbpVJA3Uu',
//   });
//
//   relayer_api.allowlist.get
//   relayer_api.getInfo;
//   relayer_api.getBalanceOf()
//   relayer_api.
//
//
//
//   const relayerInfo = await relayer.info();
//   console.log(relayerInfo);
//
//   const walletClient = createWalletClient({
//     transport: custom(relayer_api.ethereumProvider()),
//   });
//
//   const [address] = await walletClient.getAddresses();
//   console.log(address);
//
//   const message = await walletClient.signMessage({
//     account: relayerInfo.address,
//     message: 'hey',
//   });
//
//   console.log(message);
// };
//
// // b12be8a1-7e95-4a5c-b46a-fbd124ee5771
// // hmKQ1Q9svKBsif3zQlwjPojYbpVJA3Uu
//
// integration();
