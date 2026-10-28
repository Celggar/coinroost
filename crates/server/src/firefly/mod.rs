//! Moving a Firefly III ledger into Coinroost, in three steps that keep Firefly untouched:
//!
//! - `fetch` reads every page of every entity type through Firefly's API with GET requests only, and writes them
//!   as they came into a snapshot folder, with each own account's balance at every month end.
//! - `import` reads that folder offline: accounts named in a map, categories, payees and every journal, keeping
//!   the bank's `external_id` so a statement imported later adds nothing it already has.
//! - `check` compares every mapped account's balance at each month end with Firefly's own figure, to the cent.

pub mod amount;
pub mod check;
pub mod fetch;
mod http;
pub mod import;
pub mod plan;
pub mod snapshot;

/// One entity type of Firefly's API: where it is listed, and where it lives in Coinroost (`None`: a gap the
/// report names, with its count).
pub struct Kind {
    pub name: &'static str,
    pub path: &'static str,
    pub query: &'static str,
    pub home: Option<&'static str>,
}

/// Everything a Firefly ledger holds, in the order it is fetched and reported.
pub const KINDS: [Kind; 17] = [
    kind(
        "accounts",
        "/api/v1/accounts",
        "type=all",
        Some("accounts (asset, liability) and payees"),
    ),
    kind(
        "transactions",
        "/api/v1/transactions",
        "type=all",
        Some("transactions and legs"),
    ),
    kind("categories", "/api/v1/categories", "", Some("categories")),
    kind(
        "currencies",
        "/api/v1/currencies",
        "",
        Some("each account's currency"),
    ),
    kind("tags", "/api/v1/tags", "", None),
    kind("budgets", "/api/v1/budgets", "", None),
    // Firefly requires both dates and excludes both bounds: start after 1970-01-02, end before 2038-01-17.
    kind(
        "budget-limits",
        "/api/v1/budget-limits",
        "start=1970-01-03&end=2038-01-16",
        None,
    ),
    kind("bills", "/api/v1/bills", "", None),
    kind("piggy-banks", "/api/v1/piggy-banks", "", None),
    kind("attachments", "/api/v1/attachments", "", None),
    kind("recurrences", "/api/v1/recurrences", "", None),
    kind("rule-groups", "/api/v1/rule-groups", "", None),
    kind("rules", "/api/v1/rules", "", None),
    kind("webhooks", "/api/v1/webhooks", "", None),
    kind("object-groups", "/api/v1/object-groups", "", None),
    kind("transaction-links", "/api/v1/transaction-links", "", None),
    kind("link-types", "/api/v1/link-types", "", None),
];

const fn kind(
    name: &'static str,
    path: &'static str,
    query: &'static str,
    home: Option<&'static str>,
) -> Kind {
    Kind {
        name,
        path,
        query,
        home,
    }
}
