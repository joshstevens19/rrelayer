import {
  anvilStart,
  anvilStop,
  createBasicAuthClient,
  createRelayerAndFund,
  getAnvilAccounts,
  isAnvilRunning,
  isServerRunning,
  sendTxWithGas,
  startLocalNode,
  stopServer,
  waitForAnvil,
  waitForServer
} from './helpers';
import {ChildProcess} from 'child_process';
import {AdminRelayerClient} from "../clients/admin";
import {TransactionCountType} from "../clients";

// Configuration - set these environment variables or update directly
const config = {
  serverUrl: process.env.SERVER_URL || 'http://127.0.0.1:8000',
  providerUrl: process.env.PROVIDER_URL || 'http://127.0.0.1:8545',
  chainId: parseInt(process.env.CHAIN_ID || '31337'),
};

async function playground() {
  console.log('🚀 RRelayer SDK Playground with Local Node');
  console.log('==========================================\n');

  let anvilProcess: ChildProcess | null = null;
  let serverProcess: ChildProcess | null = null;

  try {
    // Step 1: Start Anvil if not running
    console.log('Step 1: Setting up Anvil...');
    const anvilRunning = await isAnvilRunning();
    
    if (!anvilRunning) {
      console.log('🔨 Anvil not running, starting it...');
      anvilProcess = await anvilStart();
      await waitForAnvil();
    } else {
      console.log('✅ Anvil is already running');
    }

    // Step 2: Start RRelayer local server if not running
    console.log('\nStep 2: Setting up RRelayer server...');
    const serverRunning = await isServerRunning();
    
    if (!serverRunning) {
      console.log('🚀 RRelayer server not running, starting it...');
      serverProcess = await startLocalNode();
      await waitForServer();
    } else {
      console.log('✅ RRelayer server is already running');
    }

    // Show available Anvil accounts
    console.log('\n💳 Available Anvil accounts:');
    const accounts = getAnvilAccounts();
    accounts.slice(0, 3).forEach((account, index) => {
      console.log(`  ${index + 1}. ${account.address}`);
    });

    // Step 3: Create a client for admin operations
    console.log('\nStep 3: Setting up SDK client...');
    const client = createBasicAuthClient();
    console.log('✅ Created admin client');

    // Get all networks
    console.log('\n📡 Getting supported networks...');
    const networks = await client.network.getAll();
    console.log(`Found ${networks.length} networks:`, networks.map(n => `${n.name} (${n.chainId})`));

    // Create and fund a relayer
    console.log('\n🏦 Creating and funding a new relayer...');
    const { relayer, fundingTxHash } = await createRelayerAndFund(
      client,
      config.chainId,
      `playground-relayer-${Date.now()}`,
      '5' // Fund with 5 ETH
    );

    console.log(`Created relayer: ${relayer.id}`);
    console.log(`Relayer address: ${relayer.address}`);
    console.log(`Funding transaction: ${fundingTxHash}`);

    // Get relayer info
    console.log('\n📋 Getting relayer info...');
    const relayerInfo = await client.relayer.get(relayer.id);
    console.log('Relayer info:', relayerInfo?.relayer);

    // Create admin relayer client
    console.log('\n👑 Creating admin relayer client...');
    const adminClient = new AdminRelayerClient({
      serverUrl: config.serverUrl,
      providerUrl: config.providerUrl,
      relayerId: relayer.id,
      auth: {
        username: 'your_username',
        password: 'your_password',
      },
    });

    // Get relayer address and balance
    console.log('\n💰 Getting relayer address and balance...');
    const address = await adminClient.address();
    const balance = await adminClient.getBalanceOf();
    console.log(`Address: ${address}`);
    console.log(`Balance: ${balance} ETH`);

    // Test signing
    console.log('\n✍️ Testing message signing...');
    const message = `Hello from playground at ${new Date().toISOString()}`;
    const signature = await adminClient.sign.text(message);
    console.log(`Message: "${message}"`);
    console.log(`Signature: ${signature.signature}`);

    // Test admin operations
    console.log('\n⚙️ Testing admin operations...');
    console.log('Updating EIP1559 status to true...');
    await adminClient.updateEIP1559Status(true);
    console.log('✅ EIP1559 status updated');

    console.log('Setting max gas price to 2 gwei...');
    await adminClient.updateMaxGasPrice('2000000000');
    console.log('✅ Max gas price set');

    // Get transaction counts
    console.log('\n📊 Getting transaction counts...');
    const pendingCount = await adminClient.transaction.getCount(TransactionCountType.PENDING);
    const inmempoolCount = await adminClient.transaction.getCount(TransactionCountType.INMEMPOOL);
    console.log(`Pending transactions: ${pendingCount}`);
    console.log(`In mempool transactions: ${inmempoolCount}`);

    // Send a test transaction using Anvil
    console.log('\n🔄 Sending test transaction with custom gas...');
    const testTxHash = await sendTxWithGas(
      accounts[1].address, // Send to second account
      '1000000000000000000', // 1 ETH in wei
      '2000000000', // 2 gwei gas price
      '21000' // Standard gas limit
    );
    console.log(`Test transaction sent: ${testTxHash}`);

    // Get all transactions
    console.log('\n📜 Getting all transactions for relayer...');
    const transactions = await adminClient.transaction.getAll();
    console.log(`Found ${transactions.items.length} transactions`);

    // Test pause/unpause functionality
    console.log('\n⏸️ Testing pause/unpause functionality...');
    await adminClient.pause();
    console.log('✅ Relayer paused');
    
    await adminClient.unpause();
    console.log('✅ Relayer unpaused');

    // Clean up - delete the test relayer
    console.log('\n🗑️ Cleaning up test relayer...');
    await client.relayer.delete(relayer.id);
    console.log('✅ Test relayer deleted');

    console.log('\n🎉 Playground completed successfully!');

  } catch (error) {
    console.error('❌ Error in playground:', error);
    
    if (error instanceof Error) {
      console.error('Error message:', error.message);
      
      // Check for common configuration issues
      if (error.message.includes('ECONNREFUSED') || error.message.includes('fetch')) {
        console.log('\n💡 Tip: Make sure your RRelayer server is running and accessible at:', config.serverUrl);
        console.log('💡 Tip: Make sure Anvil is running at:', config.providerUrl);
      } else if (error.message.includes('auth') || error.message.includes('401')) {
        console.log('\n💡 Tip: Check your username and password credentials in helpers.ts');
      } else if (error.message.includes('make')) {
        console.log('\n💡 Tip: Make sure you have the make command available and the Makefile with anvil-start target');
      }
    }
  } finally {
    // Clean up processes if we started them
    if (serverProcess) {
      console.log('\n🛑 Stopping RRelayer server...');
      stopServer(serverProcess);
    }
    
    if (anvilProcess) {
      console.log('🛑 Stopping Anvil...');
      anvilStop(anvilProcess);
    }
  }
}

// Run the playground
playground().catch(console.error);