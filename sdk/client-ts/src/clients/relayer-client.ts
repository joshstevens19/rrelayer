import {
  Address,
  TransactionReceipt,
  TypedDataDefinition,
  formatEther,
} from 'viem';
import {
  Relayer,
  SignTextResult,
  SignTypedDataResult,
  Transaction,
  TransactionSent,
  TransactionStatus,
  TransactionStatusResult,
  TransactionToSend,
  addRelayerAllowlistAddress,
  cancelTransaction,
  deleteRelayerAllowlistAddress,
  getRelayer,
  getRelayerAllowlistAddress,
  getTransaction,
  getTransactionStatus,
  getTransactions,
  getTransactionsInmempoolCount,
  pauseRelayer,
  replaceTransaction,
  sendTransaction,
  signText,
  signTypedData,
  unpauseRelayer,
  updateRelayerEIP1559Status,
  updateRelayerMaxGasPrice,
} from '../api';
import {
  ApiBaseConfig,
  PagingContext,
  PagingResult,
  defaultPagingContext,
} from '../api/types';
import { RRelayerrEthereumProvider } from '../rrelayer-ethereum-provider';
import { invariant } from '../utils';
import { RRelayerrConfig } from './core-client';

export interface RelayerClientConfig {
  providerUrl: string;
  relayerId: string;
  auth:
    | {
        apiKey: string;
      }
    | {
        authToken: string;
      };
}

export enum TransactionCountType {
  PENDING = 'PENDING',
  INMEMPOOL = 'INMEMPOOL',
}

/**
 * The relayer client to interact with the relayer
 */
export class RelayerClient {
  private _apiBaseConfig: ApiBaseConfig;
  private _relayerId: string;
  private _ethereumProvider: RRelayerrEthereumProvider;
  constructor(relayerConfig: RelayerClientConfig, coreConfig: RRelayerrConfig) {
    this._relayerId = relayerConfig.relayerId;
    this._ethereumProvider = new RRelayerrEthereumProvider(
      relayerConfig.providerUrl,
      this
    );
    if ('authToken' in relayerConfig.auth) {
      this._apiBaseConfig = {
        serverUrl: coreConfig.serverUrl,
        authToken: relayerConfig.auth.authToken,
      };
    } else {
      this._apiBaseConfig = {
        serverUrl: coreConfig.serverUrl,
        apiKey: relayerConfig.auth.apiKey,
      };
    }
  }

  /**
   * Get the relayer id
   * @returns string
   */
  public id(): string {
    return this._relayerId;
  }

  /**
   * Get the relayer address
   * @returns string
   */
  public async address(): Promise<Address> {
    return (await this.info()).address;
  }

  /**
   * Get the relayer chain id
   * @returns string
   */
  public async chainId(): Promise<number> {
    return (await this.info()).chainId;
  }

  /**
   * Get the relayer paused status
   * @returns boolean
   */
  public async paused(): Promise<boolean> {
    return (await this.info()).paused;
  }

  /**
   * Get the relayer max gas price
   * @returns boolean
   */
  public async maxGasPrice(): Promise<number | undefined> {
    return (await this.info()).maxGasPrice;
  }

  /**
   * Get is the relayer is allowlisted only
   * @returns boolean
   */
  public async allowlistedOnly(): Promise<boolean> {
    return (await this.info()).allowlistedOnly;
  }

  /**
   * Get is the relayer has EIP1559 enabled
   * @returns boolean
   */
  public async eip1559Enabled(): Promise<boolean> {
    return (await this.info()).eip1559Enabled;
  }

  /**
   * Get the relayer information
   * @returns Relayer
   */
  public async info(): Promise<Relayer> {
    const result = await getRelayer(this._relayerId, this._apiBaseConfig);

    invariant(result, 'Relayer not found');

    return result.relayer;
  }

  /**
   * Get the relayer balance
   * @returns string
   */
  public async balanceOf(): Promise<string> {
    // @ts-ignore - use the viem getBalance method without exposing the client (which is not needed here)
    const balance = await this._ethereumProvider._client.getBalance({
      address: await this.address(),
    });

    return formatEther(balance);
  }

  /**
   * Pause a relayer
   * @returns void
   */
  public pause(): Promise<void> {
    return pauseRelayer(this._relayerId, this._apiBaseConfig);
  }

  /**
   * Unpause a relayer
   * @returns void
   */
  public unpause(): Promise<void> {
    return unpauseRelayer(this._relayerId, this._apiBaseConfig);
  }

  /**
   * Update the EIP1559 status for a relayer
   * @param status The status for the EIP1559
   * @returns void
   */
  public updateEIP1559Status(status: boolean): Promise<void> {
    return updateRelayerEIP1559Status(
      this._relayerId,
      status,
      this._apiBaseConfig
    );
  }

  /**
   * Update the max gas price for a relayer
   * @param cap The cap for the max gas price
   * @returns void
   */
  public updateMaxGasPrice(cap: string): Promise<void> {
    return updateRelayerMaxGasPrice(this._relayerId, cap, this._apiBaseConfig);
  }

  public get allowlist() {
    return {
      /**
       * Add an address to the relayer allowlist
       * @param address The address to add to the allowlist
       * @returns void
       */
      add: (address: string): Promise<void> => {
        return addRelayerAllowlistAddress(
          this._relayerId,
          address,
          this._apiBaseConfig
        );
      },
      /**
       * Delete an address from the relayer allowlist
       * @param address The address to delete from the allowlist
       * @returns void
       */
      delete: (address: string): Promise<void> => {
        return deleteRelayerAllowlistAddress(
          this._relayerId,
          address,
          this._apiBaseConfig
        );
      },

      /**
       * Get the relayer allowlist
       * @returns An address of allowlist addresses
       */
      get: (
        pagingContext: PagingContext = defaultPagingContext
      ): Promise<PagingResult<string>> => {
        return getRelayerAllowlistAddress(
          this._relayerId,
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
       * @returns SignTextResult
       */
      text: (message: string): Promise<SignTextResult> => {
        return signText(this._relayerId, message, this._apiBaseConfig);
      },
      /**
       * Sign typed data
       * @param typedData The typed data to sign
       * @returns SignTypedDataResult
       */
      typedData: (
        typedData: TypedDataDefinition
      ): Promise<SignTypedDataResult> => {
        return signTypedData(this._relayerId, typedData, this._apiBaseConfig);
      },
    };
  }

  public get transactions() {
    return {
      /**
       * Get a transaction
       * @param id The transaction id
       * @returns Transaction | null
       */
      getTransaction: (transactionId: string): Promise<Transaction | null> => {
        return getTransaction(transactionId, this._apiBaseConfig);
      },
      /**
       * Get a transaction status
       * @param id The transaction id
       * @returns TransactionStatusResult | null
       */
      getTransactionStatus: (
        transactionId: string
      ): Promise<TransactionStatusResult | null> => {
        return getTransactionStatus(transactionId, this._apiBaseConfig);
      },
      /**
       * Get transactions for relayer
       * @returns Transaction[]
       */
      getTransactions: (
        pagingContext: PagingContext = defaultPagingContext
      ): Promise<PagingResult<Transaction>> => {
        return getTransactions(
          this._relayerId,
          pagingContext,
          this._apiBaseConfig
        );
      },
      /**
       *  Get the count of transactions
       * @param type The type of transaction count
       * @returns number
       */
      getCount: (type: TransactionCountType): Promise<number> => {
        switch (type) {
          case TransactionCountType.PENDING:
            return getTransactionsInmempoolCount(
              this._relayerId,
              this._apiBaseConfig
            );
          case TransactionCountType.INMEMPOOL:
            return getTransactionsInmempoolCount(
              this._relayerId,
              this._apiBaseConfig
            );
          default:
            throw new Error('Invalid transaction count type');
        }
      },
      /**
       * Replace a transaction
       * @param transactionId The transaction id
       * @param replacementTransaction The replacement transaction
       * @returns transactionId
       */
      replace: (
        transactionId: string,
        replacementTransaction: TransactionToSend
      ): Promise<TransactionSent> => {
        return replaceTransaction(
          transactionId,
          replacementTransaction,
          this._apiBaseConfig
        );
      },
      /**
       * cancel a transaction
       * @param transactionId The transaction id
       * @returns boolean
       */
      cancel: (transactionId: string): Promise<boolean> => {
        return cancelTransaction(transactionId, this._apiBaseConfig);
      },
      /**
       * Send a transaction
       * @param replacementTransaction The replacement transaction
       * @returns transactionId
       */
      send: (
        replacementTransaction: TransactionToSend
      ): Promise<TransactionSent> => {
        return sendTransaction(
          this._relayerId,
          replacementTransaction,
          this._apiBaseConfig
        );
      },
      waitForTransactionReceiptById: async (
        transactionId: string
      ): Promise<TransactionReceipt> => {
        while (true) {
          const result = await this.transactions.getTransactionStatus(
            transactionId
          );
          if (!result) {
            throw new Error('Transaction not found');
          }

          switch (result.status) {
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
            default:
              throw new Error('Unknown transaction status');
          }
        }
      },
    };
  }

  public ethereumProvider(): RRelayerrEthereumProvider {
    return this._ethereumProvider;
  }
}
