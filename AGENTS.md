# AGENTS.md — working on rrelayer

Guidance for coding agents (and humans) working in this repo. Everything here is
grounded in the actual code/CI as of v0.12.1 — when in doubt, the referenced file
is the source of truth.

rrelayer is an EVM transaction relay service written in Rust: an axum REST API +
an in-process transaction engine (queues, nonce management, gas bumping) backed by
Postgres, with pluggable signing providers (raw mnemonic, AWS KMS, AWS/GCP Secrets
Manager, Privy, Turnkey, Fireblocks, PKCS#11, private keys). Clients are a CLI, a
Rust SDK, and a TypeScript SDK.

---

## 1. Non-negotiable rules

1. **Every user-visible change MUST add a changelog entry** in
   `documentation/rrelayer/docs/pages/changelog.mdx` under `## Changes Not Deployed`,
   in the same commit/PR as the change. "User-visible" = anything a user of the
   server, CLI, SDKs, config file, or docker image could notice: features, bug
   fixes, behavior changes, config options, API/SDK surface changes. Pure
   refactors, CI tweaks, and doc typo fixes are exempt. Format is strict — see
   section 4 before touching the file.
2. **Pass the exact CI gates locally before pushing** (CI = `.github/workflows/ci.yml`):
   ```bash
   cargo fmt --all                                            # fix, then:
   cargo fmt --all -- --check
   cargo clippy -- -D warnings -A clippy::uninlined_format_args
   cargo test --exclude rust-sdk-playground --workspace
   ```
   Clippy denies ALL warnings (only `uninlined_format_args` is allowed). rustfmt
   uses `rustfmt.toml` (`use_small_heuristics = "Max"`, `reorder_imports`,
   `use_field_init_shorthand`) — IDE-default formatting will fail the check; always
   run `cargo fmt --all` from the repo root.
3. **Never put the string `Release v` (or `release/`) in a commit message or PR
   title.** CI greps master HEAD for `Release v[0-9]*\.[0-9]*\.[0-9]*` to trigger
   the release pipeline, and skips normal builds on messages containing
   `Release v`/`release/`. Branch names `release/*` are reserved for releases.
4. **The HTTP API has three hand-synced consumers.** Any change to a route path,
   request/response shape, or a serde rename in `crates/core` must be mirrored in
   the Rust SDK (`crates/sdk`), the TypeScript SDK (`sdk/typescript`), and the docs
   (`documentation/rrelayer/docs/pages/integration/`). There are no shared
   constants or codegen — grep is your safety net.
5. **New API handlers must do their own auth.** There is no route-level auth
   middleware; a handler that skips the `AppState` auth helpers is publicly
   reachable. There are two, and picking the wrong one is a security bug — see
   the endpoint recipe in section 6.

---

## 2. Repo map

| Path | What it is |
|---|---|
| `crates/core` | The server (`rrelayer_core`): axum API + transaction engine + DB + signing providers. Almost all behavior lives here. |
| `crates/cli` | The CLI (`rrelayer_cli` — the shipped binary; installers rename it to `rrelayer`). `rrelayer start` boots the core server in-process. |
| `crates/sdk` | The **Rust SDK**, published to crates.io as `rrelayer`. Depends on `rrelayer_core` for types. |
| `sdk/rust` | README pointer only — the Rust SDK is `crates/sdk`. Never add code here. |
| `sdk/typescript` | The **TypeScript SDK**, published to npm as `rrelayer` (versioned independently: npm 1.2.0 vs crates 0.12.1). |
| `crates/e2e-tests` | The real test suite: a standalone `e2e-runner` binary (~49 tests) that orchestrates anvil + Postgres + an embedded server. **Not run in CI.** |
| `playground/rust-sdk-playground` | Workspace member with compile-checked SDK usage snippets + runnable examples. Excluded from `cargo test` in CI, but it must still build and pass clippy. |
| `playground/*` (example, local-node, base, sepolia, …) | rrelayer *project fixtures* (`rrelayer.yaml` + docker-compose) used to run the server locally. `playground/base` is a TS consumer app using the *published* npm SDK. |
| `documentation/rrelayer` | Vocs docs site (npm). Contains **the changelog** and all user docs. CI never builds it — verify with `npm run build`. |
| `helm/rrelayer`, `providers/railway`, `Dockerfile`, `docker-compose.yml` | Deployment artifacts. Root docker-compose is dev Postgres only (port 5446). |
| `scripts/publish-rrelayer.sh` | Manual, interactive crates.io publish. Never run casually. |

Crate/package name traps: directory `crates/sdk` = crate `rrelayer`; `crates/core`
= `rrelayer_core`; `crates/cli` = `rrelayer_cli` (not on crates.io — ships as
GitHub Release binaries). Both SDKs are named `rrelayer` on their registries.

---

## 3. Commands

Rust (repo root unless noted):

```bash
cargo build                        # build the whole workspace
cargo build -p rrelayer_core       # just the server crate
cargo fmt --all -- --check         # CI fmt gate
cargo clippy -- -D warnings -A clippy::uninlined_format_args   # CI lint gate
cargo test --exclude rust-sdk-playground --workspace           # CI test gate (CI adds --release --target <triple>)
```

Run the server locally (needs Docker; see section 8):

```bash
cp .env.example .env               # once
docker-compose up -d               # Postgres 16 on host port 5446
cd crates/cli && make start_local  # = cargo run -- start --path ../../playground/local-node
# other fixtures: make start_example, start_e2e; manual CLI tests: make tx_send, network_list, ...
```

E2E tests (from `crates/e2e-tests`; needs Docker + Foundry's `anvil` and `forge`
on PATH + a `crates/e2e-tests/.env` copied from its `.env.example`):

```bash
make run-tests-debug                        # full suite, raw provider, RUST_LOG=info
RRELAYER_PROVIDERS="raw" make run-tests-debug   # explicit provider(s), comma-separated
make run-test TEST=transaction_send_eth     # one test, by exact registered name
```

TypeScript SDK (from `sdk/typescript`; npm, not yarn/pnpm):

```bash
npm install
npm run build          # tsc — the ONLY typecheck gate; CI never builds the TS SDK
npm run format         # prettier (.prettierrc: single quotes, 80 cols); no ESLint
npm run "playground::transaction::send"   # manual smoke scripts; need a live local server
```

Docs site (from `documentation/rrelayer`):

```bash
npm i && npm run dev   # local dev server
npm run build          # the only way to catch broken MDX/nav — CI never builds docs
npm run format         # prettier; deliberately skips changelog.mdx via .prettierignore
```

---

## 4. The changelog contract

File: `documentation/rrelayer/docs/pages/changelog.mdx`. It is hand-maintained
(the CI automation for it is commented out in `ci.yml` — do **not** resurrect or
imitate that script; its binary filenames are outdated). It is also excluded from
prettier via `documentation/rrelayer/.prettierignore` — never reformat it.

### 4.1 Adding an entry with your change (the normal case)

Line 1 of the file is a `# Changelog` H1 — leave it alone. Immediately below it
is a fixed skeleton:

```md
## Changes Not Deployed

---

### Features

---

### Bug fixes

---

### Breaking changes

---
```

Insert exactly one bullet per change **between the matching `###` heading and the
`---` line that follows it**, surrounded by blank lines:

```md
### Bug fixes

- fix: mark transactions as failed on intrinsic gas errors

---
```

Rules:
- Bullets start at column 0 with `- feat: ` / `- fix: ` and a short user-facing
  description, **all lowercase even when the PR title is Title-Case** (compare
  PRs #101/#102 to their bullets). Breaking changes are plain prose bullets with
  no prefix, describing old → new shape — the shipped precedent (0.5.0):
  ``- in the YAML changed from `automatic_top_up: Option<NetworkAutomaticTopUpConfig>` to `automatic_top_up: Option<Vec<NetworkAutomaticTopUpConfig>>` ``.
  See the breaking-change protocol in section 6.
- One line per change; keep it short and user-facing (what changed, not how).
- Features under `### Features`, fixes under `### Bug fixes`, breaking changes
  under `### Breaking changes`. Empty sections keep their skeleton — don't delete
  headings or `---` lines.

### 4.2 Release sections (only when cutting a release)

At release time the accumulated bullets move out of `## Changes Not Deployed`
into a new release section inserted directly **below** the line ``all release
branches are deployed through `release/VERSION_NUMBER` branches`` (newest release
first), and `## Changes Not Deployed` is reset to the empty skeleton (see commit
`aae21a7` for 0.12.0 as the reference diff; if a change landed without its bullet,
write it directly into the release section, as 0.12.1's `9838f8c` did). Exact
shape — copy the previous release entry and edit, don't retype:

```md
## 0.12.1 - 15th July 2026
-------------------------------------------------

github branch - https://github.com/joshstevens19/rrelayer/tree/release/0.12.1

- linux binary - https://github.com/joshstevens19/rrelayer/releases/download/v0.12.1/rrelayer_linux-amd64.tar.gz
- mac apple silicon binary - https://github.com/joshstevens19/rrelayer/releases/download/v0.12.1/rrelayer-arm64.tar.gz
- mac apple intel binary - https://github.com/joshstevens19/rrelayer/releases/download/v0.12.1/rrelayer-amd64.tar.gz
- windows binary - https://github.com/joshstevens19/rrelayer/releases/download/v0.12.1/rrelayer-amd64.zip

### Bug fixes

- fix: add better logs for config file
```

Format traps (all load-bearing, verified against the file):
- Heading is `## X.Y.Z - <day><st|nd|rd|th> <Month> <YYYY>` — no `v` prefix on the
  version, day without leading zero (`3rd July 2026`, `15th July 2026`).
- The underline under a release heading is **exactly 49 hyphens** on its own line;
  the separators inside `Changes Not Deployed` are `---`. Don't mix them up.
- Binary filenames are intentionally inconsistent: linux has an underscore
  (`rrelayer_linux-amd64.tar.gz`), mac/windows use hyphens (`rrelayer-arm64.tar.gz`,
  `rrelayer-amd64.tar.gz`, `rrelayer-amd64.zip`). Download URLs DO use the `v`
  prefix (`.../download/v0.12.1/...`). Copy from the previous entry verbatim.
- In release sections, include only the `###` subsections that have content.

---

## 5. Architecture crash course (crates/core)

Boot: `rrelayer start` → `crates/cli/src/commands/start.rs` → loads project `.env`,
auto-runs `docker compose up -d` if Postgres is unreachable → `rrelayer_core::start()`
in `crates/core/src/startup.rs`, which: reads `<project>/rrelayer.yaml`
(`crates/core/src/yaml.rs` — the struct is `SetupConfig` but the file is always
`rrelayer.yaml`), applies DB migrations (`crates/core/src/schema/`), builds one
`EvmProvider` per network (`crates/core/src/provider/` — this is where the signing
provider is selected), repopulates and spawns the per-relayer transaction queues
(`crates/core/src/transaction/queue_system/`), then starts the background tasks —
gas oracles (code in `crates/core/src/gas/`), webhooks, top-up, balance monitor —
via `run_background_tasks` (`crates/core/src/background_tasks/`), then serves axum
in `start_api()`.

HTTP: routes are nested in `startup.rs` (`crates/core/src/startup.rs:249-253`) —
`/auth`, `/networks`, `/relayers`, `/transactions`, `/signing`, plus `/health`.
The full domain shape is exhibited by `relayer/` and `transaction/`: `api/` (one
endpoint per file + `mod.rs` router), `db/` (`read.rs`, `write.rs`, `builders.rs`
— hand-written SQL, no ORM), `types/` (one newtype per file, serde +
`FromSql`/`ToSql`), `cache.rs`, and a `mod.rs` with selective `pub use`
re-exports. Other domains use a subset (`network/` has no `db/`; `signing/` has
no `types/` or `builders.rs`); copy the relayer/transaction shape for new domains
and wire new modules into `crates/core/src/lib.rs`.

Auth model: the global middleware only *stamps* whether Basic-auth credentials
(env `RRELAYER_AUTH_USERNAME`/`RRELAYER_AUTH_PASSWORD` — env vars, **not** the
yaml values, which are client-side) were valid. Handlers enforce auth themselves
via two `AppState` helpers (`crates/core/src/app_state.rs`):
`validate_basic_auth_valid` (strict — admin-only endpoints) and
`validate_allowed_passed_basic_auth` (short-circuits to `Ok` when any api_keys
are configured — only correct on relayer-scoped endpoints that subsequently call
`validate_auth_basic_or_api_key`, which accepts `x-rrelayer-api-key` from yaml
`networks[].api_keys`). Rate limiting is also in-handler
(`RateLimiter::check_and_reserve_rate_limit` + `reservation.commit()`).

Transaction lifecycle: Postgres enum `relayer.tx_status`:
`PENDING → INMEMPOOL → MINED → CONFIRMED` (plus FAILED/EXPIRED/CANCELLED/DROPPED/
REPLACED). Three tokio loops per relayer (queue_system/`start.rs`) move
transactions along, bumping gas per speed (SLOW/MEDIUM/FAST/SUPER) after
`gas_bump_blocks_every` blocks, capped by `max_gas_price_multiplier` and the
relayer's `max_gas_price`. `TransactionsQueues` is a single global
`Arc<Mutex<...>>` — long-held locks in that file stall **all** relayers.

Signing providers ("wallet managers") live in `crates/core/src/wallet/`, all
implementing `WalletManagerTrait` (`wallet/mod.rs`). `CompositeWalletManager`
routes wallet indexes ≥ `u32::MAX - 1000` to raw private keys (stored as negative
`wallet_index` in `relayer.record`) — respect both conventions in any
wallet-index arithmetic.

---

## 6. Recipes (multi-file changes that must stay in sync)

### Add an API endpoint
1. `crates/core/src/<domain>/api/<verb_noun>.rs` — handler
   `pub async fn x(State(state): State<Arc<AppState>>, headers: HeaderMap, ...) -> Result<Json<Resp>, HttpError>`.
   **First line is auth, and the helper choice matters**:
   - Admin-only endpoint (networks, relayer create/delete/pause/config, status):
     `state.validate_basic_auth_valid(&headers)?;`
   - Relayer-scoped endpoint usable with API keys:
     `state.validate_allowed_passed_basic_auth(&headers)?;` and then, after
     loading the relayer, `state.validate_auth_basic_or_api_key(...)?`. The first
     helper alone is a no-op when api_keys are configured — using it on an
     admin-only endpoint leaves that endpoint effectively unauthenticated.
   Tx/signing ops add the rate-limit reserve/commit pair.
2. Register in the domain's `api/mod.rs` router fn — names vary:
   `create_transactions_routes`, `create_relayer_routes`, `create_network_routes`,
   `create_signing_routes`, `create_basic_auth_routes` (see `startup.rs:249-253`).
   A new route group needs its own `create_*_routes()` nested in `start_api()`.
3. `pub use` new request/response types through `<domain>/mod.rs` and
   `crates/core/src/lib.rs`.
4. Mirror in the Rust SDK: raw method in `crates/sdk/src/api/<domain>/mod.rs`
   (relative path string must match the axum nesting exactly), expose on the right
   facade in `crates/sdk/src/clients.rs` (`Client` = admin/basic-auth;
   `RelayerClient` = API-key-capable; `AdminRelayerClient` = relayer + admin),
   re-export new types from `crates/sdk/src/lib.rs`, add a compile-checked snippet
   in `playground/rust-sdk-playground/src/<domain>/`.
5. Mirror in the TS SDK: `sdk/typescript/src/api/<domain>/<kebab-name>.ts` (a
   standalone function; `ApiBaseConfig` is always the LAST param; go through
   `axios-wrapper.ts`, endpoint string without leading slash), barrel-export in the
   domain `index.ts`, wire onto the matching client class in `src/clients/`
   (`clients/index.ts` is a curated export list — new public symbols must be added
   there explicitly). TS interface fields must byte-match the Rust serde renames
   (`#[serde(rename = "relayerId")]` etc. — copy from the Rust struct, don't guess).
6. Docs: update BOTH `documentation/rrelayer/docs/pages/integration/sdk/<area>/node.mdx`
   and `.../rust.mdx`, plus `integration/api/<area>.mdx`, plus sidebar/anchor
   entries in `documentation/rrelayer/vocs.config.tsx`.
7. Changelog bullet (section 4). Commit `feat: expose webhook types to Rust and
   Typescript SDKs (#97)` (`fd5b256`) is the canonical example of steps 3 and
   5–7 — mirroring types across both SDKs, docs twins, `vocs.config.tsx`, and the
   changelog in one PR (it does not add an endpoint itself).

Conventions shared by BOTH SDKs (keep new methods consistent):
- Mutating calls (send/cancel/replace/sign) take a trailing rate-limit key param
  (Rust `rate_limit_key: Option<String>`, TS `rateLimitKey?`) sent as the
  `x-rrelayer-rate-limit-key` header — both import a `RATE_LIMIT_HEADER_NAME`
  constant rather than hardcoding the string.
- "Get one" endpoints map 404 → absent: `get_or_none` → `ApiResult<Option<T>>` in
  Rust, `Promise<T | null>` in TS. Lists take a `PagingContext` and return a
  `PagingResult<T>`.
- `remove_max_gas_price` is implemented as `update_max_gas_price(0)` — the server
  treats 0 as unset. Keep that convention for related gas-cap methods.

### Add/alter a DB table or column
1. New file `crates/core/src/schema/v1_0_3.rs` (next version) with idempotent DDL
   (`CREATE TABLE IF NOT EXISTS`, `ADD COLUMN IF NOT EXISTS`, guarded
   `CREATE TYPE`) — `apply_schema` re-runs **every** migration on **every** boot
   with no version table; non-idempotent DDL bricks existing deployments.
2. Register it in `crates/core/src/schema/mod.rs` after `v1_0_2`.
3. Queries live in the owning domain's `db/` module. For `relayer.transaction`,
   writes are duplicated to `relayer.transaction_audit_log` via the
   `TRANSACTION_TABLES` const in `transaction/db/write.rs` — column lists must stay
   valid for BOTH tables, and the row mapper in `transaction/db/builders.rs` must
   learn the new column (misses panic at runtime, not compile time).
4. New Postgres schemas must also be added to the e2e reset lists:
   `crates/e2e-tests/src/infrastructure/embedded_rrelayer.rs` (`schemas_to_drop`)
   and the `reset-db` psql list in `crates/e2e-tests/Makefile`.

### Add a rrelayer.yaml config option
1. Add the field in `crates/core/src/yaml.rs` (`SetupConfig` global,
   `NetworkSetupConfig` per-network) with the existing serde-default idioms.
   `${ENV_VAR}` interpolation is automatic for all strings — note it **panics** at
   startup if the var is unset.
2. Plumb through `startup.rs` / `AppState` / queue or background-task params as
   needed.
3. Update example configs in `crates/e2e-tests/config/*.yaml`, the docs
   (`documentation/rrelayer/docs/pages/config/index.mdx` full example + the
   specific subpage), and the changelog.

### Change or remove a rrelayer.yaml option (breaking-change protocol)

Every deployed rrelayer has a `rrelayer.yaml` that must survive a binary
upgrade. What breaks users:

- **Changing a field's type or shape** → existing configs fail to parse and the
  server won't boot. This is the one shipped precedent (0.5.0:
  `automatic_top_up` went from `Option<NetworkAutomaticTopUpConfig>` to
  `Option<Vec<...>>`).
- **Making an optional field required** (or adding a required field) → parse
  failure on boot. New fields must be `Option<...>` or carry
  `#[serde(default)]`/`#[serde(default = "fn")]`.
- **Renaming or removing a field** → the *worst* case: `SetupConfig` does NOT
  use `deny_unknown_fields`, so an old key in a user's config is **silently
  ignored** and behavior quietly falls back to defaults (e.g. a renamed
  `permissions` key would silently disable allowlists). No error, no log.
- **Changing a `default_*` fn value** or a `${ENV_VAR}` becoming required
  (missing env vars **panic** at startup) → silent or fatal behavior change on
  upgrade with no config edit.

Protocol:
1. Prefer additive + optional. If renaming, keep the old name working via
   `#[serde(alias = "old_name")]` (no precedent in `yaml.rs` yet — you'd be
   setting it) or reject the old key explicitly in `read()`/`validate()` with a
   clear migration error. Never let an old key be silently ignored.
2. If the break is unavoidable: add a **plain prose bullet** under
   `### Breaking changes` in the changelog describing old → new shape, in the
   0.5.0 style above — this is exactly what that section exists for.
3. Update every in-repo `rrelayer.yaml` instance in the same PR:
   `crates/e2e-tests/config/*.yaml`, `playground/*/rrelayer.yaml`,
   `helm/rrelayer/values.yaml` (embeds a full `rrelayerConfig`),
   `providers/railway/example-app/rrelayer.yaml`, and the docs examples under
   `documentation/rrelayer/docs/pages/config/`.
4. Note: pre-1.0, breaking changes ship in minor version bumps (0.5.0 did) —
   there is no major-version gate protecting users, which makes the changelog
   bullet the only migration signal they get.

The same "breaking" lens applies beyond yaml: HTTP route/response shapes and
webhook payloads (external consumers + both SDKs), CLI flags/output that
scripts parse, env var names, and DB migrations (must be idempotent AND cope
with data written by older versions).

### Add a signing provider
Grep `fireblocks` repo-wide to find every touchpoint. Short list: config struct +
`SigningProvider` field + `validate()` in `yaml.rs`; manager in
`crates/core/src/wallet/<name>_wallet_manager.rs` implementing `WalletManagerTrait`;
`EvmProvider::new_with_<name>()` in `provider/evm_provider.rs`; dispatch (incl.
composite path and `has_main_signing_provider`) in `provider/mod.rs`; the
`private_key_only_networks` filter in `startup.rs`; e2e: `SigningProvider` enum in
`crates/e2e-tests/src/main.rs`, `config/<name>.yaml`, funding addresses in
`infrastructure/contract_interactions.rs`, Makefile target; docs page under
`config/signing-providers/` + `vocs.config.tsx`; changelog.

### Add a CLI command
Variant in `crates/cli/src/cli_interface.rs` → handler module in
`crates/cli/src/commands/<name>.rs` (+ error enum in `commands/error.rs`, `#[from]`
variant in `crates/cli/src/error.rs`) → match arm in `main.rs` (follow the
resolve-path → load-env → `ProjectLocation` → SDK client → `check_authenticate`
recipe) → Makefile target in `crates/cli/Makefile` → update the pasted `--help`
blocks in `documentation/rrelayer/docs/pages/getting-started/cli.mdx` → changelog.
The CLI never talks HTTP directly — it consumes the Rust SDK, so new endpoints
need the SDK method first.

### Add an e2e test
Async fn in `crates/e2e-tests/src/tests/<domain>/`, registered via
`TestDefinition::new("snake_case_name", ...)` in that domain's `get_tests()`; a
brand-new domain module must also be added to `TestRegistry::get_all_tests()` in
`src/tests/registry.rs` or it silently never runs. The name string is the
`make run-test TEST=<name>` key (exact match). Tests share one anvil + Postgres +
embedded server; the fixture values in `config/raw.yaml` (rate limits, allowlists,
funded addresses derived from the `.env.example` mnemonic) are load-bearing for
existing assertions, and registry order matters — the allowlist module
intentionally runs first.

### Add a docs page
`.mdx` under `documentation/rrelayer/docs/pages/` (file path = URL route) + a
sidebar entry in `documentation/rrelayer/vocs.config.tsx`. The sidebar hardcodes
heading-slug anchors — renaming any MDX heading requires grepping
`vocs.config.tsx` for the old slug. Verify with `npm run build`.

---

## 7. Testing reality (read before trusting green)

- CI runs only ~9 inline unit tests (`crates/core/src/shutdown.rs`,
  `crates/core/src/safe_proxy.rs`, `crates/sdk/src/alloy_integration.rs`).
  **CI never runs the e2e suite, never builds the TS SDK, never builds the docs.**
  A change can break all three and merge green.
- Real coverage is `crates/e2e-tests` (`e2e-runner` binary, run via its Makefile —
  see section 3). It must be run from `crates/e2e-tests/` (resolves config,
  contracts, docker-compose, `.env` from cwd). Raw provider needs no cloud
  credentials; kms/privy/turnkey/fireblocks/etc. need real secrets in `.env`.
- The e2e crate has no `#[test]` fns but IS a workspace member: compile errors in
  it fail every CI matrix leg.
- New unit tests: keep them inline (`#[cfg(test)] mod tests` at file bottom) and
  DB/network-free so `cargo test --workspace` stays hermetic.
- `sdk/typescript/jest.config.js` is dead (jest isn't installed, no test script,
  setup file missing). Don't add `*.test.ts` expecting them to run; use the
  `playground::*` scripts against a live server instead.
- For runtime verification of server changes: `cd crates/cli && make start_local`
  against `playground/local-node` (has an anvil Makefile), then hit
  `http://localhost:8000` or use CLI/SDK playground scripts.

---

## 8. Local dev environment

- Prereqs: Rust stable (edition-2024 crates need a recent stable), Docker,
  Foundry (`anvil`/`forge`/`cast`) for anything chain-touching, Node ≥ 20 for
  TS SDK/docs.
- Postgres ports by context — using the wrong one silently hits the wrong DB:
  **5446** root `docker-compose.yml` (dev), **5447** `crates/e2e-tests` compose
  (e2e), **5471** `playground/local-node` compose, **5441** projects generated by
  `rrelayer new`. `DATABASE_URL` comes from env/.env only — there is no DB setting
  in `rrelayer.yaml`.
- Server auth env vars must match between the server process and clients:
  `RRELAYER_AUTH_USERNAME`/`RRELAYER_AUTH_PASSWORD` (yaml `api_config` values are
  what the CLI/SDK read, typically `${...}`-interpolated from the same `.env`).
- The canonical, full `rrelayer.yaml` example is `crates/e2e-tests/config/raw.yaml`
  (every feature exercised); smaller runnable ones live in
  `playground/*/rrelayer.yaml`; the user-facing reference is
  `documentation/rrelayer/docs/pages/config/`.
- Secrets hygiene: `.env` (not `.env.example`), `*.key`,
  `playground/**/keystores`, and `crates/e2e-tests/rrelayer.yaml` are gitignored
  — never commit real mnemonics, private keys, or cloud credentials. The only
  acceptable hardcoded mnemonic is anvil's well-known `test test ... junk` in
  local fixtures.
- `LOCAL-SETUP.md` is stale (references `rrelayer_server/` and `setup.yaml`; the
  real config file is `rrelayer.yaml`). Prefer this file + `crates/cli/Makefile`.
- `Cargo.lock` is gitignored — ALL dependency versions float between builds.
  The workspace root sets `alloy = "1.1.3"`, but that is a caret (minimum)
  requirement, not a pin — fresh builds still pull the latest 1.x, and floating
  alloy has broken the build before (#78/#80). If a dependency break appears, pin
  it with an exact `=x.y.z` requirement at the workspace root — never in a single
  crate's manifest (crates use `workspace = true`).

---

## 9. Release & publish (maintainer-driven — don't do any of this unprompted)

1. Move `Changes Not Deployed` bullets into a new release section in the
   changelog (section 4.2) on a `release/X.Y.Z` branch and push it.
2. CI builds 4 platform binaries, seds `version =` in `crates/{cli,core,sdk}/Cargo.toml`
   (+ root, currently a no-op), commits `Release vX.Y.Z`, opens the PR to master.
   Do NOT hand-bump crate versions.
3. Merging the PR (title must keep `Release vX.Y.Z`) triggers the GitHub Release
   (tag `vX.Y.Z`, binary assets) and the ghcr.io docker image (`:X.Y.Z` + `:latest`).
4. crates.io publish is manual: `./scripts/publish-rrelayer.sh` (core first, then
   the SDK). Known issue: the script's sed expects
   `rrelayer_core = { path = "../core", version = "..." }` but
   `crates/sdk/Cargo.toml:29` currently has no `version` key — the SDK publish will
   fail until that's restored. Keep that dep on one line in the script's format.
5. npm publish of the TS SDK is manual and independent: hand-bump `version` in
   `sdk/typescript/package.json`, `npm publish` from that dir.
6. `documentation/rrelayer/docs/public/install.sh` is the production installer;
   it hardcodes the release asset names and `v`-tag format from `ci.yml`. Renaming
   the binary, archives, or tags requires updating ci.yml (both build jobs),
   docker.yml, the Dockerfile, and install.sh together.

---

## 10. Style & conventions

- **Commits/PR titles**: lowercase conventional prefix — `fix: ...`, `feat: ...`,
  `chore: ...`, `docs: ...`, optional scope (`feat(relayer): ...`). PRs target
  `master` and are squash-merged (the merge appends `(#N)`). Changelog bullets
  paraphrase the PR title in all-lowercase (section 4.1).
- **Branches**: `fix/<kebab-slug>`, `feature/<kebab-slug>`, `issue-<N>`.
  `release/*` is reserved.
- **Errors (Rust)**: thiserror enums per domain (`#[derive(Error, Debug)]`,
  `#[error("...: {0}")]`, `#[from]` conversions), plus `impl From<XError> for
  HttpError` via the `bad_request`/`forbidden`/`not_found`/`internal_server_error`
  helpers in `crates/core/src/shared/`. Confine `anyhow` to `crates/e2e-tests`
  — a few legacy uses exist in core (`transaction_blob.rs`, `evm_provider.rs`
  blob estimation); don't add more. Avoid `unwrap()`/`expect()` outside tests.
- **Logging**: `tracing`. Inside core use `info!`/`error!`; the CLI's
  server-start path uses the `rrelayer_info!`/`rrelayer_error!` re-exports from
  `rrelayer_core`; interactive CLI command output is plain `println!`;
  e2e-tests uses `tracing` directly.
- **Newtypes**: every domain scalar (RelayerId, TransactionId, GasPrice,
  EvmAddress, ChainId, …) is a newtype with serde + FromSql/ToSql; follow
  `crates/core/src/transaction/types/` as the template.
- **Long-running loops** must be shutdown-aware: `tokio::select!` on
  `subscribe_to_shutdown()`, `enter_critical_operation()` guards around
  non-interruptible sections (`crates/core/src/shutdown.rs`).
- **Editions differ intentionally**: cli/sdk/playground are edition 2024;
  core/e2e-tests are 2021. SDK uses thiserror 2.x, core uses 1.x. Don't harmonize.
- **TS**: prettier only (no ESLint), strict tsconfig, CommonJS/es2020. Never call
  axios directly — always via `src/api/axios-wrapper.ts`.

---

## 11. Trap index (quick scan before you commit)

- Forgot the changelog bullet → re-read section 1, rule 1.
- New handler without auth, or `validate_allowed_passed_basic_auth` on an
  admin-only endpoint (it's a no-op when api_keys are configured) → publicly
  reachable.
- Route/type change without touching `crates/sdk` + `sdk/typescript` + docs →
  three-way drift.
- Non-idempotent migration → every existing deployment fails to boot.
- `relayer.transaction` column added but `transaction_audit_log`/`builders.rs`
  missed → runtime panic on `row.get`.
- New `TransactionStatus` variant needs the Rust enum + `ALTER TYPE` migration for
  `relayer.tx_status` + queue-repopulation logic in `queue_system/start.rs`.
- Webhook `networks:` filters match yaml network *names*, not chain ids — renaming
  a network silently detaches its webhooks.
- Renaming/removing a `rrelayer.yaml` field is silent data loss for users:
  unknown keys are ignored (no `deny_unknown_fields`), so old configs quietly
  fall back to defaults instead of erroring. Follow the breaking-change protocol
  in section 6.
- Clippy `-D warnings` + unpinned stable toolchain: a new Rust release can fail CI
  on untouched code — that's not your change.
- jemalloc feature asymmetry: `crates/cli` must compile with AND without
  `--features jemalloc` (Windows/arm64-linux builds omit it).
- Local `docker build .` needs `mkdir -p docker-binary` first.
- e2e Makefile `start-postgres` health-checks the wrong port (5447 vs root compose
  5446) — the runner brings up its own compose; don't fight it. `verify-setup.sh`
  is stale and fails on the current tree; ignore it.
- Prettier must never touch `changelog.mdx` — it's in `.prettierignore` because
  a (currently disabled, expected to return) release step in `ci.yml` parses it
  with grep/sed line patterns.
- Rate-limit `interval` parsing (`rate_limiting/rate_limiter.rs`) only recognizes
  `"1m"` — every other string silently defaults to 60s. Configs saying
  `interval: "minute"` work by accident; don't build on that parser without
  extending it.
- Signing routes keep deprecated duplicate paths
  (`/signing/:relayer_id/...` alongside `/signing/relayers/:relayer_id/...`) for
  backward compatibility — don't remove them when touching that router.
- New native/system dependencies (openssl-like crates) must be added to every
  build environment: the apt steps in `ci.yml`/`docker.yml`, the Windows vcpkg
  step, and both Dockerfile stages. The Windows matrix leg breaks first.
- The e2e runner `pkill`s rrelayer and `kill -9`s whatever holds ports 3000/8545
  on start/teardown — never run it while a local dev server is up.
- TS SDK stale bits (existing state, don't trip on them): the
  `playground::viem::send-blob` npm script points at a file that doesn't exist,
  and `client.getRelayerClient()` hardcodes `providerUrl: 'TODO'` — use
  `createRelayerClient` with an explicit `providerUrl` for provider-backed
  methods.

---

## 12. Pre-PR checklist (definition of done)

Run through this before opening any PR:

1. `cargo fmt --all` && `cargo clippy -- -D warnings -A clippy::uninlined_format_args`
   && `cargo test --exclude rust-sdk-playground --workspace` all pass.
2. Changelog bullet added under `## Changes Not Deployed` (section 4) — or the
   change is genuinely invisible to users (pure refactor/CI/doc-typo).
   If anything about `rrelayer.yaml`, routes, webhook payloads, or CLI flags
   changed shape: prose bullet under `### Breaking changes` + every in-repo
   `rrelayer.yaml` instance updated (breaking-change protocol, section 6).
3. If any route, request/response type, or serde rename changed: Rust SDK, TS SDK,
   and docs twins updated (rule 4), `npm run build` passes in `sdk/typescript`,
   and a playground snippet exists for new SDK surface.
4. If docs/MDX changed: `npm run build` passes in `documentation/rrelayer`
   (CI won't catch it).
5. If engine/DB/signing behavior changed: e2e suite run locally
   (`make run-tests-debug` in `crates/e2e-tests`) — CI can't catch regressions
   there.
6. Commit/PR title is lowercase `fix:`/`feat:`/`chore:` style and contains no
   `Release v` / `release/` substrings.
7. No secrets in the diff (`.env` contents, mnemonics, keys, cloud credentials).
