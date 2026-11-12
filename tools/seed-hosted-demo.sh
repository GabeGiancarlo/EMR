#!/usr/bin/env bash
# Fill the hosted demonstration database with synthetic records, in one step.
#
# It asks for the hosted database's EXTERNAL connection string without showing it,
# starts a local-mode API on this machine pointed at that database, runs
# tools/seed-demo.py against it, and stops the API. The connection string is never
# printed, written to a file, or put on a command line.
#
# Usage, from the repository root on an up-to-date main checkout:
#   tools/seed-hosted-demo.sh you@example.com [role=email ...]
#
# The first argument is the email that will sign in as the workspace administrator.
# Each further argument gives another role a sign-in email, for example
# clinician=someone@example.com. See docs/deployment/demo-hosting.md, step 4.
set -euo pipefail

if [ "$#" -lt 1 ]; then
  echo "Usage: tools/seed-hosted-demo.sh ADMIN_EMAIL [role=email ...]" >&2
  exit 2
fi
admin_email="$1"
shift
staff_args=()
for pair in "$@"; do
  staff_args+=(--staff "$pair")
done

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
port=8095
mkdir -p .local
log=".local/seed-hosted-api.log"
journal=".local/demo-seed-hosted.json"

branch="$(git rev-parse --abbrev-ref HEAD)"
echo "Seeding from branch ${branch} at $(git rev-parse --short HEAD)."
if [ "$branch" != "main" ]; then
  echo "Note: the local API applies this checkout's migrations to the hosted database."
  echo "      Seed from the commit that is deployed (normally main)."
fi

printf "Paste the hosted database's EXTERNAL connection string, then press Return (it is not shown): "
IFS= read -r -s database_url
echo
# A terminal with bracketed paste wraps pasted text in ESC[200~ ... ESC[201~. A hidden
# read receives those markers as part of the text, so remove them, and any stray
# carriage return or surrounding spaces, before looking at what was pasted.
database_url="${database_url#$'\e[200~'}"
database_url="${database_url%$'\e[201~'}"
database_url="${database_url//$'\r'/}"
database_url="${database_url#"${database_url%%[![:space:]]*}"}"
database_url="${database_url%"${database_url##*[![:space:]]}"}"
if [ -z "$database_url" ]; then
  echo "Nothing was pasted. Nothing was changed." >&2
  exit 1
fi
case "$database_url" in
  postgres://*|postgresql://*) ;;
  *) echo "That does not look like a PostgreSQL connection string. Nothing was changed." >&2; exit 1 ;;
esac
# Never the development databases on this machine.
host_part="${database_url#*@}"
host_part="${host_part%%/*}"
case "$host_part" in
  127.*|localhost*|\[::1\]*|0.0.0.0*)
    echo "That connection string points at this machine. This script is for the hosted database only. Nothing was changed." >&2
    exit 1 ;;
esac
case "$database_url" in
  *sslmode=*) ;;
  *\?*) database_url="${database_url}&sslmode=require" ;;
  *) database_url="${database_url}?sslmode=require" ;;
esac
echo "Target database host: ${host_part%%:*}"

if lsof -nP -iTCP:"$port" -sTCP:LISTEN >/dev/null 2>&1; then
  echo "Port ${port} is already in use on this machine. Stop whatever is using it and run this again." >&2
  exit 1
fi

echo "Building and starting a local-mode API on 127.0.0.1:${port} (the first build can take a few minutes)..."
NEXUS_LOCAL_DEV=1 HOST=127.0.0.1 PORT="$port" RUST_LOG=warn DATABASE_URL="$database_url" \
  cargo run --quiet -p emr-api >"$log" 2>&1 &
api_pid=$!
unset database_url
stop_api() {
  if kill -0 "$api_pid" 2>/dev/null; then
    kill "$api_pid" 2>/dev/null || true
    wait "$api_pid" 2>/dev/null || true
  fi
}
trap stop_api EXIT

ready=""
for _ in $(seq 1 600); do
  if ! kill -0 "$api_pid" 2>/dev/null; then
    echo "The local API stopped before it was ready. Its log, ${log}, says:" >&2
    tail -n 5 "$log" >&2
    exit 1
  fi
  if curl -fsS "http://127.0.0.1:${port}/healthz" >/dev/null 2>&1; then
    ready=1
    break
  fi
  sleep 1
done
if [ -z "$ready" ]; then
  echo "The local API did not become ready within ten minutes. See ${log}." >&2
  exit 1
fi

echo "Seeding synthetic records as ${admin_email}..."
python3 tools/seed-demo.py --api "http://127.0.0.1:${port}" --admin-email "$admin_email" \
  ${staff_args[@]+"${staff_args[@]}"} --journal "$journal" --confirm-synthetic-demo

echo "Done. The local API has been stopped; the hosted database now holds the demonstration workspace."
