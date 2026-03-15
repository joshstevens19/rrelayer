export enum WebhookEventType {
  TransactionQueued = 'transaction_queued',
  TransactionSent = 'transaction_sent',
  TransactionMined = 'transaction_mined',
  TransactionConfirmed = 'transaction_confirmed',
  TransactionFailed = 'transaction_failed',
  TransactionExpired = 'transaction_expired',
  TransactionCancelled = 'transaction_cancelled',
  TransactionReplaced = 'transaction_replaced',
  TextSigned = 'text_signed',
  TypedDataSigned = 'typed_data_signed',
  LowBalance = 'low_balance',
}

export interface WebhookTransactionData {
  id: string;
  relayerId: string;
  to: `0x${string}`;
  from: `0x${string}`;
  value: string;
  data: string;
  chainId: number;
  status: string;
  txHash?: string | null;
  queuedAt: string;
  sentAt?: string | null;
  confirmedAt?: string | null;
  expiresAt: string;
  externalId?: string | null;
  blobs?: string[] | null;
  nonce: string;
  minedAt?: string | null;
  minedAtBlockNumber?: string | null;
  maxPriorityFee?: string | null;
  maxFee?: string | null;
  isNoop: boolean;
}

export interface WebhookPayload {
  eventType: WebhookEventType;
  transaction: WebhookTransactionData;
  timestamp: string;
  apiVersion: string;
  originalTransaction?: WebhookTransactionData | null;
  receipt?: Record<string, unknown> | null;
}

export interface WebhookSigningData {
  relayerId: string;
  chainId: number;
  signature: string;
  signedAt: string;
  message?: string | null;
  domainData?: Record<string, unknown> | null;
  messageData?: Record<string, unknown> | null;
  primaryType?: string | null;
}

export interface WebhookSigningPayload {
  eventType: WebhookEventType;
  signing: WebhookSigningData;
  timestamp: string;
  apiVersion: string;
}

export interface WebhookBalanceAlertData {
  relayerId: string;
  address: `0x${string}`;
  chainId: number;
  currentBalance: string;
  minimumBalance: string;
  currentBalanceFormatted: string;
  minimumBalanceFormatted: string;
  detectedAt: string;
}

export interface WebhookLowBalancePayload {
  eventType: WebhookEventType;
  balanceAlert: WebhookBalanceAlertData;
  timestamp: string;
  apiVersion: string;
}
