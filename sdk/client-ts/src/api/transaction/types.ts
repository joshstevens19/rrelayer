import { Address, Hash } from 'viem';
import { GasPriceResult } from '../gas';

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
  value?: string | null;
  data?: string | null;
  nonce: string;
  status: TransactionStatus;
  knownTransactionHash?: Hash | null;
  queuedAt: string;
  expiresAt: string;
  sentAt?: string | null;
  sentWithGas?: GasPriceResult | null;
  minedAt?: string | null;
  speed: TransactionSpeed;
  sentWithMaxPriorityFeePerGas?: string | null;
  sentWithMaxFeePerGas?: string | null;
}

export interface TransactionToSend {
  to: string;
  value?: string | null;
  data?: string | null;
  speed?: TransactionSpeed;
}

export interface TransactionSent {
  id: string;
  hash: Hash;
}
