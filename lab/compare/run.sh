#!/usr/bin/env bash
# Starts the servers, runs every tool, writes results.json and RESULTS.md, and stops the servers. Extra arguments go to run.ts (--only <ids>).
set -euo pipefail
cd "$(dirname "$0")"
if [ ! -d node_modules ]; then env -u NODE_COMPAT nub install; fi
if [ -z "${SQLX_TS_BIN:-}" ] && [ ! -x .out/sqlx-ts/bin/sqlx-ts ]; then
  PATH="/opt/homebrew/opt/rustup/bin:$PATH" cargo install --locked --git https://github.com/JasonShin/sqlx-ts --tag v0.49.0 --root .out/sqlx-ts sqlx-ts
fi
./setup.sh
trap ./teardown.sh EXIT
env -u NODE_COMPAT node run.ts "$@"
