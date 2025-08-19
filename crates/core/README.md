# reelayer_server

## Docker

### Server

docker build -f docker/server/Dockerfile -t rrelayer_server .

## Unit tests

### Coverage

install:

`cargo install cargo-tarpaulin`

run tests coverage:

`cargo tarpaulin`

to generate html report

`cargo tarpaulin --out Html`
