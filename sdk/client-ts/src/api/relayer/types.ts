import { Address } from 'viem';

export interface Relayer {
  id: string;
  name: string;
  chainId: number;
  address: Address;
  maxGasPrice?: number;
  paused: boolean;
  allowlistedOnly: boolean;
  eip1559Enabled: boolean;
  createdAt: string;
}
