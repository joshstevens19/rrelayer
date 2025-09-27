# rrelayer

## The last core feature to see how much scope gets added:
- cron job to hit a contract call with parameters every n minutes
- send tx on event firing using rindexer

## TODO List

- Finish documentation
- AI write go and python SDK
- Internal documentation in the README.MD throughout
- Create CI to run E2E tests
- Create CI to build CLI binary (copy from rindexer)
- Publish NPM packages for TS package + Go + Python + Rust

## BUG

- Look at if you run out of funds inmempool to bump and what it should do.. just keep retrying or?
- Testing loading up ALL supported gas providers so it works
- Testing handling custom gas with http call
- rrelayer auth without being in project
- look at signers in the networks

