import asyncio
import subprocess
from datetime import datetime
from time import sleep

from rrelayer.client import createClient

config = {
    "serverUrl": "http://localhost:8000",
    "providerUrl": "http://localhost:8545",
    "chainId": 31337,
}


def createBasicAuthClient():
    return createClient(
        "http://localhost:8000",
        "your_username",
        "your_password",
    )


def anvilStart(quiet: bool = False):
    if not quiet:
        print("🔨 Starting Anvil...")

    try:
        workdir = "../../playground/local-node"
        # Example command — running anvil
        cmd = ["make", "start-anvil"]

        # Launch the process in that directory
        proc = subprocess.Popen(
            cmd,
            cwd=workdir,  # 👈 sets working directory
            stdout=subprocess.PIPE,  # or DEVNULL to suppress
            stderr=subprocess.STDOUT,
            text=True,
        )
        print(f"Started anvil in node with (PID={proc.pid})")
    except Exception as e:
        print("Failed to start anvil node", e)


def startLocalNode(quiet: bool = False):
    if not quiet:
        print("🚀 Starting local RRelayer server....")

    try:
        workdir = "../../crates/cli"
        # Example command — running anvil
        cmd = ["cargo", "run", "--", "start", "--path", "../../playground/local-node"]

        # Launch the process in that directory
        proc = subprocess.Popen(
            cmd,
            cwd=workdir,  # 👈 sets working directory
            stdout=subprocess.PIPE,  # or DEVNULL to suppress
            stderr=subprocess.STDOUT,
            text=True,
        )
        print(proc.stdout)
        print(f"Started rrelayer service with (PID={proc.pid})")

        return proc
    except Exception as e:
        print("Failed to start anvil node", e)


async def createRelayerAndFund(
    client,
    chainId: int = 31337,
    name: str = "",
    fundingAmount: str = "1",
    quiet: bool = True,
):
    relayerName = f"funded-relayer-{datetime.now()}"
    if name:
        relayerName = name

    if not quiet:
        print(f"🔧 Creating relayer: {relayerName}")

    relayer = await client.relayer.create(chainId, relayerName)

    if not quiet:
        print(f"✅ Created relayer {relayer['id']} at address {relayer['address']}")

        print(f"💰 Funding relayer with {fundingAmount} ETH...")

    fundingAmountWei = float(fundingAmount) * 10**18

    sendTxWithGas(relayer["address"], str(fundingAmountWei), quiet=quiet)
    # Wait a bit for the transaction to be mined
    sleep(2)

    return relayer


def sendTxWithGas(
    to: str,
    value: str = "0",
    gasPrice: str = "1000000000",
    gasLimit: str = "21000",
    data: str = "0x",
    quiet: bool = False,
):
    if not quiet:
        print(f"💸 Sending transaction to {to} with value {value} wei")

    try:
        cmd = ""
        if data == "0x" or data is None:
            cmd = f"cast send {to} --value {value} --gas-price {gasPrice} --gas-limit {gasLimit} --rpc-url http://127.0.0.1:8545 --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
        else:
            cmd = f"cast send {to} {data} --value {value} --gas-price {gasPrice} --gas-limit {gasLimit} --rpc-url http://127.0.0.1:8545 --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"

        subprocess.run(cmd.split(" "), check=True)
    except Exception as e:
        print("Failed to send transaction:", e)


def isAnvilRunning():
    try:
        cmd = [
            "curl",
            "-s",
            "-X",
            "POST",
            "-H",
            "Content-Type: application/json",
            "--data",
            '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}',
            "http://127.0.0.1:8545",
        ]
        subprocess.run(cmd, check=True)
        return True
    except Exception as e:
        print("Failed to check Anvil status:", e)
        return False


def isServerRunning() -> bool:
    try:
        cmd = ["curl", "-s", "http://localhost:8000/health"]
        subprocess.run(cmd, check=True)
        return True
    except Exception as e:
        print("Failed to check rrelayer service status:", e)
        return False


def waitForServer(
    maxAttempts: int = 60, delaySeconds: int = 1, quiet: bool = False
) -> bool:
    for i in range(maxAttempts):
        if isServerRunning():
            if not quiet:
                print("✅ RRelayer server is ready")
            return True

        if not quiet:
            print(f"⏳ Waiting for RRelayer server... ({i + 1}/{maxAttempts})")

        sleep(delaySeconds)

    return False


def stopAnvil(quiet: bool = False):
    if not quiet:
        print("🔨 Stopping Anvil...")

    try:
        workdir = "../../playground/local-node"

        cmd = ["make", "stop-anvil"]

        # Launch the process in that directory
        proc = subprocess.Popen(
            cmd,
            cwd=workdir,  # 👈 sets working directory
            stdout=subprocess.PIPE,  # or DEVNULL to suppress
            stderr=subprocess.STDOUT,
            text=True,
        )
        # print(proc.stdout)
        print(f"Stopped anvil in node with (PID={proc.pid})")

        proc.wait()
    except Exception as e:
        print("Failed to stop anvil node", e)


def startDatabaseContainer(quiet: bool = False):
    if not quiet:
        print("🔨 Starting Postgres Container...")

    try:
        workdir = "../../playground/local-node"
        cmd = ["docker-compose", "up"]

        # Launch the process in that directory
        proc = subprocess.Popen(
            cmd,
            cwd=workdir,  # 👈 sets working directory
            stdout=subprocess.DEVNULL,  # or DEVNULL to suppress
            stderr=subprocess.STDOUT,
            text=True,
        )

        # print("Output", proc.stdout)
        # proc.poll()
        print(f"Started database container with (PID={proc.pid})")

    except Exception as e:
        print("Failed to start database container", e)


def isDatabaseContainerRunning() -> bool:
    try:
        cmd = cmd = [
            "pg_isready",
            "-d",
            "postgres",
            "-h",
            "localhost",
            "-p",
            "5471",
            "-U",
            "postgres",
        ]
        subprocess.run(cmd, check=True)
        return True
    except Exception as e:
        print("Failed to check rrelayer service status:", e)
        return False


def waitForDatabaseContainer(
    maxAttempts: int = 60, delaySeconds: int = 1, quiet: bool = False
):
    for i in range(maxAttempts):
        if isDatabaseContainerRunning():
            if not quiet:
                print("✅ Database container is ready")
            return True

        if not quiet:
            print(f"⏳ Waiting for Database container... ({i + 1}/{maxAttempts})")

        sleep(delaySeconds)

    return False


def waitForAnvilNode(maxAttempts: int = 60, delaySeconds: int = 1, quiet: bool = False):
    for i in range(maxAttempts):
        if isAnvilRunning():
            if not quiet:
                print("✅ Anvil node is ready")
            return True

        if not quiet:
            print(f"⏳ Waiting for Anvil node... ({i + 1}/{maxAttempts})")

        sleep(delaySeconds)

    return False


def stopDatabaseContainer(quiet: bool = False):
    if not quiet:
        print("🛑 Stopping Postgres Container...")

    try:
        workdir = "../../playground/local-node"

        cmd = ["docker-compose", "down"]

        # Launch the process in that directory
        proc = subprocess.Popen(
            cmd,
            cwd=workdir,  # 👈 sets working directory
            stdout=subprocess.PIPE,  # or DEVNULL to suppress
            stderr=subprocess.STDOUT,
            text=True,
        )

        print(f"Stopped database container with (PID={proc.pid})")

        proc.wait()
    except Exception as e:
        print("Failed to stop database container", e)


async def begin(
    fundingAmount: str = "5", relayerName: str = "rrelayer", quiet: bool = True
):
    if not quiet:
        print("🚀 Setting up RRelayer playground...")

    try:
        if not isDatabaseContainerRunning():
            startDatabaseContainer(quiet)

        if not waitForDatabaseContainer():
            raise Exception("Database container failed to start")

        anvilRunning = isAnvilRunning()

        if not anvilRunning:
            if not quiet:
                print("🔨 Starting Anvil...")

            anvilStart(quiet)

        if not waitForAnvilNode():
            raise Exception("Anvil failed to start")

        # Start running rrelayer service
        if not isServerRunning():
            rrelayer_node = startLocalNode(quiet)

        waitForServer()

        client = createBasicAuthClient()

        # Create Relayer
        relayerInfo = await createRelayerAndFund(
            client,
            fundingAmount=fundingAmount,
            name=relayerName,
            quiet=quiet,
        )

        print(relayerInfo)

        print("Stopping services...")

        print("Stopping rrelayer node")

        rrelayer_node.terminate()

        stopDatabaseContainer(quiet)

        stopAnvil(quiet)

    except Exception as e:
        print(f"Error setting up RRelayer playground: {e}")


def endChildProcesses(node_process, quiet: bool = True):
    print("Stopping services...")

    node_process.terminate()

    stopDatabaseContainer(quiet)

    stopAnvil(quiet)


if __name__ == "__main__":
    asyncio.run(begin(quiet=False))
