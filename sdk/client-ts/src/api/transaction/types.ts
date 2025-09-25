import { Address, Hash } from 'viem';
import { GasPriceResult, BlobGasPriceResult } from '../gas';

export enum TransactionStatus {
  PENDING = 'PENDING',
  INMEMPOOL = 'INMEMPOOL',
  MINED = 'MINED',
  CONFIRMED = 'CONFIRMED',
  FAILED = 'FAILED',
  EXPIRED = 'EXPIRED',
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
  to: Address;
  from: Address;
  value: string;
  data: string;
  nonce: string;
  chainId: number;
  gasLimit?: string | null;
  status: TransactionStatus;
  blobs?: any[] | null;
  txHash?: Hash | null;
  queuedAt: string;
  expiresAt: string;
  sentAt?: string | null;
  confirmedAt?: string | null;
  sentWithGas?: GasPriceResult | null;
  sentWithBlobGas?: BlobGasPriceResult | null;
  minedAt?: string | null;
  minedAtBlockNumber?: string | null;
  speed: TransactionSpeed;
  maxPriorityFee?: string | null;
  maxFee?: string | null;
  isNoop: boolean;
  externalId?: string | null;
}

export interface TransactionToSend {
  to: string;
  value?: string | null;
  data?: string | null;
  speed?: TransactionSpeed;
  external_id?: string;
}

export interface TransactionSent {
  id: string;
  hash: Hash;
}
