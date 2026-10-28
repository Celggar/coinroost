# Fixtures

One folder per case. Each holds:

- `input.*` — a synthetic export with the real file's structure. Every name, reference and amount is invented.
- `expected.json` — the reference output for that input: `profile` (an id from `spec/profiles/`), `namespace`
  (the account key that goes into `external_id`), `input`, and `result` with the transactions (amount as
  written and in minor units, description, `external_id`), the error rows and the row counts.

Every implementation of the normalizer must reproduce `result` exactly; `crates/normalizer/tests/fixtures.rs`
checks the Rust one. Generated inputs keep their generator next to them (`coopeduc/make_input.py` writes
both Coopeduc cases).

| Folder | What it pins down |
|---|---|
| `bac-account` | Three-section CSV, windows-1252, quoted thousands, padded descriptions, equal rows on one day, an unreadable date |
| `coopeduc` | One transaction per PDF text line, the table header on every page, a line without amounts |
| `coopeduc-word-gap` | Words separated by narrow `TJ` offsets instead of spaces: they join into one word, as in the reference |
