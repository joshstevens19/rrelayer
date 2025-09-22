# rrelayer

## The last core feature to see how much scope gets added:
- turnkey signer
- cron job to hit a contract call with parameters every n minutes
- send tx on event firing using rindexer

## TODO List

- Testing sending all tx flows with:
  - AWS key manager - already manually tested but need e2e
  - GCP key manager - already manually tested but need e2e
  - KMS - not tested and needs e2e
  - Privy - already manually tested but need e2e
  - Turnkey - not tested and needs e2e
  - Testing loading up infura gas 
  - Testing loading up tenderly gas
  - Testing handling custom gas with http call
- Look over the gas logic again maybe add some max to protect over speeding huge amounts of ETH incase it gets its wrong
- Change the TS sdk to work with the changes in the playground
- Look over indexes required on all queries
- AI write go and python SDK
- Finish documentation
- Internal documentation in the README.MD throughout
- Create CI to run E2E tests
- Create CI to build CLI binary (copy from rindexer)
- Publish NPM packages for TS package + Go + Python + Rust