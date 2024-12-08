import { createWalletClient, custom } from 'viem';
import { createRRelayerrClient } from '../clients';

export const integration = async () => {
  const client = createRRelayerrClient({
    serverUrl: 'http://127.0.0.1:8000',
  });

  const relayer = await client.createRelayerClient({
    relayerId: 'b12be8a1-7e95-4a5c-b46a-fbd124ee5771',
    apiKey: 'hmKQ1Q9svKBsif3zQlwjPojYbpVJA3Uu',
  });

  const relayerInfo = await relayer.info();
  console.log(relayerInfo);

  const walletClient = createWalletClient({
    transport: custom(relayer.ethereumProvider()),
  });

  const [address] = await walletClient.getAddresses();
  console.log(address);

  const message = await walletClient.signMessage({
    account: relayerInfo.address,
    message: 'hey',
  });

  console.log(message);
};

// b12be8a1-7e95-4a5c-b46a-fbd124ee5771
// hmKQ1Q9svKBsif3zQlwjPojYbpVJA3Uu

integration();
