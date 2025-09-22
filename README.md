# rrelayer

// copy from docs

# TODO LIST

The last feature to see how much scope gets added:
  - turnkey signer
  - cron job to hit a contract call with parameters every n minutes
  - send tx on event firing using rindexer

- Testing sending all tx flows with
  - AWS key manager - already manually tested but need e2e
  - GCP key manager - already manually tested but need e2e
  - KMS - not tested
  - Privy - already manually tested but need e2e
- Look over the gas logic again maybe add some max to protect over speeding huge amounts of ETH
- Change the TS sdk to work with the changes in the playground
- AI write go and python SDK
- Finish documentation
- Internal documentation in the README.MD throughout
- Create CI to run E2E tests
- Create CI to build CLI binary (copy from rindexer)
- Publish NPM packages for TS package + Go + Python + Rust