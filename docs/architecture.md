# Architecture

## Components

```
crates/normalizer   Bank statement parsing: CSV, XLSX, PDF -> normalized rows
crates/domain       Accounts, transactions, transfers, categories, rules
server/             Axum HTTP API, PostgreSQL, serves the web build
web/                SvelteKit (Svelte 5) static app
spec/profiles/      One declarative profile per bank export format
spec/fixtures/      Synthetic input files and their expected normalized output
```

The mobile app lives in [coinroost-mobile](https://github.com/Celggar/coinroost-mobile) and links the same Rust core
as a native library.

## One normalizer, three places

The normalizer is written once in Rust and runs:

- **on the server**, natively;
- **in the browser**, compiled to WebAssembly, so a statement can be parsed before anything is uploaded;
- **in the mobile app**, through UniFFI-generated Swift bindings, so the statement never leaves the phone.

Every implementation must reproduce `spec/fixtures/*/expected.json` exactly. Bank profiles are data, not code: adding
a bank means adding a profile and a fixture, not a new parser.

### Normalized row

| Field | Type | Notes |
|---|---|---|
| `date` | ISO 8601 date | Booking date as printed by the bank |
| `amount_minor` | signed integer | Minor units; negative is money leaving the account |
| `currency` | ISO 4217 code | `USD` today; the model is multi-currency |
| `description` | string | As printed, whitespace normalized |
| `external_id` | string | Stable hash of account, date, amount, description and occurrence number |

`external_id` is the deduplication key: importing the same file twice, from any client, is a no-op.

## Rules

Categorization rules are evaluated **in order, first match wins**, and are deterministic: running them again over
the same data produces the same result. Rules never fire implicitly on later edits.

## Transfers

A transfer between two of the household's own accounts is one transaction with two legs. When only one side has
been imported yet, it waits as an unmatched leg and is paired automatically when the other side arrives. Transfers
never count as income or spending.

## Deployment

One container for the server (which also serves the web build) plus PostgreSQL. Each household runs its own
instance; the project does not host anyone's data.

## Open questions

- Direct bank connections versus file import only.
- Multi-currency accounts and exchange rates.
