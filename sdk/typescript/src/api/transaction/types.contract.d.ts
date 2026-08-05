import type { TransactionSent } from './types';

type Assert<T extends true> = T;
type Equal<Left, Right> =
  (<Value>() => Value extends Left ? 1 : 2) extends <
    Value,
  >() => Value extends Right ? 1 : 2
    ? true
    : false;

export type PendingTransactionSentContract = Assert<
  { id: string; hash: null } extends TransactionSent ? true : false
>;

export type TransactionSentHashContract = Assert<
  Equal<TransactionSent['hash'], `0x${string}` | null>
>;
