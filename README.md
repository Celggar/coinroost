# Coinroost

Self-hosted personal finance for a household: import your bank statements, categorize every transaction with
rules you can predict, and see where the money goes. Your data stays on your own server, and on your phone the
statement is parsed locally and never leaves the device.

> **Status: pre-alpha.** Nothing here is usable yet. The architecture is decided and the first spike is next.

## Why another one

Coinroost is built by people who ran [Firefly III](https://github.com/firefly-iii/firefly-iii) for real and kept
hitting the same walls. It borrows ideas, not code. The goals are:

- **Rules you can predict.** Ordered, first-match categorization rules; re-running them never shuffles results.
- **Transfers are first-class.** Moving money between your own accounts is a transfer, not a withdrawal plus a
  deposit that inflates your spending.
- **Coverage you can see.** Gaps in imported history are shown, not discovered months later.
- **Payee is not the channel.** A payment app or card network is how you paid, not who you paid.
- **Light on hardware.** One small binary and PostgreSQL; comfortable on a home server or a small VPS.
- **Statements parsed where they live.** Bank exports (CSV, XLSX, PDF) are normalized by a shared core that runs
  on the server, in the browser and inside the mobile app.

## Architecture

| Part | Technology | Where |
|---|---|---|
| Statement normalizer and domain core | Rust, shared through UniFFI (mobile) and WebAssembly (web) | `crates/` |
| API server | Rust (Axum) + PostgreSQL, contract in `spec/openapi.json` | `crates/server/` |
| Web app | SvelteKit (Svelte 5), built as static files served by the API | `web/` |
| Bank profiles and golden fixtures | Declarative data, used by every implementation | `spec/` |
| Mobile app | SwiftUI (iPhone first; Android later through Skip) | [coinroost-mobile](https://github.com/Celggar/coinroost-mobile) |

Money is stored as integer minor units with an ISO 4217 currency code. Each imported row carries a stable
deduplication id, so importing the same statement twice, from the web or from the phone, never duplicates it.

See [docs/architecture.md](docs/architecture.md) for the details and [docs/roadmap.md](docs/roadmap.md) for the plan.

## Running the checks

`./ci.sh` runs formatting, lints, every test, the iOS and WebAssembly builds of the normalizer, the web app's
checks and build, an end-to-end import in a headless Chromium against a real server, the repository policy and
a secret scan. It needs Node.js 24 besides Rust. The server tests need PostgreSQL 18 at `DATABASE_URL`; each test
creates its own database there, and the end-to-end test uses a database named `coinroost_e2e` on the same
server. A throwaway one for development:

```sh
docker run -d --name coinroost-dev-postgres -p 127.0.0.1:55432:5432 \
  -e POSTGRES_USER=coinroost -e POSTGRES_PASSWORD=coinroost -e POSTGRES_DB=coinroost postgres:18.6-alpine
export DATABASE_URL=postgres://coinroost:coinroost@127.0.0.1:55432/coinroost
./ci.sh
```

To run it yourself:

```sh
(cd web && npm ci && npm run build)
cargo run -p coinroost-server -- user add you@example.com    # asks for a password, prints the TOTP enrollment
COINROOST_WEB_DIR=web/build cargo run -p coinroost-server -- serve
```

## Running it with Docker

`deploy/` runs the server, its web app and PostgreSQL with Docker Compose on one machine. The app is published on
`127.0.0.1:8094` alone, for a reverse proxy on the same host; the database has no port and no route out.

```sh
docker volume create coinroost-pgdata   # once: the database lives here, outside any compose file
deploy/deploy.sh                         # builds, starts, then checks the port and the health endpoint
docker compose -f deploy/compose.prod.yml --env-file ~/.config/coinroost/prod.env \
  exec app coinroost-server user add you@example.com   # asks for your password
```

- `deploy.sh` reads `~/.config/coinroost/prod.env` (see `deploy/prod.env.example`) and refuses it unless only its
  owner can read it. When it does not exist, it creates one with a random database password nobody has to type.
- A reverse proxy running in its own compose project can reach the app by name instead: set
  `COINROOST_PROXY_NETWORK` to a network that project creates, and `deploy.sh` joins the app to it
  (`deploy/compose.proxy.yml`) as `coinroost-app-1:8080`. The database never joins it. Add the proxy's host names
  to `COINROOST_MCP_HOSTS` if MCP clients come through it.
- The app gets 256 MB and the database 512 MB. The app runs as an unprivileged user on a read-only filesystem.
- `docker compose down -v` cannot delete your data: the volume is declared external.
- To move a Firefly III ledger in, fetch it on the host first (see [Migrating from Firefly III](#migrating-from-firefly-iii)).
  The snapshot is readable by you alone, so run the import as yourself; `firefly check` takes the same arguments:
  ```sh
  docker compose -f deploy/compose.prod.yml --env-file ~/.config/coinroost/prod.env run --rm --no-deps \
    --user "$(id -u):$(id -g)" -v ~/firefly-snapshot:/snapshot:ro -v ~/map.json:/map.json:ro \
    app firefly import /snapshot --household <household id> --accounts /map.json
  ```

The server listens on `127.0.0.1:8080` (`COINROOST_BIND` changes it) and creates the database if it is missing.

Rules live on the Rules page: the first rule from the top that matches sets the category and payee of each
imported transaction, and the Uncategorized page lists what no rule sorted. A category or payee you set by hand
is never changed by a rule. A whole rule set can also be loaded from a file; loading it again changes nothing:

```sh
cat > rules.json <<'EOF'
{ "categories": [{ "name": "Groceries", "kind": "expense" }],
  "rules": [{ "name": "market", "when": { "contains": "SUPERMARKET", "direction": "out" },
              "then": { "category": "Groceries", "payee": "Market" } }] }
EOF
cargo run -p coinroost-server -- rules load rules.json --household <household id>
cargo run -p coinroost-cli -- categorize --rules rules.json < transactions.jsonl   # try a set without a server
```

Employers paid by the fortnight are loaded the same way; the payroll report then lists, per employer, the
fortnights still owed (measured at the account's last imported statement, never beyond), the ones paid ahead and
each payment's history:

```sh
cat > employers.json <<'EOF'
{ "employers": [{ "name": "Acme", "match": "ACME", "account": "checking",
                  "min_amount_minor": 70000, "amount_minor": 80000, "start": "2026-01" }] }
EOF
cargo run -p coinroost-server -- payroll load employers.json --household <household id>
```
After a route changes, `cargo run -p coinroost-server -- openapi > spec/openapi.json` refreshes the contract and
`npm run api-types` in `web/` refreshes the app's types from it.

## Connecting an MCP client

The server also speaks the [Model Context Protocol](https://modelcontextprotocol.io) at `/mcp` (Streamable
HTTP), so any MCP client can read the ledger, sum it up and, once you say yes, record or sort transactions. Every
tool sends a request to the same API the web app uses, with your token, so it gets the same checks and nothing
more. Give each client its own token:

```sh
cargo run -p coinroost-server -- token add you@example.com --name laptop                # prints crt_… once
cargo run -p coinroost-server -- token add you@example.com --name viewer --read-only    # may only read
cargo run -p coinroost-server -- token revoke you@example.com --name laptop
```

Then point the client at `http://127.0.0.1:8080/mcp` with the header `Authorization: Bearer crt_…`.

| Kind | Tools |
|---|---|
| Read | `list_accounts`, `list_categories`, `list_rules`, `list_transactions`, `search_transactions`, `about` |
| Reports | `spending_summary`, `monthly_trend` (with the gaps between statements), `top_payees`, `payroll_status` |
| Write | `create_transaction`, `update_transaction`, `categorize_transaction`, `delete_transaction`, `create_category` |
| Rules | `create_rule`, `move_rule`, `preview_rules`, `apply_rules` |

Amounts go in and out as decimal strings in the account's currency (`"45.50"`), never rounded; accounts,
categories, payees and rules are named by name or id. A category set through a tool counts as set by hand, so
rules never change it. A read-only token gets the API's refusal on any change, rule previews included.

`/mcp` accepts only the `Host` names in `COINROOST_MCP_HOSTS` (comma separated; a name alone matches any port,
`name:port` only that port). Unset or empty, that means loopback: behind a reverse proxy, add its public name. It
refuses every request that carries an `Origin` header, which browsers send, and never reads the login cookie, so a
web page open in your browser cannot drive it.

## Migrating from Firefly III

Three commands move a Firefly III ledger (API v1, tested against 6.6.3) without changing anything in it. Firefly's
tokens cannot be limited to reading, so the promise is in the client: it can only send GET requests.

```sh
(umask 077; read -rs -p "Firefly token: " t && printf %s "$t" > ~/firefly.token); echo   # readable by you alone
FIREFLY_URL=http://firefly.lan FIREFLY_TOKEN_FILE=~/firefly.token \
  cargo run -p coinroost-server -- firefly fetch --out ~/firefly-snapshot
cargo run -p coinroost-server -- firefly import ~/firefly-snapshot --household <household id> --accounts map.json
cargo run -p coinroost-server -- firefly check ~/firefly-snapshot --household <household id> --accounts map.json
```

- **`fetch`** copies every page of every entity type into a new folder only you can read (it holds your financial
  data). It also copies the groups no list returns (a liability's opening credit, found through the ids the lists
  skip) and each asset and liability account's balance at every month end. Firefly must answer on `http://`: run it
  on your network or through a local tunnel.
- **`import`** works offline from that folder and a map that names each Firefly asset or liability account:
  ```json
  { "accounts": { "1": { "name": "Checking", "kind": "checking", "namespace": "bank-checking", "profile": "bac-account" } },
    "categories": { "Savings": "transfer" } }
  ```
  - The namespace must be the one the account's statements are imported with. Each leg keeps Firefly's
    `external_id`, so a statement imported later adds nothing it already has, and the migration run after a
    statement adds only what is missing.
  - The legs follow the accounts: a payment between two of your accounts becomes one transfer whatever Firefly
    called it.
  - Categories keep Firefly's choice and rules never change them. Expense and revenue accounts become payees.
  - Running it again adds nothing.
- **`check`** compares every mapped account's balance at each month end with Firefly's own figure, to the cent, and
  exits non-zero if any differs.

The import report counts every Firefly entity type next to where it now lives. Tags, budgets, bills, piggy banks,
attachments, recurring transactions, Firefly's own rules, webhooks, object groups and transaction links have no home
in Coinroost yet: they are listed as gaps with their counts, never dropped silently.

## Contributing

Contributions are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md) first: every change goes through a pull
request reviewed by the maintainer, and everything in the repository is written in English.

## Support the project

If Coinroost is useful to you, you can sponsor it through [GitHub Sponsors](https://github.com/sponsors/Celggar).

## License

[MIT](LICENSE)
