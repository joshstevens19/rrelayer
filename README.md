# rrelayerr

// copy from docs

# PROD RUN!!!

RUSTFLAGS="-C target-cpu=native" cargo run --profile maxperf --features jemalloc

## Server MVP

- use proper ethereum typings
- support blob storage transactions
- support logs endpoints to see audit logs of activity with breakdown of transaction
- create clone feature which reuses the same PK 
- create CLI tool to install to run the relayer
- look at the vercel server API example

# AWS env

must be a string

```bash
AWS_ACCESS_KEY_ID: Your access key ID.
AWS_SECRET_ACCESS_KEY: Your secret access key.
AWS_SESSION_TOKEN(optional): Required only if you are using temporary credentials, for example, credentials for an IAM role obtained through AWS STS.
AWS_DEFAULT_REGION(optional): The AWS region where your Secrets Manager secrets are stored. While your code attempts to default to "us-east-1" if no region is found in the environment or configuration, setting this environment variable can provide an explicit default.
```
