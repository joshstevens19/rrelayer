import { Hex, WalletClient, createWalletClient, http } from 'viem';
import { privateKeyToAccount } from 'viem/accounts';
import {
  CreateRelayerResult,
  GasEstimatorResult,
  GetRelayerResult,
  Network,
  Relayer,
  User,
  addUser,
  authenticate,
  createRelayer,
  createRelayerApiKey,
  deleteRelayer,
  deleteRelayerApiKey,
  deleteUser,
  disableNetwork,
  editUser,
  enableNetwork,
  generateAuthSecret,
  getAllNetworks,
  getDisabledNetworks,
  getEnabledNetworks,
  getGasPrices,
  getRelayer,
  getRelayerApiKeys,
  getRelayers,
  getUsers,
  refreshAuthToken,
} from '../api';
import { JwtRole, TokenPair } from '../api/authentication/types';
import {
  ApiBaseConfig,
  PagingContext,
  PagingResult,
  defaultPagingContext,
} from '../api/types';
import { invariant } from '../utils';
import { RRelayerrClient } from './core-client';
import { RelayerClient } from './relayer-client';
import { NetworkStatus } from './types';

export type AdminSetup =
  | {
      accessPrivateKey: Hex;
      validTokenPair?: TokenPair;
    }
  | {
      walletClient: any;
      validTokenPair?: TokenPair;
    };

export class AdminClient {
  private _currentTokenPair: TokenPair | null = null;
  private _walletClient: WalletClient;
  constructor(private _rrelayerClient: RRelayerrClient, config: AdminSetup) {
    if ('accessPrivateKey' in config) {
      this._walletClient = createWalletClient({
        account: privateKeyToAccount(config.accessPrivateKey),
        transport: http(),
      });
    } else {
      this._walletClient = config.walletClient;
    }

    if (config.validTokenPair) {
      this._currentTokenPair = config.validTokenPair;
    }
  }

  private _apiBaseConfig = (): ApiBaseConfig => {
    invariant(this._currentTokenPair, 'No access token');
    return {
      serverUrl: this._rrelayerClient.config.serverUrl,
      authToken: this._currentTokenPair.accessToken,
    };
  };

  /**
   *  Get gas prices for a given chain
   * @param chainId The chain id
   * @returns GasEstimatorResult
   */
  public async getGasPrices(
    chainId: string | number
  ): Promise<GasEstimatorResult | null> {
    if (!this._currentTokenPair) {
      await this.authentication.authenticate();
    }

    return getGasPrices(chainId, this._apiBaseConfig());
  }

  public get user() {
    return {
      /**
       * Get all users
       * @returns User array
       */
      get: async (
        pagingContext: PagingContext = defaultPagingContext
      ): Promise<PagingResult<User>> => {
        if (!this._currentTokenPair) {
          await this.authentication.authenticate();
        }
        return getUsers(pagingContext, this._apiBaseConfig());
      },
      /**
       * Edit a user
       * @param user The user
       * @param newRole The new role
       * @returns void
       */
      edit: async (user: string, newRole: JwtRole): Promise<void> => {
        if (!this._currentTokenPair) {
          await this.authentication.authenticate();
        }
        return editUser(user, newRole, this._apiBaseConfig());
      },
      /**
       * Add a user
       * @param user The user
       * @param role The role
       * @returns void
       */
      add: async (user: string, role: JwtRole): Promise<void> => {
        if (!this._currentTokenPair) {
          await this.authentication.authenticate();
        }

        return addUser(user, role, this._apiBaseConfig());
      },
      /**
       * Delete a user
       * @param user The user
       * @returns void
       */
      delete: async (user: string): Promise<void> => {
        if (!this._currentTokenPair) {
          await this.authentication.authenticate();
        }
        return deleteUser(user, this._apiBaseConfig());
      },
    };
  }

  public get authentication() {
    return {
      /**
       * Authenticate a user
       * @returns TokenPair
       */
      authenticate: async (): Promise<TokenPair> => {
        const account = (await this._walletClient.getAddresses())[0];
        const secret = await generateAuthSecret(
          (
            await this._walletClient.getAddresses()
          )[0],
          {
            serverUrl: this._rrelayerClient.config.serverUrl,
          }
        );

        const signature = await this._walletClient.signMessage({
          account,
          message: secret.challenge,
        });

        const tokenPair = await authenticate(
          {
            id: secret.id,
            signedBy: account,
            signature,
          },
          {
            serverUrl: this._rrelayerClient.config.serverUrl,
          }
        );

        return (this._currentTokenPair = tokenPair);
      },
      /**
       * Refresh a token
       * @param refreshToken The refresh token
       * @returns TokenPair
       */
      refresh: async (refreshToken: string): Promise<TokenPair> => {
        const result = await refreshAuthToken(
          refreshToken,
          this._apiBaseConfig()
        );

        return (this._currentTokenPair = result);
      },
      /**
       * Check if the user is authenticated
       * @returns boolean
       */
      isAuthenticated: (): boolean => {
        return this._currentTokenPair !== null;
      },
    };
  }

  public get networks() {
    return {
      /**
       * disable a network
       * @param chainId The chain id
       * @returns void
       */
      disable: async (chainId: string | number): Promise<void> => {
        if (!this._currentTokenPair) {
          await this.authentication.authenticate();
        }
        return disableNetwork(chainId, this._apiBaseConfig());
      },
      /**
       * enable a network
       * @param chainId The chain id
       * @returns void
       */
      enable: async (chainId: string | number): Promise<void> => {
        if (!this._currentTokenPair) {
          await this.authentication.authenticate();
        }
        return enableNetwork(chainId, this._apiBaseConfig());
      },
      /**
       * get networks
       * @param status The network status
       * @returns Network array
       */
      get: (status: NetworkStatus = NetworkStatus.ALL): Promise<Network[]> => {
        switch (status) {
          case NetworkStatus.ALL:
            return getAllNetworks(this._apiBaseConfig());
          case NetworkStatus.DISABLED:
            return getDisabledNetworks(this._apiBaseConfig());
          case NetworkStatus.ENABLED:
            return getEnabledNetworks(this._apiBaseConfig());
          default:
            throw new Error('Invalid network status');
        }
      },
    };
  }

  public get relayer() {
    return {
      apiKeys: {
        /**
         * Create a new relayer api key
         * @param relayerId The relayer id
         * @returns The api key string
         */
        create: async (relayerId: string): Promise<string> => {
          if (!this._currentTokenPair) {
            await this.authentication.authenticate();
          }

          return createRelayerApiKey(relayerId, this._apiBaseConfig());
        },
        /**
         * Delete a relayer api key
         * @param relayerId The relayer id
         * @param apiKey The api key to delete
         * @returns void
         */
        delete: async (relayerId: string, apiKey: string): Promise<void> => {
          if (!this._currentTokenPair) {
            await this.authentication.authenticate();
          }

          return deleteRelayerApiKey(relayerId, apiKey, this._apiBaseConfig());
        },
        /**
         * Get the relayer api keys
         * @returns All the relayer API keys
         */
        get: (
          relayerId: string,
          pagingContext: PagingContext = defaultPagingContext
        ): Promise<PagingResult<string>> => {
          return getRelayerApiKeys(
            relayerId,
            pagingContext,
            this._apiBaseConfig()
          );
        },
      },
      /**
       * Create a new relayer api key
       * @param relayerId The relayer id
       * @returns string
       */
      createNewRelayer: async (
        chainId: string | number,
        name: string
      ): Promise<CreateRelayerResult> => {
        if (!this._currentTokenPair) {
          await this.authentication.authenticate();
        }

        return createRelayer(chainId, name, this._apiBaseConfig());
      },
      /**
       * Create a new relayer with API key returning a new instance to be able to be use
       * @param chainId The chain id
       * @param name The name of the relayer
       * @returns CreateRelayerWithApiKeyResult
       */
      createNewRelayerWithApiKey: async (
        chainId: string | number,
        name: string
      ) => {
        if (!this._currentTokenPair) {
          await this.authentication.authenticate();
        }

        const result = await createRelayer(
          chainId,
          name,
          this._apiBaseConfig()
        );
        const apiKey = await createRelayerApiKey(
          result.id,
          this._apiBaseConfig()
        );

        return {
          relayer: result,
          client: await this._rrelayerClient.createRelayerClient({
            relayerId: result.id,
            apiKey,
          }),
        };
      },
      /**
       * Delete a relayer
       * @returns void
       */
      delete: async (id: string): Promise<void> => {
        if (!this._currentTokenPair) {
          await this.authentication.authenticate();
        }

        return deleteRelayer(id, this._apiBaseConfig());
      },

      /**
       * Get a relayer
       * @param id The id of the relayer
       * @returns Relayer
       */
      get: async (id: string): Promise<GetRelayerResult | null> => {
        if (!this._currentTokenPair) {
          await this.authentication.authenticate();
        }

        return getRelayer(id, this._apiBaseConfig());
      },
      /**
       * Get a relayer
       * @param id The id of the relayer
       * @returns Relayer
       */
      getAll: async (
        pagingContext: PagingContext = defaultPagingContext,
        onlyForChainId: string | number | undefined = undefined
      ): Promise<PagingResult<Relayer>> => {
        if (!this._currentTokenPair) {
          await this.authentication.authenticate();
        }

        return getRelayers(
          onlyForChainId,
          pagingContext,
          this._apiBaseConfig()
        );
      },
      /**
       *  Create a relayer client from the admin
       * @param relayerId The relayer id
       * @returns RelayerClient
       */
      createRelayerClient: async (
        relayerId: string
      ): Promise<RelayerClient> => {
        if (!this._currentTokenPair) {
          await this.authentication.authenticate();
        }

        const relayer = await this.relayer.get(relayerId);
        if (!relayer) {
          throw new Error(`Relayer ${relayerId} not found`);
        }

        if (relayer.providerUrls.length === 0) {
          throw new Error('Please provide a provider url');
        }

        return new RelayerClient(
          {
            relayerId,
            providerUrl: relayer.providerUrls[0],
            auth: {
              authToken: this._currentTokenPair!.accessToken,
            },
          },
          this._rrelayerClient.config
        );
      },
    };
  }
}
