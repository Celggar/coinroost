#!/usr/bin/env bash
# End-to-end test of the web app against a real server. Builds the server, creates the database coinroost_e2e on
# the PostgreSQL server of DATABASE_URL if it is missing (every run enrolls its own user in its own household, so
# runs never see each other's data), starts coinroost-server on a free port with web/build, and runs Playwright.
# Needs DATABASE_URL and a built web app (npm run build).
set -euo pipefail
web=$(cd "$(dirname "$0")/.." && pwd)
root=$(cd "$web/.." && pwd)
: "${DATABASE_URL:?DATABASE_URL is not set}"
export DATABASE_URL="${DATABASE_URL%/*}/coinroost_e2e"
[ -f "$web/build/200.html" ] || { echo "no web build: run npm run build in web/" >&2; exit 1; }

cargo build -q -p coinroost-server --locked --manifest-path "$root/Cargo.toml"
server="$root/target/debug/coinroost-server"
"$server" migrate

email="e2e-$(date +%s)-$$@example.com"
password=$(head -c 24 /dev/urandom | base64)
secret=$(printf '%s\n' "$password" | "$server" user add "$email" | sed -n 's/^secret //p')
[ -n "$secret" ] || { echo "user add printed no secret" >&2; exit 1; }

# Playwright empties test-results/ when it starts, so the server log lives next to it.
log="$web/e2e-server.log"
COINROOST_BIND=127.0.0.1:0 COINROOST_WEB_DIR="$web/build" "$server" serve 2>"$log" &
pid=$!
trap 'kill "$pid" 2>/dev/null || true' EXIT
address=""
for _ in $(seq 100); do
  address=$(sed -n 's/^serving on //p' "$log")
  [ -n "$address" ] && break
  kill -0 "$pid" 2>/dev/null || { cat "$log" >&2; exit 1; }
  sleep 0.1
done
[ -n "$address" ] || { echo "the server did not start" >&2; cat "$log" >&2; exit 1; }

cd "$web"
npx playwright install chromium >/dev/null
BASE_URL="http://$address" E2E_EMAIL="$email" E2E_PASSWORD="$password" E2E_SECRET="$secret" npx playwright test "$@"
