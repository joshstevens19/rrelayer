# rrelayer

## The last core feature to see how much scope gets added:
- cron job to hit a contract call with parameters every n minutes
- send tx on event firing using rindexer

## TODO List

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

# Pawel feedback

- do rrelayer login and then throw on methods which can not be used outside the server


# Idea for later

- safe signers and submitters logic + look at account abstracted accounts
- ability to put your signing provider in other places like auto-top-up
- more complex allowlist aka methods etc
