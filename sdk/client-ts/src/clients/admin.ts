import {
  getTransactionsInmempoolCount,
  pauseRelayer,
  unpauseRelayer,
  updateRelayerEIP1559Status,
  updateRelayerMaxGasPrice, removeRelayerMaxGasPrice,
} from '../api';
import {RelayerClient, RelayerClientConfig} from "./relayer";
import {TransactionCountType} from "./types";

export interface AdminRelayerClientConfig {
  serverUrl: string;
  providerUrl: string;
  relayerId: string;
  username: string;
  password: string;
}

export class AdminRelayerClient extends RelayerClient {
  constructor(config: AdminRelayerClientConfig) {
    let relayer: RelayerClientConfig = {
      serverUrl: config.serverUrl,
      providerUrl: config.providerUrl,
      relayerId: config.relayerId,
      auth: {
        username: config.username,
        password: config.password
      }
    }
    super(relayer);
  }

  /**
   * Pause a relayer
   * @returns void
   */
  public pause(): Promise<void> {
    return pauseRelayer(this.id, this._apiBaseConfig);
  }

  /**
   * Unpause a relayer
   * @returns void
   */
  public unpause(): Promise<void> {
    return unpauseRelayer(this.id, this._apiBaseConfig);
  }

  /**
   * Update the EIP1559 status for a relayer
   * @param status The status for the EIP1559
   * @returns void
   */
  public updateEIP1559Status(status: boolean): Promise<void> {
    return updateRelayerEIP1559Status(
        this.id,
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
    return updateRelayerMaxGasPrice(this.id, cap, this._apiBaseConfig);
  }

  /**
   * Remove the max gas price for a relayer
   * @returns void
   */
  public removeMaxGasPrice(): Promise<void> {
    return removeRelayerMaxGasPrice(this.id, this._apiBaseConfig);
  }

  public get transactions() {
    return {
      ...super.transactions,
      /**
       * Get the count of transactions
       * @param type The type of transaction count
       * @returns number
       */
      getCount: (type: TransactionCountType): Promise<number> => {
        switch (type) {
          case TransactionCountType.PENDING:
            return getTransactionsInmempoolCount(
                this.id,
                this._apiBaseConfig
            );
          case TransactionCountType.INMEMPOOL:
            return getTransactionsInmempoolCount(
                this.id,
                this._apiBaseConfig
            );
          default:
            throw new Error('Invalid transaction count type');
        }
      },
    };
  }
}
