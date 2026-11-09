#!/usr/bin/env bash
# End-to-end test of the web app against a real server. Builds the server, creates the database coinroost_e2e on
# the PostgreSQL server of DATABASE_URL if it is missing (every run enrolls its own user in its own household, so
# runs never see each other's data) and a setup link, starts coinroost-server on a free port with web/build, and runs Playwright.
# A second server, on coinroost_e2e_first (emptied each run: its test needs an app with no users), lets the first
# account be made from the web. Needs DATABASE_URL, a built web app (npm run build), and psql or Docker.
# E2E_DATABASE names another database pair: a branch that adds a migration runs apart, since a database that has
# a migration the code lacks refuses to migrate.
set -euo pipefail
web=$(cd "$(dirname "$0")/.." && pwd)
root=$(cd "$web/.." && pwd)
: "${DATABASE_URL:?DATABASE_URL is not set}"
db=${E2E_DATABASE:-coinroost_e2e}
[[ $db =~ ^[a-z_][a-z0-9_]*$ ]] || { echo "E2E_DATABASE must be a plain name" >&2; exit 1; }
export DATABASE_URL="${DATABASE_URL%/*}/$db"
[ -f "$web/build/200.html" ] || { echo "no web build: run npm run build in web/" >&2; exit 1; }

cargo build -q -p coinroost-server --locked --manifest-path "$root/Cargo.toml"
server="$root/target/debug/coinroost-server"
"$server" migrate

email="e2e-$(date +%s)-$$@example.com"
password=$(head -c 24 /dev/urandom | base64)
secret=$(printf '%s\n' "$password" | "$server" user add "$email" | sed -n 's/^secret //p')
[ -n "$secret" ] || { echo "user add printed no secret" >&2; exit 1; }
# add_user <name>: one more user, in <name>_email, <name>_password and <name>_secret. A TOTP code is accepted once,
# so a test that logs in on its own (the form errors, the forced colors, the rules, the landing, the empty pages,
# the shell, the queue, the forms that open, the presses, the install) takes its own user.
add_user() {
  local email password secret
  email="e2e-$1-$(date +%s)-$$@example.com"
  password=$(head -c 24 /dev/urandom | base64)
  secret=$(printf '%s\n' "$password" | "$server" user add "$email" | sed -n 's/^secret //p')
  [ -n "$secret" ] || { echo "user add printed no secret" >&2; exit 1; }
  printf -v "$1_email" '%s' "$email"
  printf -v "$1_password" '%s' "$password"
  printf -v "$1_secret" '%s' "$secret"
}
add_user forms
add_user contrast
add_user rules
add_user landing
add_user empty
add_user shell
add_user categorize
add_user queue
add_user open
add_user press
add_user glide
add_user pwa
add_user feedback
# The feedback page from its empty state, once per motion setting (feedback-page.spec.ts).
add_user feedback_motion
add_user feedback_still
# Setup links for the web enrollment, recovery-code and language tests, each of which makes its own user in a new
# household.
invite=$("$server" invite add 2>/dev/null | sed -n 's/^invite .*#//p')
recovery_invite=$("$server" invite add 2>/dev/null | sed -n 's/^invite .*#//p')
language_invite=$("$server" invite add 2>/dev/null | sed -n 's/^invite .*#//p')
[ -n "$invite" ] && [ -n "$recovery_invite" ] && [ -n "$language_invite" ] || { echo "invite add printed no link" >&2; exit 1; }

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
psql_run "drop database if exists ${db}_first with (force)"
DATABASE_URL="${DATABASE_URL%/*}/${db}_first" "$server" migrate

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
start "$web/e2e-server-first.log" first open "${DATABASE_URL%/*}/${db}_first"

cd "$web"
npx playwright install chromium >/dev/null
BASE_URL="http://$address" E2E_FIRST_URL="http://$first" E2E_SERVER="$server" E2E_EMAIL="$email" E2E_PASSWORD="$password" E2E_SECRET="$secret" E2E_INVITE="$invite" \
  E2E_FORMS_EMAIL="$forms_email" E2E_FORMS_PASSWORD="$forms_password" E2E_FORMS_SECRET="$forms_secret" \
  E2E_CONTRAST_EMAIL="$contrast_email" E2E_CONTRAST_PASSWORD="$contrast_password" E2E_CONTRAST_SECRET="$contrast_secret" \
  E2E_RULES_EMAIL="$rules_email" E2E_RULES_PASSWORD="$rules_password" E2E_RULES_SECRET="$rules_secret" \
  E2E_LANDING_EMAIL="$landing_email" E2E_LANDING_PASSWORD="$landing_password" E2E_LANDING_SECRET="$landing_secret" \
  E2E_EMPTY_EMAIL="$empty_email" E2E_EMPTY_PASSWORD="$empty_password" E2E_EMPTY_SECRET="$empty_secret" \
  E2E_SHELL_EMAIL="$shell_email" E2E_SHELL_PASSWORD="$shell_password" E2E_SHELL_SECRET="$shell_secret" \
  E2E_CATEGORIZE_EMAIL="$categorize_email" E2E_CATEGORIZE_PASSWORD="$categorize_password" E2E_CATEGORIZE_SECRET="$categorize_secret" \
  E2E_QUEUE_EMAIL="$queue_email" E2E_QUEUE_PASSWORD="$queue_password" E2E_QUEUE_SECRET="$queue_secret" \
  E2E_OPEN_EMAIL="$open_email" E2E_OPEN_PASSWORD="$open_password" E2E_OPEN_SECRET="$open_secret" \
  E2E_PRESS_EMAIL="$press_email" E2E_PRESS_PASSWORD="$press_password" E2E_PRESS_SECRET="$press_secret" \
  E2E_GLIDE_EMAIL="$glide_email" E2E_GLIDE_PASSWORD="$glide_password" E2E_GLIDE_SECRET="$glide_secret" \
  E2E_PWA_EMAIL="$pwa_email" E2E_PWA_PASSWORD="$pwa_password" E2E_PWA_SECRET="$pwa_secret" \
  E2E_FEEDBACK_EMAIL="$feedback_email" E2E_FEEDBACK_PASSWORD="$feedback_password" E2E_FEEDBACK_SECRET="$feedback_secret" \
  E2E_FEEDBACK_MOTION_EMAIL="$feedback_motion_email" E2E_FEEDBACK_MOTION_PASSWORD="$feedback_motion_password" E2E_FEEDBACK_MOTION_SECRET="$feedback_motion_secret" \
  E2E_FEEDBACK_STILL_EMAIL="$feedback_still_email" E2E_FEEDBACK_STILL_PASSWORD="$feedback_still_password" E2E_FEEDBACK_STILL_SECRET="$feedback_still_secret" \
  E2E_RECOVERY_INVITE="$recovery_invite" E2E_LANGUAGE_INVITE="$language_invite" npx playwright test "$@"
