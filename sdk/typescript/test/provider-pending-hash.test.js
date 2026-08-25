const assert = require('node:assert/strict');
const http = require('node:http');
const test = require('node:test');
const { Provider } = require('../dist/provider');
const { RelayerClient } = require('../dist/clients/relayer');

const TO = '0x0000000000000000000000000000000000000001';
const HASH = `0x${'12'.repeat(32)}`;

test('eth_sendTransaction polls an accepted transaction by ID when hash is pending', async () => {
  let polledId;
  const relayer = {
    fallbackSpeed: undefined,
    transaction: {
      send: async () => ({ id: 'accepted-id', hash: null }),
      waitForTransactionHashById: async (id) => {
        polledId = id;
        return HASH;
      },
    },
  };

  const provider = new Provider('http://127.0.0.1:1', relayer);
  const result = await provider.request({
    method: 'eth_sendTransaction',
    params: [{ to: TO }],
  });

  assert.equal(polledId, 'accepted-id');
  assert.equal(result, HASH);
});

test('eth_sendTransaction returns a known accepted hash without polling', async () => {
  let pollCalled = false;
  const relayer = {
    fallbackSpeed: undefined,
    transaction: {
      send: async () => ({ id: 'known-id', hash: HASH }),
      waitForTransactionHashById: async () => {
        pollCalled = true;
        return HASH;
      },
    },
  };

  const provider = new Provider('http://127.0.0.1:1', relayer);
  const result = await provider.request({
    method: 'eth_sendTransaction',
    params: [{ to: TO }],
  });

  assert.equal(result, HASH);
  assert.equal(pollCalled, false);
});

test('relayer client polls the transaction read route until a hash is durable', async () => {
  let reads = 0;
  const server = http.createServer((request, response) => {
    assert.equal(request.url, '/transactions/accepted-id');
    reads += 1;
    response.setHeader('content-type', 'application/json');
    response.end(
      JSON.stringify({
        id: 'accepted-id',
        status: reads === 1 ? 'PENDING' : 'INMEMPOOL',
        txHash: reads === 1 ? null : HASH,
      })
    );
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));

  try {
    const address = server.address();
    assert.notEqual(typeof address, 'string');
    const client = new RelayerClient({
      serverUrl: `http://127.0.0.1:${address.port}`,
      providerUrl: 'http://127.0.0.1:1',
      relayerId: 'relayer-id',
      auth: { username: 'test', password: 'test' },
    });

    const result = await client.transaction.waitForTransactionHashById(
      'accepted-id',
      1
    );
    assert.equal(result, HASH);
    assert.equal(reads, 2);
    await assert.rejects(
      client.transaction.waitForTransactionHashById('accepted-id', 0, 2),
      /tryEveryMs must be a positive integer/
    );
  } finally {
    await new Promise((resolve, reject) =>
      server.close((error) => (error ? reject(error) : resolve()))
    );
  }
});

test('relayer client stops polling after the configured attempt bound', async () => {
  let reads = 0;
  const server = http.createServer((_request, response) => {
    reads += 1;
    response.setHeader('content-type', 'application/json');
    response.end(
      JSON.stringify({
        id: 'never-hashed',
        status: 'PENDING',
        txHash: null,
      })
    );
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));

  try {
    const address = server.address();
    assert.notEqual(typeof address, 'string');
    const client = new RelayerClient({
      serverUrl: `http://127.0.0.1:${address.port}`,
      providerUrl: 'http://127.0.0.1:1',
      relayerId: 'relayer-id',
      auth: { username: 'test', password: 'test' },
    });

    await assert.rejects(
      client.transaction.waitForTransactionHashById('never-hashed', 1, 2),
      /Timed out waiting for transaction never-hashed hash after 2 attempts/
    );
    assert.equal(reads, 2);
  } finally {
    await new Promise((resolve, reject) =>
      server.close((error) => (error ? reject(error) : resolve()))
    );
  }
});
