import { createContext } from 'react';
import { AdminClient } from 'rrelayer-sdk/dist/clients/admin-client';
import { RRelayerClient } from 'rrelayer-sdk/dist/clients/core-client';

export interface RRelayerSDKContextType {
  core: RRelayerClient;
  admin: AdminClient;
}

export const RRelayerSDKContext =
  createContext<RRelayerSDKContextType | null>(null);
