import { Address } from 'viem';

export interface Relayer {
  id: string;
  name: string;
  chainId: number;
  address: Address;
  walletIndex: number;
  maxGasPrice?: number;
  paused: boolean;
  eip1559Enabled: boolean;
  createdAt: string;
}
