# rrelayer

## The last core feature to see how much scope gets added:
- cron job to hit a contract call with parameters every n minutes
- send tx on event firing using rindexer

## TODO List

- Testing sending all tx flows with:
  - KMS - tested and needs e2e
- Change the TS sdk to work with the changes in the playground
- Finish documentation
- AI write go and python SDK
- Internal documentation in the README.MD throughout
- Create CI to run E2E tests
- Create CI to build CLI binary (copy from rindexer)
- Publish NPM packages for TS package + Go + Python + Rust

## BUG

- Look over the gas logic again maybe add some max to protect over speeding huge amounts of ETH incase it gets its wrong (also legacy and EIP-1559)
- Testing loading up infura gas
- Testing loading up tenderly gas
- Testing handling custom gas with http call