# rrelayer

## TODO List

31st September plan:
- Last change for the gas bumping + price in bump_every config
- Look into authentication to login and then use it outside rrelayer.yaml
- Reread documentation 
  - TransactionSpeed code snippets tag default
- Get CI builds / releases working
- Testing
    - loading up ALL supported gas providers so it works
    - Testing handling custom gas with http call
- Release—20:00 UK time

# AFTER

- Create CI to run E2E tests on push
- Write go and python SDK (create issues)
- cron job to hit a contract call with parameters every n minutes
- send tx on event firing using rindexer