export interface Relayer {
  id: string;
  name: string;
  chainId: number;
  address: `0x${string}`;
  walletIndex: number;
  maxGasPrice?: number;
  paused: boolean;
  eip1559Enabled: boolean;
  createdAt: Date;
  isPrivateKey: boolean;
}
