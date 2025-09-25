import {
  CreateRelayerResult,
  GetRelayerResult,
  getRelayer,
  createRelayer,
  deleteRelayer,
  Relayer,
  getRelayers,
  Network,
  getAllNetworks,
  Transaction,
  getTransaction,
  TransactionStatusResult,
  getTransactionStatus,
  getGasPrices,
  GasEstimatorResult,
} from '../api';
import { RelayerClient } from './relayer';
import {
  ApiBaseConfig,
  defaultPagingContext,
  PagingContext,
  PagingResult,
} from '../api/types';
import { AdminRelayerClient } from './admin';

export interface CreateClientConfig {
  serverUrl: string;
  auth: {
    username: string;
    password: string;
  };
}

export interface CreateRelayerClientConfig {
  serverUrl: string;
  relayerId: string;
  apiKey: string;
}

export class Client {
  private readonly _apiBaseConfig: ApiBaseConfig;
  constructor(private readonly _config: CreateClientConfig) {
    this._apiBaseConfig = {
      serverUrl: _config.serverUrl,
      username: _config.auth.username,
      password: _config.auth.password,
    };
  }

  public get relayer() {
    return {
      /**
       * Create a new relayer api key
       * @param chainId The chain id to create the relayer on
       * @param name The name of the relayer
       * @returns string
       */
      create: async (
        chainId: string | number,
        name: string
      ): Promise<CreateRelayerResult> => {
        return createRelayer(chainId, name, this._apiBaseConfig);
      },
      /**
       * Delete a relayer
       * @returns void
       */
      delete: async (id: string): Promise<void> => {
        return deleteRelayer(id, this._apiBaseConfig);
      },
      /**
       * Get a relayer
       * @param id The id of the relayer
       * @returns Relayer
       */
      get: async (id: string): Promise<GetRelayerResult | null> => {
        return getRelayer(id, this._apiBaseConfig);
      },
      /**
       * Get a relayer
       * @param pagingContext The Paging information
       * @param onlyForChainId If you only want it based on a chain id
       * @returns Relayer
       */
      getAll: async (
        pagingContext: PagingContext = defaultPagingContext,
        onlyForChainId?: number
      ): Promise<PagingResult<Relayer>> => {
        return getRelayers(onlyForChainId, pagingContext, this._apiBaseConfig);
      },
    };
  }

  public get network() {
    const apiBaseConfig = this._apiBaseConfig;
    return {
      /**
       * get all networks
       * @returns Network array
       */
      getAll: (): Promise<Network[]> => {
        return getAllNetworks(apiBaseConfig);
      },
      getGasPrices(
        chainId: string | number
      ): Promise<GasEstimatorResult | null> {
        return getGasPrices(chainId, apiBaseConfig);
      },
    };
  }

  public get transaction() {
    return {
      /**
       * Get a transaction
       * @param transactionId The transaction id
       * @returns Transaction | null
       */
      get: (transactionId: string): Promise<Transaction | null> => {
        return getTransaction(transactionId, this._apiBaseConfig);
      },
      /**
       * Get a transaction status
       * @param transactionId The transaction id
       * @returns TransactionStatusResult | null
       */
      getStatus: (
        transactionId: string
      ): Promise<TransactionStatusResult | null> => {
        return getTransactionStatus(transactionId, this._apiBaseConfig);
      },
    };
  }

  /**
   * Create admin relayer client
   * @param relayerId The relayer id
   * @returns AdminRelayerClient The admin relayer client
   */
  public async getRelayerClient(
    relayerId: string
  ): Promise<AdminRelayerClient> {
    const relayer = await this.relayer.get(relayerId);
    if (!relayer) {
      throw new Error(`Relayer ${relayerId} not found`);
    }

    return new AdminRelayerClient({
      serverUrl: this._config.serverUrl,
      providerUrl: 'TODO',
      relayerId,
      auth: this._config.auth,
    });
  }
}

export const createClient = (config: CreateClientConfig): Client => {
  return new Client(config);
};

export const createRelayerClient = async (
  config: CreateRelayerClientConfig
): Promise<RelayerClient> => {
  return new RelayerClient({
    serverUrl: config.serverUrl,
    providerUrl: 'TODO',
    relayerId: config.relayerId,
    auth: {
      apiKey: config.apiKey,
    },
  });
};
