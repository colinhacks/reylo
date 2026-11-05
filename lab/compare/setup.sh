#!/usr/bin/env bash
# Starts a throwaway Postgres 18 and MySQL 8.4 and loads the schema and seed rows into each, and into a SQLite file. Prints the URLs as shell assignments.
set -euo pipefail
cd "$(dirname "$0")"
PG_BIN="${PG18_BIN:-/opt/homebrew/opt/postgresql@18/bin}"
MYSQL_BIN="${MYSQL_BIN:-/opt/homebrew/opt/mysql@8.4/bin}"
PG_PORT="${COMPARE_PG_PORT:-57700}"
MY_PORT="${COMPARE_MYSQL_PORT:-57701}"
mkdir -p .out
LANG=en_US.UTF-8 ../../oracle/cluster.sh start "$PG_BIN" "$PG_PORT" > .out/pg.log 2>&1
"$PG_BIN/psql" -q "postgres://postgres@localhost:$PG_PORT/postgres" -c "drop database if exists compare" -c "create database compare"
PG_URL="postgres://postgres@localhost:$PG_PORT/compare"
"$PG_BIN/psql" -q -v ON_ERROR_STOP=1 "$PG_URL" -f schema/postgres.sql -f schema/postgres-seed.sql
D="/tmp/reylo-compare-mysql-$MY_PORT"
if ! "$MYSQL_BIN/mysqladmin" -uroot -S "$D/sock" ping >/dev/null 2>&1; then
  rm -rf "$D" && mkdir -p "$D"
  "$MYSQL_BIN/mysqld" --no-defaults --initialize-insecure --datadir="$D/data" --user="$USER"
  "$MYSQL_BIN/mysqld" --no-defaults --daemonize --datadir="$D/data" --socket="$D/sock" --port="$MY_PORT" --bind-address=127.0.0.1 --mysqlx=OFF --pid-file="$D/pid" --log-error="$D/err.log" --user="$USER"
fi
"$MYSQL_BIN/mysql" -uroot -S "$D/sock" -e "drop database if exists compare; create database compare"
"$MYSQL_BIN/mysql" -uroot -S "$D/sock" compare < schema/mysql.sql
"$MYSQL_BIN/mysql" -uroot -S "$D/sock" compare < schema/mysql-seed.sql
LITE="$PWD/.out/compare.db"
rm -f "$LITE"
sqlite3 "$LITE" < schema/sqlite.sql
sqlite3 "$LITE" < schema/sqlite-seed.sql
cat > .out/urls.env <<URLS
COMPARE_PG=$PG_URL
COMPARE_MYSQL=mysql://root@127.0.0.1:$MY_PORT/compare
COMPARE_SQLITE=$LITE
URLS
cat .out/urls.env
