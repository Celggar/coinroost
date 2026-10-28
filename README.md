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

The server listens on `127.0.0.1:8080` (`COINROOST_BIND` changes it) and creates the database if it is missing.
After a route changes, `cargo run -p coinroost-server -- openapi > spec/openapi.json` refreshes the contract and
`npm run api-types` in `web/` refreshes the app's types from it.

## Contributing

Contributions are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md) first: every change goes through a pull
request reviewed by the maintainer, and everything in the repository is written in English.

## Support the project

If Coinroost is useful to you, you can sponsor it through [GitHub Sponsors](https://github.com/sponsors/Celggar).

## License

[MIT](LICENSE)
