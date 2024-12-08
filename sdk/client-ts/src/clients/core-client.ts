import { CreateRelayerResult, GetRelayerResult, getRelayer } from '../api';
import { AdminClient, AdminSetup } from './admin-client';
import { RelayerClient } from './relayer-client';

export interface RRelayerrConfig {
  serverUrl: string;
}

export interface CreateRelayerWithApiKeyResult {
  relayer: CreateRelayerResult;
  client: RelayerClient;
}

export interface CreateRelayerClientConfig {
  /**
   * Will use the relayers if not provided
   */
  providerUrl?: string;
  relayerId: string;
  apiKey: string;
}

export class RRelayerrClient {
  constructor(public config: RRelayerrConfig) {}

  /**
   * Create admin client
   * @returns AdminClient
   */
  public createAdminClient(setup: AdminSetup) {
    return new AdminClient(this, setup);
  }

  public get relayer() {
    return {
      /**
       * Get a relayer
       * @param id The id of the relayer
       * @returns Relayer
       */
      get: async (
        id: string,
        apiKey: string
      ): Promise<GetRelayerResult | null> => {
        const result = await getRelayer(id, {
          serverUrl: this.config.serverUrl,
          apiKey: apiKey,
        });
        return result;
      },
    };
  }

  /**
   * Create relayer client
   * @param config The relayer client config
   * @returns RelayerClient
   */
  public async createRelayerClient(
    config: CreateRelayerClientConfig
  ): Promise<RelayerClient> {
    const relayer = await this.relayer.get(config.relayerId, config.apiKey);
    if (!relayer) {
      throw new Error(`Relayer ${config.relayerId} not found`);
    }

    let providerUrl = config.providerUrl;
    if (!providerUrl) {
      if (relayer.providerUrls.length === 0) {
        throw new Error('Please provide a provider url');
      }
      providerUrl = relayer.providerUrls[0];
    }

    return new RelayerClient(
      {
        ...config,
        providerUrl: providerUrl,
        auth: {
          apiKey: config.apiKey,
        },
      },
      this.config
    );
  }
}

export const createRRelayerrClient = (
  config: RRelayerrConfig
): RRelayerrClient => {
  return new RRelayerrClient(config);
};
