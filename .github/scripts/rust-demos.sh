#!/usr/bin/env bash
# The Rust crate's demos (rust/demo-*), built and run on the CI job's databases: each statement typed
# by #[reylo::query] as the crate compiles, against a database made from the demo's schema, then the
# modules reylo generate and reylo-build write, compiled and run, then a misspelt column in a
# statement of each, which must fail the build at its file, line and column. A demo builds only with
# its `live` feature, so the workspace's own build, which has no database, leaves them out.
#
# REYLO_TEST_PG and REYLO_TEST_MYSQL name the servers, as for the migrations' tests. The MySQL schema
# is loaded with the client in MYSQL_CONTAINER, the service's container, else the mysql on the PATH.
# The reylo binary is the workspace's debug build, target/debug/reylo.
set -euo pipefail
pg=${REYLO_TEST_PG:?REYLO_TEST_PG names the Postgres server}
mysql=${REYLO_TEST_MYSQL:?REYLO_TEST_MYSQL names the MySQL server}
server=${pg%/*}

psql "$pg" -v ON_ERROR_STOP=1 -q -c 'drop database if exists rust_demo' -c 'create database rust_demo' -c 'drop database if exists rust_docs' -c 'create database rust_docs' -c 'drop database if exists rust_docs_migrate' -c 'create database rust_docs_migrate'
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
DATABASE_URL="$server/rust_docs" MIGRATE_URL="$server/rust_docs_migrate" cargo run -q -p demo-docs --features live
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
query=rust/demo-codegen/src/queries/get_user.sql
cp "$query" "${RUNNER_TEMP:-/tmp}/get_user.sql"
# A checkout that ends its lines in CRLF, as git does on Windows, compiles the same modules.
sed 's/$/\r/' "${RUNNER_TEMP:-/tmp}/get_user.sql" > "$query"
DATABASE_URL="$server/rust_demo" cargo build -q -p demo-codegen --features live
# An edited query fails to compile until its module is written again.
cp "${RUNNER_TEMP:-/tmp}/get_user.sql" "$query"
echo '-- edited' >> "$query"
if DATABASE_URL="$server/rust_demo" cargo build -q -p demo-codegen --features live 2> "${RUNNER_TEMP:-/tmp}/stale.log"; then
  echo '::error::demo-codegen compiled with a query edited after its module was written'
  exit 1
fi
grep -q 'get_user.sql changed after its module was written' "${RUNNER_TEMP:-/tmp}/stale.log" || { cat "${RUNNER_TEMP:-/tmp}/stale.log"; exit 1; }
mv "${RUNNER_TEMP:-/tmp}/get_user.sql" "$query"
echo '::endgroup::'

echo '::group::a refused statement fails the build where it is written'
# The macro's error names the line and column inside the string, since rustc underlines the whole string.
queries=rust/demo-reylo/src/queries.rs
cp "$queries" "${RUNNER_TEMP:-/tmp}/queries.rs"
lines=$(wc -l < "$queries")
printf '\n#[reylo::query("\n    select id,\n        nme from users\n")]\npub mod misspelt {}\n' >> "$queries"
# A misspelt reflect names what the database said, and one in a form it does not take keeps its enum,
# so that the enum's uses fail with nothing of their own.
printf '\n#[reylo::reflect("posts.stauts")]\npub enum Misspelt {}\n\n#[reylo::reflect(column = "posts.status")]\npub enum OldForm {}\n\n#[allow(dead_code)]\nfn uses(_: OldForm) {}\n' >> "$queries"
refused=0
DATABASE_URL="$server/rust_demo" cargo build -q -p demo-reylo --features live 2> "${RUNNER_TEMP:-/tmp}/refused.log" || refused=$?
mv "${RUNNER_TEMP:-/tmp}/queries.rs" "$queries"
cat "${RUNNER_TEMP:-/tmp}/refused.log"
[ "$refused" -ne 0 ] || { echo '::error::demo-reylo compiled with a misspelt column'; exit 1; }
grep -q "column \"nme\" does not exist (42703)" "${RUNNER_TEMP:-/tmp}/refused.log" || { echo "::error::the error is not the database's own"; exit 1; }
grep -q "at $queries:$((lines + 4)):9\$" "${RUNNER_TEMP:-/tmp}/refused.log" || { echo "::error::the error does not name $queries:$((lines + 4)):9"; exit 1; }
grep -q 'as a column: column "stauts" does not exist' "${RUNNER_TEMP:-/tmp}/refused.log" || { echo "::error::a misspelt reflect does not say what the database said"; exit 1; }
if grep -q 'cannot find type `OldForm`' "${RUNNER_TEMP:-/tmp}/refused.log"; then
  echo '::error::a reflect in a form it does not take left its enum out'
  exit 1
fi
# A build script's statement fails with Cargo's own error at its file, line and column, not a panic.
report=rust/demo-codegen/src/reports/post_counts.sql
cp "$report" "${RUNNER_TEMP:-/tmp}/post_counts.sql"
sed 's/max(u\.joined)/max(u.joind)/' "${RUNNER_TEMP:-/tmp}/post_counts.sql" > "$report"
refused=0
DATABASE_URL="$server/rust_demo" cargo build -q -p demo-codegen --features live 2> "${RUNNER_TEMP:-/tmp}/build-script.log" || refused=$?
mv "${RUNNER_TEMP:-/tmp}/post_counts.sql" "$report"
cat "${RUNNER_TEMP:-/tmp}/build-script.log"
[ "$refused" -ne 0 ] || { echo '::error::demo-codegen built with a misspelt column in its build script'"'"'s query'; exit 1; }
grep -q '^error: demo-codegen@0.0.0: src/reports/post_counts.sql:1:42: column u.joind does not exist (42703)$' "${RUNNER_TEMP:-/tmp}/build-script.log" || { echo '::error::the build script'"'"'s error is not a Cargo error at post_counts.sql:1:42'; exit 1; }
if grep -q 'panicked' "${RUNNER_TEMP:-/tmp}/build-script.log"; then
  echo '::error::the build script panicked'
  exit 1
fi
echo '::endgroup::'

echo '::group::a cast into an enum that cannot hold the value fails to compile'
# A list of three values into one of two, and a Postgres enum type into a list: each a type error,
# which cargo check and the editor see, not one that waits for the code to be generated.
cp "$queries" "${RUNNER_TEMP:-/tmp}/queries.rs"
printf '\n#[allow(dead_code)]\nfn narrower(state: set_task_state::State) -> planned_states::State {\n    reylo::Enum::cast(state)\n}\n\n#[allow(dead_code)]\nfn into_a_list(mood: users_by_mood::Mood) -> planned_states::Mood {\n    reylo::Enum::cast(mood)\n}\n' >> "$queries"
refused=0
DATABASE_URL="$server/rust_demo" cargo check -q -p demo-reylo --features live 2> "${RUNNER_TEMP:-/tmp}/casts.log" || refused=$?
mv "${RUNNER_TEMP:-/tmp}/queries.rs" "$queries"
cat "${RUNNER_TEMP:-/tmp}/casts.log"
[ "$refused" -ne 0 ] || { echo '::error::demo-reylo compiled with a cast into an enum that cannot hold the value'; exit 1; }
grep -q 'planned_states::State` has no variant for one of the values of the enum cast into it' "${RUNNER_TEMP:-/tmp}/casts.log" || { echo '::error::a cast of a list into an enum lacking one of its values does not say so'; exit 1; }
grep -qE 'error\[E0271\]: type mismatch resolving `<[^`]*Mood as [^`]*Enum>::Id == [^`]*EnumId<' "${RUNNER_TEMP:-/tmp}/casts.log" || { echo '::error::a cast of a Postgres enum type into a list does not fail on the type'; exit 1; }
echo '::endgroup::'
