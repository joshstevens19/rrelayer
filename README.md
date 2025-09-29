# rrelayer

## TODO List

30th September plan:
- Finish documentation
- Do internal documentation throughout the .MD files
- Get install script working locally
- Get CI builds / releases working

31st September plan:
- Testing
    - loading up ALL supported gas providers so it works
    - Testing handling custom gas with http call
- Look into authentication to login and then use it outside rrelayer.yaml
- Publish packages - TS + Rust
- Release—20:00 UK time

# AFTER

- Create CI to run E2E tests on push
- Write go and python SDK (create issues)
- cron job to hit a contract call with parameters every n minutes
- send tx on event firing using rindexer