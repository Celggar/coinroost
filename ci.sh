#!/usr/bin/env bash
# The full check, the same locally and before every push: formatting, lints, tests, the mobile and web
# targets of the normalizer, repository policy and secret scan.
#
# On a machine that declares a resource ceiling for this project (the systemd user slice `coinroost.slice`),
# the whole run is confined to it so a build never takes the machine. Anywhere else it runs as is.
set -euo pipefail
cd "$(dirname "$0")"

if [ -z "${COINROOST_CI_CONFINED:-}" ] && command -v systemd-run >/dev/null 2>&1 \
  && [ "$(systemctl --user show coinroost.slice -p LoadState --value 2>/dev/null)" = "loaded" ]; then
  exec systemd-run --user --slice=coinroost.slice --scope --same-dir --quiet -- \
    env COINROOST_CI_CONFINED=1 bash "$0" "$@"
fi

if [ -d "$HOME/.cargo/bin" ]; then PATH="$HOME/.cargo/bin:$PATH"; fi

step() { printf '\n== %s\n' "$*"; }

step "format"
cargo fmt --all --check

step "lints"
cargo clippy --workspace --all-targets --locked -- -D warnings

step "tests"
cargo test --workspace --locked

for target in aarch64-apple-ios wasm32-unknown-unknown; do
  step "normalizer builds for $target"
  cargo check -p coinroost-normalizer --target "$target" --locked
done

step "repository policy"
base=$(git merge-base origin/main HEAD 2>/dev/null || git rev-list --max-parents=0 HEAD | tail -1)
bash scripts/policy/check.sh files
bash scripts/policy/check.sh commits "$base" HEAD
bash scripts/policy/check.sh language "$base" HEAD

step "secrets"
gitleaks git --no-banner --redact .

printf '\nci: all checks passed\n'
