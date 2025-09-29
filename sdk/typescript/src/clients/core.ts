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
  getRelayerAllowlistAddress,
  TransactionSpeed, cloneRelayer,
  getNetwork,
} from '../api';
import { RelayerClient } from './relayer';
import {
  ApiBaseConfig,
  defaultPagingContext,
  PagingContext,
  PagingResult,
} from '../api/types';
import { AdminRelayerClient } from './admin';
import { http } from 'viem';

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
  providerUrl: string;
  speed?: TransactionSpeed;
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
       * Create a new relayer
       * @param chainId The chain id to create the relayer on
       * @param name The name of the relayer
       * @returns Promise<CreateRelayerResult>
       */
      create: async (
        chainId: number,
        name: string
      ): Promise<CreateRelayerResult> => {
        return createRelayer(chainId, name, this._apiBaseConfig);
      },
      /**
       * Clone an existing relayer
       * @param relayerId The relayer id you want to clone
       * @param chainId The chain id to clone the relayer to
       * @param name The name of the new relayer
       * @returns Promise<CreateRelayerResult>
       */
      clone: async (
          relayerId: string,
          chainId: number,
          name: string
      ): Promise<CreateRelayerResult> => {
        return cloneRelayer(relayerId, chainId, name, this._apiBaseConfig);
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
      get: async (chainId: number): Promise<Network | null> => {
        return getNetwork(chainId, apiBaseConfig);
      },
      /**
       * get all networks
       * @returns Network array
       */
      getAll: (): Promise<Network[]> => {
        return getAllNetworks(apiBaseConfig);
      },
      /**
       * Get gas prices for the network
       * @param chainId The chain id
       */
      getGasPrices(
        chainId: number
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

  public get allowlist() {
    return {
      /**
       * Get the relayer allowlist
       * @returns An address of allowlist addresses
       */
      get: (
        relayerId: string,
        pagingContext: PagingContext = defaultPagingContext
      ): Promise<PagingResult<string>> => {
        return getRelayerAllowlistAddress(
          relayerId,
          pagingContext,
          this._apiBaseConfig
        );
      },
    };
  }

  /**
   * Create admin relayer client
   * @param relayerId The relayer id
   * @param speed How fast you want the transactions to be mined at - optional defaults to fast
   * @returns AdminRelayerClient The admin relayer client
   */
  public async getRelayerClient(
    relayerId: string,
    speed?: TransactionSpeed
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
      speed,
    });
  }

  /**
   * Create viem http instance
   * @param chain_id The chain id
   * @returns HttpTransport - Viem standard
   */
  public async getViemHttp(chain_id: number) {
    let networks = await this.network.get(chain_id);
    if (!networks) {
      throw new Error(`Chain ${chain_id} not found`);
    }

    if (!networks.providerUrls) {
      throw new Error(`Chain ${chain_id} has no provider urls`);
    }

    return http(networks.providerUrls[0]);
  }
}

export const createClient = (config: CreateClientConfig): Client => {
  return new Client(config);
};

export const createRelayerClient = (
  config: CreateRelayerClientConfig
): RelayerClient => {
  return new RelayerClient({
    serverUrl: config.serverUrl,
    providerUrl: config.providerUrl,
    relayerId: config.relayerId,
    auth: {
      apiKey: config.apiKey,
    },
  });
};
