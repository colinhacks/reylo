#!/usr/bin/env bash
# Stops the servers setup.sh started.
set -uo pipefail
cd "$(dirname "$0")"
MYSQL_BIN="${MYSQL_BIN:-/opt/homebrew/opt/mysql@8.4/bin}"
PG_PORT="${COMPARE_PG_PORT:-57700}"
MY_PORT="${COMPARE_MYSQL_PORT:-57701}"
../../oracle/cluster.sh stop "$PG_PORT"
D="/tmp/reylo-compare-mysql-$MY_PORT"
"$MYSQL_BIN/mysqladmin" -uroot -S "$D/sock" shutdown && rm -rf "$D"
