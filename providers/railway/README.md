# Railway

## Deploy an example project

<https://github.com/joshstevens19/rrelayer/tree/master/providers/railway>

1. Clone the relevant directory

  ```bash
  # this will clone the railway directory
  mkdir rrelayer-railway && cd rrelayer-railway
  git clone \
    --depth=1 \
    --no-checkout \
    --filter=tree:0 \
    https://github.com/joshstevens19/rrelayer .
  git sparse-checkout set --no-cone providers/railway .
  git checkout && cp -r providers/railway/* . && rm -rf providers
  ```

2. Initialize a new Railway project

  Install [Railway CLI](https://docs.railway.com/guides/cli) if not already installed.

  ```bash
  railway login
  ```

  ```bash
  railway init --name rrelayer-example
  ```

3. Create a service and link it to the project

  ```bash
  railway up --detach
  railway link --name rrelayer-example --environment production
  ```

4. Create a Postgres database

  ```bash
  railway add --database postgres
  ```

5. Configure environment variables

  ```bash
  railway open
  ```

  - Open the service "Variables" tab:

    - Select "Add Variable Reference" and add a reference for `DATABASE_URL` and append ?sslmode=disable to the end of the value. The result should look like `${{Postgres.DATABASE_URL}}?sslmode=disable`.

    - Select "Add Variable Reference" and add a reference for `POSTGRES_PASSWORD`.

    - Select "New Variable" with name `RRELAYER_AUTH_USERNAME` and value `admin` (or your chosen username).

    - Select "New Variable" with name `RRELAYER_AUTH_PASSWORD` and set a strong password.

    - Select "New Variable" with name `RAW_DANGEROUS_MNEMONIC` and paste a development mnemonic (use a secure signing provider in production).

    - Select "New Variable" with name `PORT` and value `3000`.

  - Hit "Deploy" or press Shift+Enter.

6. Create a domain to access the API

  ```bash
  railway domain
  ```

7. Redeploy the service

  ```bash
  railway up
  ```
