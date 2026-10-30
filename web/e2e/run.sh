#!/usr/bin/env bash
# End-to-end test of the web app against a real server. Builds the server, creates the database coinroost_e2e on
# the PostgreSQL server of DATABASE_URL if it is missing (every run enrolls its own user in its own household, so
# runs never see each other's data) and a setup link, starts coinroost-server on a free port with web/build, and runs Playwright.
# A second server, on coinroost_e2e_first (emptied each run: its test needs an app with no users), lets the first
# account be made from the web. Needs DATABASE_URL, a built web app (npm run build), and psql or Docker.
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
# A setup link for the web enrollment test, which makes its own user in a new household.
invite=$("$server" invite add 2>/dev/null | sed -n 's/^invite .*#//p')
[ -n "$invite" ] || { echo "invite add printed no link" >&2; exit 1; }

# The connection settings go to psql through the environment, never the command line, where other users see them.
admin=${DATABASE_URL#*://}
export PGUSER=${admin%%:*} PGPASSWORD=${admin#*:} PGDATABASE=postgres
PGPASSWORD=${PGPASSWORD%%@*}
pghost=${admin#*@}
pghost=${pghost%%/*}
export PGHOST=${pghost%:*} PGPORT=${pghost##*:}
psql_run() {
  if command -v psql >/dev/null; then
    psql -q -v ON_ERROR_STOP=1 -c "$1"
  else
    docker run --rm --network host -e PGUSER -e PGPASSWORD -e PGDATABASE -e PGHOST -e PGPORT postgres:18.6-alpine \
      psql -q -v ON_ERROR_STOP=1 -c "$1"
  fi
}
psql_run 'drop database if exists coinroost_e2e_first with (force)'
DATABASE_URL="${DATABASE_URL%/*}/coinroost_e2e_first" "$server" migrate

# start <log> <variable> <open|closed> <database URL>: starts a server and sets the variable to the address it
# serves on. Settings go through the environment, never argv.
# Playwright empties test-results/ when it starts, so the logs live next to it.
pids=()
trap 'kill "${pids[@]}" 2>/dev/null || true' EXIT
start() {
  local log=$1 variable=$2
  COINROOST_FIRST_ACCOUNT=$3 DATABASE_URL=$4 COINROOST_BIND=127.0.0.1:0 COINROOST_WEB_DIR="$web/build" \
    "$server" serve >/dev/null 2>"$log" &
  pids+=($!)
  local found=""
  for _ in $(seq 100); do
    found=$(sed -n 's/^serving on //p' "$log")
    [ -n "$found" ] && break
    kill -0 "${pids[-1]}" 2>/dev/null || { cat "$log" >&2; return 1; }
    sleep 0.1
  done
  [ -n "$found" ] || { echo "the server did not start" >&2; cat "$log" >&2; return 1; }
  printf -v "$variable" '%s' "$found"
}
start "$web/e2e-server.log" address closed "$DATABASE_URL"
start "$web/e2e-server-first.log" first open "${DATABASE_URL%/*}/coinroost_e2e_first"

cd "$web"
npx playwright install chromium >/dev/null
BASE_URL="http://$address" E2E_FIRST_URL="http://$first" E2E_SERVER="$server" E2E_EMAIL="$email" E2E_PASSWORD="$password" E2E_SECRET="$secret" E2E_INVITE="$invite" \
  npx playwright test "$@"
