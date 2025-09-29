import {
  Address,
  Chain,
  TransactionReceipt,
  TypedDataDefinition,
  formatEther,
  defineChain,
} from 'viem';
import * as knownChains from 'viem/chains';
import {
  Relayer,
  Transaction,
  TransactionSent,
  TransactionStatus,
  TransactionStatusResult,
  TransactionToSend,
  cancelTransaction,
  getRelayer,
  getRelayerAllowlistAddress,
  getTransaction,
  getTransactionStatus,
  getTransactions,
  replaceTransaction,
  sendTransaction,
  TransactionSpeed, SignedTextHistory, getSignedTextHistory, SignedTypedDataHistory, getSignedTypedDataHistory,
} from '../api';
import {
  ApiBaseConfig,
  PagingContext,
  PagingResult,
  defaultPagingContext,
} from '../api/types';
import { Provider } from '../provider';
import { invariant } from '../utils';
import {
  signText,
  SignTextResult,
  signTypedData,
  SignTypedDataResult,
} from '../api';

export interface RelayerClientConfig {
  serverUrl: string;
  providerUrl: string;
  relayerId: string;
  auth:
    | {
        apiKey: string;
      }
    | {
        username: string;
        password: string;
      };
  speed?: TransactionSpeed;
}

export class RelayerClient {
  public readonly id: string;
  public readonly speed: TransactionSpeed | undefined = undefined;
  protected readonly _apiBaseConfig: ApiBaseConfig;
  private readonly _ethereumProvider: Provider;
  constructor(config: RelayerClientConfig) {
    this.id = config.relayerId;
    this.speed = config.speed;
    this._ethereumProvider = new Provider(config.providerUrl, this);
    if ('apiKey' in config.auth) {
      this._apiBaseConfig = {
        serverUrl: config.serverUrl,
        apiKey: config.auth.apiKey,
      };
    } else {
      this._apiBaseConfig = {
        serverUrl: config.serverUrl,
        username: config.auth.username,
        password: config.auth.password,
      };
    }
  }

  /**
   * Get the relayer address
   * @returns string
   */
  public async address(): Promise<Address> {
    return (await this.getInfo()).address;
  }

  /**
   * Get the relayer information
   * @returns Relayer
   */
  public async getInfo(): Promise<Relayer> {
    const result = await getRelayer(this.id, this._apiBaseConfig);

    invariant(result, 'Relayer not found');

    return result.relayer;
  }

  /**
   * Get the relayer balance
   * @returns string
   */
  public async getBalanceOf(): Promise<string> {
    // @ts-ignore - use the viem getBalance method without exposing the client (which is not needed here)
    const balance = await this._ethereumProvider._client.getBalance({
      address: await this.address(),
    });

    return formatEther(balance);
  }

  public get allowlist() {
    return {
      /**
       * Get the relayer allowlist
       * @returns An address of allowlist addresses
       */
      get: (
        pagingContext: PagingContext = defaultPagingContext
      ): Promise<PagingResult<`0x${string}`>> => {
        return getRelayerAllowlistAddress(
          this.id,
          pagingContext,
          this._apiBaseConfig
        );
      },
    };
  }

  public get sign() {
    return {
      /**
       * Sign a message
       * @param message The message to sign
       * @param rateLimitKey Optional rate limit key
       * @returns SignTextResult
       */
      text: (
        message: string,
        rateLimitKey?: string
      ): Promise<SignTextResult> => {
        return signText(this.id, message, rateLimitKey, this._apiBaseConfig);
      },
      /**
       * Get signed text history
       * @param pagingContext Paging context for pagination
       * @returns PagingResult<SignedTextHistory>
       */
      textHistory: (
          pagingContext: PagingContext = defaultPagingContext
      ): Promise<PagingResult<SignedTextHistory>> => {
        return getSignedTextHistory(this.id, pagingContext, this._apiBaseConfig);
      },
      /**
       * Sign typed data
       * @param typedData The typed data to sign
       * @param rateLimitKey Optional rate limit key
       * @returns SignTypedDataResult
       */
      typedData: (
        typedData: TypedDataDefinition,
        rateLimitKey?: string
      ): Promise<SignTypedDataResult> => {
        return signTypedData(
          this.id,
          typedData,
          rateLimitKey,
          this._apiBaseConfig
        );
      },
      /**
       * Get signed typed data history
       * @param pagingContext Paging context for pagination
       * @returns PagingResult<SignedTypedDataHistory>
       */
      typedDataHistory: (
          pagingContext: PagingContext = defaultPagingContext
      ): Promise<PagingResult<SignedTypedDataHistory>> => {
        return getSignedTypedDataHistory(this.id, pagingContext, this._apiBaseConfig);
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
      /**
       * Get transactions for relayer
       * @returns Transaction[]
       */
      getAll: (
        pagingContext: PagingContext = defaultPagingContext
      ): Promise<PagingResult<Transaction>> => {
        return getTransactions(this.id, pagingContext, this._apiBaseConfig);
      },
      /**
       * Replace a transaction
       * @param transactionId The transaction id
       * @param replacementTransaction The replacement transaction
       * @param rateLimitKey The rate limit key if you want rate limit feature on
       * @returns transactionId
       */
      replace: (
        transactionId: string,
        replacementTransaction: TransactionToSend,
        rateLimitKey?: string | undefined
      ): Promise<TransactionSent> => {
        return replaceTransaction(
          transactionId,
          replacementTransaction,
          rateLimitKey,
          this._apiBaseConfig
        );
      },
      /**
       * cancel a transaction
       * @param transactionId The transaction id
       * @param rateLimitKey The rate limit key if you want rate limit feature on
       * @returns TransactionSent - The cancel transaction details
       */
      cancel: (
        transactionId: string,
        rateLimitKey?: string | undefined
      ): Promise<TransactionSent> => {
        return cancelTransaction(
          transactionId,
          rateLimitKey,
          this._apiBaseConfig
        );
      },
      /**
       * Send a transaction
       * @param transaction The transaction to send
       * @param rateLimitKey The rate limit key if you want rate limit feature on
       * @returns transactionId
       */
      send: (
        transaction: TransactionToSend,
        rateLimitKey?: string | undefined
      ): Promise<TransactionSent> => {
        return sendTransaction(
          this.id,
          transaction,
          rateLimitKey,
          this._apiBaseConfig
        );
      },
      waitForTransactionReceiptById: async (
        transactionId: string
      ): Promise<TransactionReceipt> => {
        while (true) {
          const result = await this.transaction.getStatus(transactionId);
          if (!result) {
            throw new Error('Transaction not found');
          }

          switch (result.status.toUpperCase()) {
            case TransactionStatus.PENDING:
            case TransactionStatus.INMEMPOOL:
              await new Promise((resolve) => setTimeout(resolve, 500));
              break;
            case TransactionStatus.MINED:
            case TransactionStatus.CONFIRMED:
            case TransactionStatus.FAILED:
              invariant(result.receipt, 'Transaction receipt not found');
              return result.receipt;
            case TransactionStatus.EXPIRED:
              throw new Error('Transaction expired');
            case TransactionStatus.CANCELLED:
              throw new Error('Transaction was cancelled');
            case TransactionStatus.REPLACED:
              throw new Error('Transaction was replaced');
            case TransactionStatus.DROPPED:
              throw new Error('Transaction was dropped from mempool');
            default:
              throw new Error(`Unknown transaction status ${result.status}`);
          }
        }
      },
    };
  }

  /**
   * EIP-1193 Ethereum provider which can work with viem/ethers and others
   */
  public ethereumProvider(): Provider {
    return this._ethereumProvider;
  }

  /**
   * Get the viem chain object for this relayer
   * @returns Promise<Chain>
   */
  public async getViemChain(): Promise<Chain> {
    const relayerInfo = await this.getInfo();
    return this.getChainById(relayerInfo.chainId);
  }

  private async getChainById(chainId: number) {
    const chainEntries = Object.values(knownChains);
    const knownChain = chainEntries.find(
      (chain) => 'id' in chain && chain.id === chainId
    );

    if (knownChain) return knownChain;

    // Return default for unknown chains
    return defineChain({
      id: chainId,
      name: `Chain ${chainId}`,
      network: `chain-${chainId}`,
      nativeCurrency: { name: 'Ether', symbol: 'ETH', decimals: 18 },
      rpcUrls: {
        default: { http: [`https://rpc.chain-${chainId}.com`] },
      },
    });
  }
}
