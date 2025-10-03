# RRelayer E2E Tests

This crate contains end-to-end tests for the rrelayer using Anvil.

## Running

Setup your `.env` the `.env.example` shows you what is needed.

You can run the e2e tests running:

### Raw
```bash
RRELAYER_PROVIDERS="raw" make run-tests-debug
```

### aws_secret_manager
```bash
RRELAYER_PROVIDERS="aws_secret_manager" make run-tests-debug
```

### gcp_secret_manager
```bash
RRELAYER_PROVIDERS="gcp_secret_manager" make run-tests-debug
```

### aws_kms
```bash
RRELAYER_PROVIDERS="aws_kms" make run-tests-debug
```

### privy
```bash
RRELAYER_PROVIDERS="privy" make run-tests-debug
```

### turnkey
```bash
RRELAYER_PROVIDERS="turnkey" make run-tests-debug
```

you can pass in other signer providers into `RRELAYER_PROVIDERS` and run many if you want
aka `RRELAYER_PROVIDERS="raw,aws_secret_manager"`.