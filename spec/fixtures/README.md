# Fixtures

One folder per profile in `spec/profiles/`, named after the profile id:

- `input.*` — a synthetic export with the real file's structure. Every name, reference and amount is invented.
- `expected.json` — the reference output for that input: `profile`, `namespace` (the account key that goes
  into `external_id`), `input`, and `result` with the transactions (amount as written and in minor units,
  description, `external_id`), the error rows and the row counts.

Every implementation of the normalizer must reproduce `result` exactly; `crates/normalizer/tests/fixtures.rs`
checks the Rust one. A fixture whose input is generated keeps its generator next to it (`make_input.py`).
