import { Address } from 'viem';
import { JwtRole } from '../authentication/types';

export interface User {
  address: Address;
  role: JwtRole;
}
