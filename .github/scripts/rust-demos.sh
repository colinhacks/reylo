#!/usr/bin/env bash
# The Rust crate's demos (rust/demo-*), built and run on the CI job's databases: each statement typed
# by #[reylo::query] as the crate compiles, against a database made from the demo's schema, then the
# modules reylo generate and reylo-build write, compiled and run. A demo builds only with its `live`
# feature, so the workspace's own build, which has no database, leaves them out.
#
# REYLO_TEST_PG and REYLO_TEST_MYSQL name the servers, as for the migrations' tests. The MySQL schema
# is loaded with the client in MYSQL_CONTAINER, the service's container, else the mysql on the PATH.
# The reylo binary is the workspace's debug build, target/debug/reylo.
set -euo pipefail
pg=${REYLO_TEST_PG:?REYLO_TEST_PG names the Postgres server}
mysql=${REYLO_TEST_MYSQL:?REYLO_TEST_MYSQL names the MySQL server}
server=${pg%/*}

psql "$pg" -v ON_ERROR_STOP=1 -q -c 'drop database if exists rust_demo' -c 'create database rust_demo' -c 'drop database if exists rust_docs' -c 'create database rust_docs'
psql "$server/rust_demo" -v ON_ERROR_STOP=1 -q -f rust/demo-reylo/schema.sql
psql "$server/rust_docs" -v ON_ERROR_STOP=1 -q -f rust/demo-docs/schema.sql
mysql_client() {
  if [ -n "${MYSQL_CONTAINER:-}" ]; then
    docker exec -i "$MYSQL_CONTAINER" mysql -uroot "$@"
  else
    local address=${mysql#*@}
    mysql -h "${address%:*}" -P "${address##*:}" -uroot "$@"
  fi
}
mysql_client -e 'drop database if exists rust_demo; create database rust_demo'
mysql_client rust_demo < rust/demo-mysql/schema.sql
rm -f rust/demo-sqlite/demo.db
python3 -c 'import sqlite3, sys; sqlite3.connect(sys.argv[1]).executescript(open(sys.argv[2]).read())' rust/demo-sqlite/demo.db rust/demo-sqlite/schema.sql

echo '::group::demo-reylo, on Postgres'
DATABASE_URL="$server/rust_demo" cargo run -q -p demo-reylo --features live
echo '::endgroup::'
echo '::group::demo-docs, the docs site'"'"'s snippets'
DATABASE_URL="$server/rust_docs" cargo run -q -p demo-docs --features live
echo '::endgroup::'
echo '::group::demo-mysql, on MySQL'
DATABASE_URL="$mysql/rust_demo" cargo run -q -p demo-mysql --features live
echo '::endgroup::'
echo '::group::demo-sqlite, on SQLite'
DATABASE_URL=sqlite:demo.db cargo run -q -p demo-sqlite --features live
echo '::endgroup::'
echo '::group::demo-codegen, reylo generate and reylo-build'
(cd rust/demo-codegen && DATABASE_URL="$server/rust_demo" ../../target/debug/reylo generate rust-sqlx src/queries)
DATABASE_URL="$server/rust_demo" cargo run -q -p demo-codegen --features live
# An edited query fails to compile until its module is written again.
query=rust/demo-codegen/src/queries/get_user.sql
cp "$query" "${RUNNER_TEMP:-/tmp}/get_user.sql"
echo '-- edited' >> "$query"
if DATABASE_URL="$server/rust_demo" cargo build -q -p demo-codegen --features live 2> "${RUNNER_TEMP:-/tmp}/stale.log"; then
  echo '::error::demo-codegen compiled with a query edited after its module was written'
  exit 1
fi
grep -q 'get_user.sql changed after its module was written' "${RUNNER_TEMP:-/tmp}/stale.log" || { cat "${RUNNER_TEMP:-/tmp}/stale.log"; exit 1; }
mv "${RUNNER_TEMP:-/tmp}/get_user.sql" "$query"
echo '::endgroup::'
