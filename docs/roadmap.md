# Roadmap

| Phase | Goal | Done when |
|---|---|---|
| **0 · Spike** | Prove the shared Rust normalizer | One CSV profile and one PDF profile parse identically to a reference implementation on synthetic fixtures, and the crate builds for `aarch64-apple-ios` and `wasm32` |
| **1 · Replace Firefly III for one household** | Accounts, import, rules, transfers, reports, migration from Firefly III | A real household runs it side by side with Firefly III until balances match, then switches |
| **2 · iPhone app** | SwiftUI app with on-device statement parsing | Import a statement on the phone and see it on the web, with no duplicate |
| **3 · Public release** | Anyone can install it | `docker compose up` plus documentation, a demo instance and a first tagged release |
| **4 · Android** | The same app on Android through Skip | The iPhone feature set runs on Android |

Out of scope for now: investments and crypto portfolios, and a hosted service run by the project.
