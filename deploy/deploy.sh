#!/usr/bin/env bash
# Builds and starts coinroost with docker compose, then checks what it started:
#   deploy/deploy.sh [env file]        (default: ~/.config/coinroost/prod.env)
# The env file must be readable by its owner alone. When it does not exist it is created with a random database
# password, which nobody has to type or see.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
env_file=${1:-$HOME/.config/coinroost/prod.env}
compose=(docker compose -f "$here/compose.prod.yml" --env-file "$env_file")

if [[ ! -e $env_file ]]; then
  mkdir -p "$(dirname "$env_file")"
  password=$(head -c 24 /dev/urandom | od -An -tx1 | tr -d ' \n')
  (umask 077 && printf 'POSTGRES_USER=coinroost\nPOSTGRES_DB=coinroost\nPOSTGRES_PASSWORD=%s\n' "$password" >"$env_file")
  echo "created $env_file with a random database password"
fi
mode=$(stat -c %a "$env_file")
if [[ $mode != 600 ]]; then
  echo "$env_file is mode $mode: run chmod 600 on it" >&2
  exit 1
fi
if ! docker volume inspect coinroost-pgdata >/dev/null 2>&1; then
  echo "no volume coinroost-pgdata: create it once with docker volume create coinroost-pgdata" >&2
  exit 1
fi

# The image is built from the working tree: deploy a commit, so the version printed below is what runs.
if [[ -n $(git -C "$here/.." status --porcelain) ]]; then
  echo "the working tree has uncommitted changes: commit or stash them first" >&2
  exit 1
fi

"${compose[@]}" up -d --build --wait --wait-timeout 180

port=$(sed -n 's/^COINROOST_PORT=//p' "$env_file")
port=${port:-8094}
listening=$(ss -Hlnt "sport = :$port" | awk '{print $4}')
if [[ -z $listening ]] || grep -qv '^127\.0\.0\.1:' <<<"$listening"; then
  echo "port $port must listen on 127.0.0.1 alone; it listens on: ${listening:-nothing}" >&2
  exit 1
fi
curl -fsS "http://127.0.0.1:$port/api/v1/health" >/dev/null
echo "coinroost $(git -C "$here/.." rev-parse --short HEAD) is up on http://127.0.0.1:$port"
