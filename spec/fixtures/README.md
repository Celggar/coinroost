# Fixtures

One folder per case. Each holds:

- `input.*` — a synthetic export with the real file's structure. Every name, reference and amount is invented.
- `expected.json` — the reference output for that input: `profile` (an id from `spec/profiles/`), `namespace`
  (the account key that goes into `external_id`), `input`, and `result` with the transactions (amount as
  written and in minor units, description, `external_id`), the error rows and the row counts.

Every implementation of the normalizer must reproduce `result` exactly; `crates/normalizer/tests/fixtures.rs`
checks the Rust one. Generated inputs keep their generator next to them (`coopeduc/make_input.py` writes
both Coopeduc cases, `bg-account/make_input.py` both Banco General workbooks). `bg-christmas-savings/make_input.py`
also draws the same statement in other ways (rules as rectangles, thin fills, one path, scaled, rotated, rules
that stop short); every drawing must give the same table.

| Folder | What it pins down |
|---|---|
| `bac-account` | Three-section CSV, windows-1252, quoted thousands, padded descriptions, equal rows on one day, an unreadable date |
| `coopeduc` | One transaction per PDF text line, the table header on every page, a line without amounts |
| `coopeduc-word-gap` | Words separated by narrow `TJ` offsets instead of spaces: they join into one word, as in the reference |
| `bac-card` | Card CSV with a header block and a summary block, a masked card number, purchases positive (sign inverted), rows without a date, quoted amounts, equal purchases on one day |
| `bg-account` | XLSX with seven metadata rows above the header, one signed `Monto`, dates and numbers both as text and as cells, a blank row |
| `bg-account-debit-credit` | The same account in the accounting layout: `Débito` already negative and `Crédito` |
| `bg-christmas-savings` | PDF table drawn with ruling lines: descriptions over several lines in one cell, Spanish month dates, a signed `Monto` with a dollar sign, the header repeated on the second page, an opening balance with a merged cell, an impossible date and an empty amount |
