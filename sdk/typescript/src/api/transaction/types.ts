import { GasPriceResult, BlobGasPriceResult } from '../network';

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

/**
 * How the queue reacts when a bid for the transaction would exceed its gas price
 * ceiling: 'freeze' never bids above the ceiling (the last compliant bid stays live
 * until it mines or expires), 'cap' clamps the bid at exactly the ceiling.
 */
export type GasPriceCeilingBehavior = 'freeze' | 'cap';

/**
 * An absolute per-transaction gas price ceiling (wei) honored on the initial send
 * and through the gas bump loop. Bounds maxFeePerGas - and legacy gas price, which
 * derives from it; blob gas is not covered.
 */
export interface GasPriceCeiling {
  maxPrice: number;
  /** Defaults to 'freeze' when omitted. */
  behavior?: GasPriceCeilingBehavior;
}

export interface Transaction {
  id: string;
  relayerId: string;
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
  /**
   * Set when the node permanently rejected this transaction's payload and the queue
   * replaced it with a same-nonce no-op; once that no-op mines the transaction
   * resolves to FAILED carrying this reason.
   */
  failedReason?: string | null;
  gasPriceCeiling?: GasPriceCeiling | null;
  /**
   * True once the gas price ceiling actually bound a bid - distinguishes "expired
   * because the ceiling held the price down" from a plain expiry.
   */
  gasPriceCeilingHit: boolean;
}

export interface TransactionToSend {
  to: string;
  value?: string | bigint | null;
  data?: string | null;
  speed?: TransactionSpeed | null;
  blobs?: `0x${string}`[];
  externalId?: string;
  gasPriceCeiling?: GasPriceCeiling;
}

export interface TransactionSent {
  id: string;
  hash: `0x${string}`;
}
