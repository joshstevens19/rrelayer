import { GasPriceResult, BlobGasPriceResult } from '../network';

export interface TransactionAuthorization {
  address: `0x${string}`;
  chainId: number;
  nonce: number;
  r: string | bigint;
  s: string | bigint;
  yParity: number;
}

export enum TransactionStatus {
  PENDING = 'PENDING',
  INMEMPOOL = 'INMEMPOOL',
  MINED = 'MINED',
  CONFIRMED = 'CONFIRMED',
  FAILED = 'FAILED',
  EXPIRED = 'EXPIRED',
  CANCELLED = 'CANCELLED',
  REPLACED = 'REPLACED',
  DROPPED = 'DROPPED',
}

export enum TransactionSpeed {
  SLOW = 'SLOW',
  MEDIUM = 'MEDIUM',
  FAST = 'FAST',
  SUPER = 'SUPER',
}

export interface Transaction {
  id: string;
  relayerId: string;
  authorizationList?: TransactionAuthorization[];
  to: `0x${string}`;
  from: `0x${string}`;
  value: string;
  data: string;
  nonce: string;
  chainId: number;
  gasLimit?: string | null;
  status: TransactionStatus;
  blobs?: any[] | null;
  txHash?: `0x${string}` | null;
  queuedAt: Date;
  expiresAt: Date;
  sentAt?: string | null;
  confirmedAt?: string | null;
  sentWithGas?: GasPriceResult | null;
  sentWithBlobGas?: BlobGasPriceResult | null;
  minedAt?: Date | null;
  minedAtBlockNumber?: string | null;
  speed: TransactionSpeed;
  maxPriorityFee?: string | null;
  maxFee?: string | null;
  isNoop: boolean;
  externalId?: string | null;
  cancelledByTransactionId?: string | null;
}

export interface TransactionToSend {
  authorizationList?: TransactionAuthorization[];
  to: string;
  value?: string | bigint | null;
  data?: string | null;
  speed?: TransactionSpeed | null;
  blobs?: `0x${string}`[];
  externalId?: string;
}

export interface TransactionSent {
  id: string;
  hash: `0x${string}`;
}
