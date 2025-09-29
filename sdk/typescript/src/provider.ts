import {
  PublicClient,
  SendTransactionParameters,
  createPublicClient,
  http,
} from 'viem';
import { RelayerClient } from './clients/relayer';

interface RequestArguments {
  method: any;
  params?: any;
}

export class RpcError extends Error {
  public code: number;
  public data?: unknown;
  public message: string;
  constructor(code: number, message: string, data?: unknown) {
    super(message);
    this.code = code;
    this.data = data;
    this.message = message;
  }
}

export class Provider {
  private _client: PublicClient;
  constructor(
    private _providerUrl: string,
    private _relayer: RelayerClient
  ) {
    this._client = createPublicClient({
      transport: http(this._providerUrl),
    });
  }

  public async request(args: RequestArguments): Promise<unknown> {
    try {
      if (args.method === 'eth_signTransaction') {
        throw new Error(
          'Signing and not sending a transaction is not supporte. Use eth_sendTransaction instead.'
        );
      }

      if (args.method === 'eth_sendRawTransaction') {
        throw new Error(
          'Sending raw transaction is not supported by rrelayer. Use eth_sendTransaction instead.'
        );
      }

      if (args.method === 'eth_sign' || args.method === 'personal_sign') {
        const result = await this._relayer.sign.text(args.params[0]);
        return result.signature;
      }

      if (args.method === 'eth_sendTransaction') {
        const transaction: SendTransactionParameters = args.params[0];
        if (!transaction.to) {
          throw new Error(
            'To address is required to send transactions with a rrelayer'
          );
        }

        const result = await this._relayer.transaction.send({
          to: transaction.to,
          value: transaction.value ? transaction.value.toString() : undefined,
          data: transaction.data,
          speed: this._relayer.speed,
        });

        return result.hash;
      }

      if (args.method === 'eth_signTypedData_v4') {
        const result = await this._relayer.sign.typedData(args.params[1]);
        return result.signature;
      }

      if (
        args.method === 'eth_requestAccounts' ||
        args.method === 'eth_accounts'
      ) {
        const result = await this._relayer.address();
        return [result];
      }

      return await this._client.request(args);
    } catch (error: unknown) {
      // if the error already looks like a rpc error then throw it
      if (error instanceof Error && 'code' in error) {
        throw error;
      }

      throw new RpcError(5000, 'An unknown error occurred');
    }
  }

  /**
   * @deprecated Please use `request` instead.
   */
  public async send(
    method: string,
    params?: unknown[] | object
  ): Promise<unknown> {
    return this.request({ method, params });
  }
}
