import { createClient, Client } from '../clients';
import { AdminRelayerClient } from '../clients/admin';
import { spawn, ChildProcess } from 'child_process';
import { exec } from 'child_process';
import { promisify } from 'util';
import { Address } from 'viem';

const execAsync = promisify(exec);

// Configuration - set these environment variables or update directly
const config = {
  serverUrl: process.env.SERVER_URL || 'http://localhost:8000',
  providerUrl: process.env.PROVIDER_URL || 'http://localhost:8545',
  chainId: parseInt(process.env.CHAIN_ID || '31337'),
};

export const createBasicAuthClient = (): Client => {
  return createClient({
    serverUrl: 'http://localhost:8000',
    auth: {
      username: 'your_username',
      password: 'your_password',
    },
  });
};

// export const createBasicAuthClient2 = async (): Client => {
//   let blah = createClient({
//     serverUrl: 'http://localhost:8000',
//     auth: {
//       username: 'your_username',
//       password: 'your_password',
//     },
//   });
//
//   const relayer = await blah.getRelayerClient('94afb207-bb47-4392-9229-ba87e4d783cb');
//   blah.getViemHttp()
// };

// export const blah = async () => {
//  let relayer = createRelayerClient(
//      {
//        serverUrl: 'http://localhost:8000',
//        relayerId: '94afb207-bb47-4392-9229-ba87e4d783cb',
//        apiKey: 'YOUR_API_KEY',
//      }
//  );
// }

/**
 * Start Anvil using the make command
 * @returns Promise that resolves when Anvil is started
 */
export const anvilStart = async (
  quiet: boolean = false
): Promise<ChildProcess> => {
  if (!quiet) console.log('🔨 Starting Anvil...');

  try {
    // Use make command to start anvil in the playground/local-node directory
    const anvilProcess = spawn('make', ['start-anvil'], {
      stdio: 'pipe',
      detached: true,
      cwd: '../../playground/local-node', // Navigate to root playground/local-node
    });

    return new Promise((resolve, reject) => {
      let output = '';
      let isStarted = false;

      anvilProcess.stdout?.on('data', (data) => {
        output += data.toString();

        // Only show logs until Anvil is started
        if (!isStarted && !quiet) {
          console.log('Anvil output:', data.toString());
        }

        // Check if Anvil is ready (look for listening message)
        if (
          output.includes('Listening on') ||
          output.includes('127.0.0.1:8545')
        ) {
          if (!quiet) console.log('✅ Anvil started successfully');
          isStarted = true;
          resolve(anvilProcess);
        }
      });

      anvilProcess.stderr?.on('data', (data) => {
        // Only show error logs until Anvil is started
        if (!isStarted && !quiet) {
          console.error('Anvil error:', data.toString());
        }
      });

      anvilProcess.on('error', (error) => {
        console.error('Failed to start Anvil:', error);
        reject(error);
      });

      anvilProcess.on('close', (code) => {
        if (code !== 0) {
          reject(new Error(`Anvil exited with code ${code}`));
        }
      });

      // Timeout after 30 seconds
      setTimeout(() => {
        reject(new Error('Anvil start timeout'));
      }, 30000);
    });
  } catch (error) {
    console.error('Error starting Anvil:', error);
    throw error;
  }
};

/**
 * Start the local RRelayer server using the CLI command
 * @returns Promise that resolves when the server is started
 */
export const startLocalNode = async (
  quiet: boolean = false
): Promise<ChildProcess> => {
  if (!quiet) console.log('🚀 Starting local RRelayer server...');

  try {
    // Run the CLI command to start the local server
    const serverProcess = spawn(
      'cargo',
      ['run', '--', 'start', '--path', '../../playground/local-node'],
      {
        stdio: 'pipe',
        detached: true,
        cwd: '../../crates/cli', // Navigate to cli directory
        env: {
          ...process.env,
          RUST_BACKTRACE: '1',
        },
      }
    );

    return new Promise((resolve, reject) => {
      let output = '';

      serverProcess.stdout?.on('data', (data) => {
        output += data.toString();
        if (!quiet) console.log('Server output:', data.toString());

        // Check if server is ready (look for server start message)
        if (
          output.includes('Server running') ||
          output.includes('listening') ||
          output.includes('8000')
        ) {
          if (!quiet)
            console.log('✅ Local RRelayer server started successfully');
          resolve(serverProcess);
        }
      });

      serverProcess.stderr?.on('data', (data) => {
        const errorMsg = data.toString();
        if (!quiet) console.error('Server error:', errorMsg);

        // Some server messages might come through stderr but aren't errors
        if (
          errorMsg.includes('Server running') ||
          errorMsg.includes('listening') ||
          errorMsg.includes('8000')
        ) {
          if (!quiet)
            console.log('✅ Local RRelayer server started successfully');
          resolve(serverProcess);
        }
      });

      serverProcess.on('error', (error) => {
        console.error('Failed to start local server:', error);
        reject(error);
      });

      serverProcess.on('close', (code) => {
        if (code !== 0) {
          reject(new Error(`Server exited with code ${code}`));
        }
      });

      // Timeout after 60 seconds (server might take longer to start)
      setTimeout(() => {
        reject(new Error('Local server start timeout'));
      }, 60000);
    });
  } catch (error) {
    console.error('Error starting local server:', error);
    throw error;
  }
};

/**
 * Check if RRelayer server is running
 * @returns Promise<boolean> - true if server is running
 */
export const isServerRunning = async (): Promise<boolean> => {
  try {
    const { stdout } = await execAsync(
      'curl -s http://localhost:8000/health || echo "failed"'
    );
    return !stdout.includes('failed') && !stdout.includes('Connection refused');
  } catch {
    return false;
  }
};

/**
 * Wait for RRelayer server to be ready
 * @param maxAttempts - Maximum number of attempts to check (default: 60)
 * @param delayMs - Delay between checks in milliseconds (default: 1000)
 */
export const waitForServer = async (
  maxAttempts: number = 60,
  delayMs: number = 1000,
  quiet: boolean = false
): Promise<void> => {
  for (let i = 0; i < maxAttempts; i++) {
    if (await isServerRunning()) {
      if (!quiet) console.log('✅ RRelayer server is ready');
      return;
    }

    if (!quiet)
      console.log(
        `⏳ Waiting for RRelayer server... (${i + 1}/${maxAttempts})`
      );
    await new Promise((resolve) => setTimeout(resolve, delayMs));
  }

  throw new Error('RRelayer server failed to start within the expected time');
};

/**
 * Stop server process
 * @param serverProcess - The server process to stop
 */
export const stopServer = (
  serverProcess: ChildProcess,
  quiet: boolean = false
): void => {
  if (!quiet) console.log('🛑 Stopping RRelayer server...');

  if (serverProcess) {
    serverProcess.kill('SIGTERM');
    if (!quiet) console.log('✅ RRelayer server stopped');
  }
};

/**
 * Send a transaction with gas using Anvil
 * @param to - Recipient address
 * @param value - Amount in wei
 * @param gasPrice - Gas price in wei
 * @param gasLimit - Gas limit
 * @param data - Transaction data (optional)
 * @returns Transaction hash
 */
export const sendTxWithGas = async (
  to: string,
  value: string = '0',
  gasPrice: string = '1000000000', // 1 gwei
  gasLimit: string = '21000',
  data: string = '0x',
  quiet: boolean = false
): Promise<string> => {
  if (!quiet)
    console.log(`💸 Sending transaction to ${to} with value ${value} wei`);

  try {
    // Build the cast send command - different syntax for transfers vs contract calls
    let command;
    if (data === '0x' || !data) {
      // Simple transfer - no data needed
      command = `FOUNDRY_DISABLE_NIGHTLY_WARNING=1 cast send ${to} --value ${value} --gas-price ${gasPrice} --gas-limit ${gasLimit} --rpc-url http://127.0.0.1:8545 --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80`;
    } else {
      // Contract call - include data
      command = `FOUNDRY_DISABLE_NIGHTLY_WARNING=1 cast send ${to} "${data}" --value ${value} --gas-price ${gasPrice} --gas-limit ${gasLimit} --rpc-url http://127.0.0.1:8545 --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80`;
    }

    const { stdout, stderr } = await execAsync(command);

    // Only treat stderr as error if it contains actual error messages, not warnings
    if (
      stderr &&
      !stderr.includes('Warning:') &&
      !stderr.includes('This is a nightly build')
    ) {
      if (!quiet) console.error('Transaction error:', stderr);
      throw new Error(stderr);
    }

    const txHash = stdout.trim();
    if (!quiet) console.log(`✅ Transaction sent: ${txHash}`);
    return txHash;
  } catch (error) {
    if (!quiet) console.error('Failed to send transaction:', error);
    throw error;
  }
};

/**
 * Create a relayer and fund it using Anvil
 * @param client - The RRelayer client
 * @param chainId - Chain ID to create relayer on
 * @param name - Name for the relayer
 * @param fundingAmount - Amount to fund in ETH (default: "1")
 * @returns Object with relayer info and funding transaction hash
 */
export const createRelayerAndFund = async (
  client: Client,
  chainId: number = 31337,
  name?: string,
  fundingAmount: string = '1',
  quiet: boolean = false
): Promise<{
  relayer: { id: string; address: string };
  fundingTxHash: string;
}> => {
  try {
    // Generate relayer name if not provided
    const relayerName = name || `funded-relayer-${Date.now()}`;

    if (!quiet) console.log(`🔧 Creating relayer: ${relayerName}`);

    // Create the relayer
    const relayer = await client.relayer.create(chainId, relayerName);
    if (!quiet)
      console.log(
        `✅ Created relayer ${relayer.id} at address ${relayer.address}`
      );

    // Fund the relayer using Anvil
    if (!quiet) console.log(`💰 Funding relayer with ${fundingAmount} ETH...`);

    const fundingAmountWei = (parseFloat(fundingAmount) * 1e18).toString();

    const fundingTxHash = await sendTxWithGas(
      relayer.address,
      fundingAmountWei,
      '1000000000', // 1 gwei gas price
      '21000', // standard transfer gas limit
      '0x',
      quiet
    );

    if (!quiet)
      console.log(`✅ Relayer funded with transaction: ${fundingTxHash}`);

    // Wait a bit for the transaction to be mined
    await new Promise((resolve) => setTimeout(resolve, 2000));

    return {
      relayer,
      fundingTxHash,
    };
  } catch (error) {
    if (!quiet) console.error('Failed to create and fund relayer:', error);
    throw error;
  }
};

/**
 * Stop Anvil process
 * @param anvilProcess - The Anvil process to stop
 */
export const anvilStop = (
  anvilProcess: ChildProcess,
  quiet: boolean = false
): void => {
  if (!quiet) console.log('🛑 Stopping Anvil...');

  if (anvilProcess) {
    anvilProcess.kill('SIGTERM');
    if (!quiet) console.log('✅ Anvil stopped');
  }
};

/**
 * Get the default Anvil accounts with their private keys
 * @returns Array of account objects with address and privateKey
 */
export const getAnvilAccounts = (): [
  { address: Address; privateKey: string },
  { address: Address; privateKey: string },
  { address: Address; privateKey: string },
] => {
  return [
    {
      address: '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266',
      privateKey:
        '0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80',
    },
    {
      address: '0x70997970C51812dc3A010C7d01b50e0d17dc79C8',
      privateKey:
        '0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d',
    },
    {
      address: '0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC',
      privateKey:
        '0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a',
    },
    // Add more accounts as needed
  ];
};

/**
 * Check if Anvil is running
 * @returns Promise<boolean> - true if Anvil is running
 */
export const isAnvilRunning = async (): Promise<boolean> => {
  try {
    const { stdout } = await execAsync(
      'curl -s -X POST -H "Content-Type: application/json" --data \'{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}\' http://127.0.0.1:8545'
    );
    return stdout.includes('result');
  } catch {
    return false;
  }
};

/**
 * Wait for Anvil to be ready
 * @param maxAttempts - Maximum number of attempts to check (default: 30)
 * @param delayMs - Delay between checks in milliseconds (default: 1000)
 */
export const waitForAnvil = async (
  maxAttempts: number = 30,
  delayMs: number = 1000,
  quiet: boolean = false
): Promise<void> => {
  for (let i = 0; i < maxAttempts; i++) {
    if (await isAnvilRunning()) {
      if (!quiet) console.log('✅ Anvil is ready');
      return;
    }

    if (!quiet)
      console.log(`⏳ Waiting for Anvil... (${i + 1}/${maxAttempts})`);
    await new Promise((resolve) => setTimeout(resolve, delayMs));
  }

  throw new Error('Anvil failed to start within the expected time');
};

export interface BeginResult {
  relayer: AdminRelayerClient;
  client: Client;
  relayerInfo: { id: string; address: string };
  accounts: ReturnType<typeof getAnvilAccounts>;
  end: () => Promise<void>;
}

export const begin = async (
  fundingAmount: string = '5',
  relayerName?: string,
  quiet: boolean = true
): Promise<BeginResult> => {
  if (!quiet) console.log('🚀 Setting up RRelayer playground...\n');

  let anvilProcess: ChildProcess | null = null;
  let serverProcess: ChildProcess | null = null;

  try {
    // const anvilRunning = await isAnvilRunning();
    // if (!anvilRunning) {
    //   if (!quiet) console.log('🔨 Starting Anvil...');
    //   anvilProcess = await anvilStart(quiet);
    //   await waitForAnvil(30, 1000, quiet);
    // }
    //
    // const serverRunning = await isServerRunning();
    // if (!serverRunning) {
    //   if (!quiet) console.log('🚀 Starting RRelayer server...');
    //   serverProcess = await startLocalNode(quiet);
    //   await waitForServer(60, 1000, quiet);
    // }

    const client = createBasicAuthClient();

    const { relayer: relayerInfo } = await createRelayerAndFund(
      client,
      config.chainId,
      relayerName || `begin-relayer-${Date.now()}`,
      fundingAmount,
      quiet
    );

    const relayer = await client.getRelayerClient(relayerInfo.id);

    const accounts = getAnvilAccounts();

    if (!quiet)
      console.log(
        `✅ Ready! Relayer ${relayerInfo.id} at ${relayerInfo.address}\n`
      );

    const end = async () => {
      try {
        await client.relayer.delete(relayerInfo.id);
      } catch (error) {
        if (!quiet) console.warn('⚠️ Could not delete relayer:', error);
      }

      if (serverProcess) {
        stopServer(serverProcess, quiet);
      }

      if (anvilProcess) {
        anvilStop(anvilProcess, quiet);
      }
    };

    return {
      relayer,
      client,
      relayerInfo,
      accounts,
      end,
    };
  } catch (error) {
    // Cleanup on error
    if (serverProcess) stopServer(serverProcess, quiet);
    if (anvilProcess) anvilStop(anvilProcess, quiet);
    throw error;
  }
};
