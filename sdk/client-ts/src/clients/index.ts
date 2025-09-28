export {
  CreateRelayerResult,
  GasEstimatorResult,
  Network,
  Relayer,
  SignTextResult,
  SignTypedDataResult,
  Transaction,
  TransactionToSend,
} from '../api';
export {
  CreateRelayerClientConfig,
  Client,
  CreateClientConfig,
  createClient,
  createRelayerClient,
} from './core';
export { RelayerClientConfig } from './relayer';
export { TransactionCountType } from './types';

export const createBlobFromString = (message: string): `0x${string}` => {
  const BLOB_SIZE = 131072;
  const blobData = new Uint8Array(BLOB_SIZE);

  const messageBytes = new TextEncoder().encode(message);

  if (messageBytes.length >= BLOB_SIZE) {
    throw new Error(
      `Message too long: ${messageBytes.length} bytes, max: ${BLOB_SIZE - 1}`
    );
  }

  blobData.set(messageBytes, 0);

  return ('0x' +
    Array.from(blobData)
      .map((b) => b.toString(16).padStart(2, '0'))
      .join('')) as `0x${string}`;
};
