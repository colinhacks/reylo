#!/usr/bin/env bash
# Reylo's Rust crates as crates.io would serve them, used the way a project outside this repository
# uses them: the Rust counterpart of install-check.ts.
#
# 1. `cargo package` packs every published crate and builds each one from its own tarball, against
#    the tarballs of the others, so a crate that reads a file its package doesn't carry, or names
#    a dependency crates.io can't resolve, fails here rather than for whoever installs it.
# 2. New projects, each in a directory of its own, depend on `reylo` (and one on `reylo-build`) by
#    version alone, each of Reylo's crates resolved to its packed copy, and their statements are
#    typed at compile time against a database made from their schema, then run: one on sqlx 0.9,
#    the default, and one on sqlx 0.8 for each of Postgres and MySQL, which no workspace member can
#    be, since a workspace builds `reylo` once with every member's features.
#
# REYLO_TEST_PG names the Postgres server, and REYLO_TEST_MYSQL the MySQL one, whose schema is loaded
# with the client in MYSQL_CONTAINER, else the mysql on the PATH. Run it after
# `engine/build.sh --prepare`.
set -euo pipefail
pg=${REYLO_TEST_PG:?REYLO_TEST_PG names the Postgres server}
mysql=${REYLO_TEST_MYSQL:?REYLO_TEST_MYSQL names the MySQL server}
server=${pg%/*}
root=$(pwd)

# The published crates, each with its version, from the workspace's own manifests.
crates=$(cargo metadata --no-deps --format-version 1 --locked | python3 -c '
import json, sys
for p in json.load(sys.stdin)["packages"]:
    if p["publish"] is None:
        print(p["name"], p["version"])
')
echo "$crates"
flags=$(awk '{ printf "-p %s ", $1 }' <<<"$crates")

echo '::group::cargo package, each crate built from its tarball'
# The SQLite engine's package carries the amalgamation the prepare step writes into its ignored
# `.sqlite/`, and cargo counts an ignored file a package includes as uncommitted. So the tree git
# tracks must be clean, untracked files included, and `--allow-dirty` admits the prepared files alone.
if [ -n "$(git status --porcelain --untracked-files=all)" ]; then
  echo 'the working tree has changes git has not committed:' >&2
  git status --porcelain --untracked-files=all >&2
  exit 1
fi
# shellcheck disable=SC2086
cargo package --locked --allow-dirty $flags
echo '::endgroup::'

projects=$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/reylo-fresh.XXXXXX")
# The new projects build on the oldest Rust the crates declare, the workspace's rust-version, so that
# every run proves it, where this repository builds on the release rust-toolchain.toml pins. They sit
# outside the repository, where that file does not reach anyway.
floor=$(sed -n 's/^rust-version = "\(.*\)"$/\1/p' Cargo.toml)
rustup toolchain install "$floor" --profile minimal
export RUSTUP_TOOLCHAIN=$floor
cargo --version
trap 'rm -rf "$projects"' EXIT
# One target directory for every project, so that what they share compiles once.
export CARGO_TARGET_DIR="$projects/target"
version=$(awk '$1 == "reylo" { print $2 }' <<<"$crates")

# A project's manifest: its dependencies, then each of Reylo's crates patched to its packed copy.
manifest() {
  cat <<EOF
[package]
name = "$1"
version = "0.0.0"
edition = "2021"
publish = false

$2

# Each of Reylo's crates is the tarball cargo package wrote, as crates.io would serve it.
[patch.crates-io]
EOF
  while read -r name crate_version; do
    echo "$name = { path = \"$root/target/package/$name-$crate_version\" }"
  done <<<"$crates"
}

mysql_client() {
  if [ -n "${MYSQL_CONTAINER:-}" ]; then
    docker exec -i "$MYSQL_CONTAINER" mysql -uroot "$@"
  else
    local address=${mysql#*@}
    mysql -h "${address%:*}" -P "${address##*:}" -uroot "$@"
  fi
}

# A project's migrations, which its app runs with reylo::migrate!() on a database of their own: a
# table in one, a row in the next, since MySQL refuses DDL beside anything else in one migration.
migrations() {
  mkdir -p "$1/reylo/migrations"
  echo "create table notes (id int primary key, body $2 not null);" > "$1/reylo/migrations/20260101000000_notes.sql"
  echo "insert into notes (id, body) values (1, 'hello');" > "$1/reylo/migrations/20260102000000_hello.sql"
}

# What each project's app runs on that database, MIGRATE_URL: the deploy, again with nothing left to
# run, then the status, as `reylo migrate deploy` and `reylo migrate status` would.
migrate_main() {
  cat <<EOF
    let fresh = sqlx::${1}::connect(&std::env::var("MIGRATE_URL").expect("MIGRATE_URL")).await?;
    let ran = reylo::migrate!().run(&fresh).await?;
    assert_eq!(ran, ["20260101000000_notes.sql", "20260102000000_hello.sql"]);
    assert!(reylo::migrate!().run(&fresh).await?.is_empty());
    let status = reylo::migrate!().status(&fresh).await?;
    assert!(status.is_current(), "{status}");
    let body: String = sqlx::query_scalar("select body from notes where id = 1").fetch_one(&fresh).await?;
    assert_eq!(body, "hello");
EOF
}

# --- sqlx 0.9, the default, with a build script ---------------------------------------------------
project="$projects/fresh"
mkdir -p "$project/src" "$project/queries"
manifest fresh '[dependencies]
reylo = { version = "='"$version"'", features = ["postgres"] }
sqlx = { version = "0.9", default-features = false, features = ["postgres", "runtime-tokio"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }

[build-dependencies]
reylo-build = "='"$version"'"' > "$project/Cargo.toml"
cat > "$project/schema.sql" <<'EOF'
create table users (id int primary key, name text not null, nickname text);
create table posts (id serial primary key, author_id int not null references users (id), title text not null);
insert into users values (1, 'ada', 'countess'), (2, 'bob', null);
insert into posts (author_id, title) values (1, 'hello');
EOF
cat > "$project/queries/titles.sql" <<'EOF'
select u.name, p.title from users u left join posts p on p.author_id = u.id order by u.id
EOF
cat > "$project/build.rs" <<'EOF'
fn main() {
    reylo_build::rust_sqlx().input("queries").out("queries.rs").run();
}
EOF
migrations "$project" text
cat > "$project/src/main.rs" <<EOF
#[reylo::query("select id, name, nickname from users where id = :id")]
mod get_user {}

#[reylo::query("select count(*) as n from users")]
mod count_users {}

mod queries {
    include!(concat!(env!("OUT_DIR"), "/queries.rs"));
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
$(migrate_main PgPool)
    let pool = sqlx::PgPool::connect(&std::env::var("DATABASE_URL").expect("DATABASE_URL")).await?;
    let bob: Option<get_user::Row> = get_user::Query { id: 2 }.run(&pool).await?;
    let nickname: Option<String> = bob.expect("bob").nickname;
    assert_eq!(nickname, None);
    let n: i64 = count_users::Query.run(&pool).await?.n;
    assert_eq!(n, 2);
    let titles: Vec<queries::titles::Row> = queries::titles::Query.run(&pool).await?;
    let title: &Option<String> = &titles[1].title;
    assert_eq!(title, &None);
    println!("a project outside the workspace built Reylo from its packages, ran its queries and migrated a database");
    Ok(())
}
EOF
psql "$pg" -v ON_ERROR_STOP=1 -q -c 'drop database if exists reylo_fresh' -c 'create database reylo_fresh' -c 'drop database if exists reylo_fresh_migrate' -c 'create database reylo_fresh_migrate'
psql "$server/reylo_fresh" -v ON_ERROR_STOP=1 -q -f "$project/schema.sql"
echo '::group::a new project on sqlx 0.9, built from the packages and run'
(cd "$project" && DATABASE_URL="$server/reylo_fresh" MIGRATE_URL="$server/reylo_fresh_migrate" cargo run -q)
echo '::endgroup::'

# --- sqlx 0.8 on Postgres: a borrowed parameter's stream, an enum bound, an exact numeric ---------
project="$projects/fresh-sqlx08"
mkdir -p "$project/src"
manifest fresh-sqlx08 '[dependencies]
reylo = { version = "='"$version"'", default-features = false, features = ["sqlx_0_8", "postgres", "migrate", "rust_decimal", "chrono"] }
sqlx = { version = "0.8", default-features = false, features = ["postgres", "runtime-tokio", "rust_decimal", "chrono"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
futures-util = { version = "0.3", default-features = false }' > "$project/Cargo.toml"
cat > "$project/schema.sql" <<'EOF'
create table items (id int primary key, name text not null, status text not null check (status in ('draft', 'done')), price numeric, due date);
insert into items values (1, 'a', 'draft', 1.50, '2026-11-17'), (2, 'b', 'done', 1e40, 'infinity');
EOF
migrations "$project" text
cat > "$project/src/main.rs" <<EOF
use futures_util::StreamExt;

#[reylo::query("select id, status from items where name <> :name order by id")]
mod other_items {}

#[reylo::query("select id from items where status = :status")]
mod items_by_status {}

#[reylo::query("select price from items where id = :id")]
mod price {}

#[reylo::query("select due from items where id = :id")]
mod due {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
$(migrate_main PgPool)
    let pool = sqlx::PgPool::connect(&std::env::var("DATABASE_URL").expect("DATABASE_URL")).await?;
    // A text parameter the rows' stream borrows.
    let rows: Vec<other_items::Row> = other_items::Query { name: "a" }.stream(&pool).map(|row| row.expect("a row")).collect().await;
    assert_eq!(rows.len(), 1);
    // A value of the enum the CHECK lists, whose encoder writes into the buffer each sqlx version
    // names its own way, as the text a parameter compared with it takes.
    let done = items_by_status::Query { status: rows[0].status.label() }.run(&pool).await?;
    assert_eq!(done.len(), 1);
    let exact: Option<reylo::sqlx::types::Decimal> = price::Query { id: 1 }.run(&pool).await?.expect("a").price;
    assert_eq!(exact.map(|d| d.to_string()), Some("1.50".to_string()));
    // 10^40 is past a Decimal's range: an error, where sqlx 0.8's own decoder panics.
    assert!(price::Query { id: 2 }.run(&pool).await.is_err());
    // A date, read exactly, and infinity refused, as on sqlx 0.9.
    let date: Option<reylo::sqlx::types::chrono::NaiveDate> = due::Query { id: 1 }.run(&pool).await?.expect("a").due;
    assert_eq!(date.map(|d| d.to_string()), Some("2026-11-17".to_string()));
    assert!(due::Query { id: 2 }.run(&pool).await.is_err());
    println!("a project on sqlx 0.8 built Reylo from its packages, ran its Postgres queries and migrated a database");
    Ok(())
}
EOF
psql "$pg" -v ON_ERROR_STOP=1 -q -c 'drop database if exists reylo_fresh_sqlx08' -c 'create database reylo_fresh_sqlx08' -c 'drop database if exists reylo_fresh_sqlx08_migrate' -c 'create database reylo_fresh_sqlx08_migrate'
psql "$server/reylo_fresh_sqlx08" -v ON_ERROR_STOP=1 -q -f "$project/schema.sql"
echo '::group::a new project on sqlx 0.8 and Postgres'
(cd "$project" && DATABASE_URL="$server/reylo_fresh_sqlx08" MIGRATE_URL="$server/reylo_fresh_sqlx08_migrate" cargo run -q)
echo '::endgroup::'

# --- sqlx 0.8 on MySQL: text with a binary collation ----------------------------------------------
project="$projects/fresh-sqlx08-mysql"
mkdir -p "$project/src"
manifest fresh-sqlx08-mysql '[dependencies]
reylo = { version = "='"$version"'", default-features = false, features = ["sqlx_0_8", "mysql", "migrate"] }
sqlx = { version = "0.8", default-features = false, features = ["mysql", "runtime-tokio"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }' > "$project/Cargo.toml"
cat > "$project/schema.sql" <<'EOF'
create table words (id int primary key, word varchar(20) collate utf8mb4_bin not null);
insert into words values (1, 'Hello');
EOF
migrations "$project" 'varchar(20)'
cat > "$project/src/main.rs" <<EOF
#[reylo::query("select word from words where id = :id")]
mod word {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
$(migrate_main MySqlPool)
    let pool = sqlx::MySqlPool::connect(&std::env::var("DATABASE_URL").expect("DATABASE_URL")).await?;
    // sqlx 0.8 reads no text of a binary collation into a String; Reylo's decoder does.
    let word: String = word::Query { id: 1 }.run(&pool).await?.expect("hello").word;
    assert_eq!(word, "Hello");
    println!("a project on sqlx 0.8 built Reylo from its packages, ran its MySQL queries and migrated a database");
    Ok(())
}
EOF
mysql_client -e 'drop database if exists reylo_fresh_sqlx08; create database reylo_fresh_sqlx08; drop database if exists reylo_fresh_sqlx08_migrate; create database reylo_fresh_sqlx08_migrate'
mysql_client reylo_fresh_sqlx08 < "$project/schema.sql"
echo '::group::a new project on sqlx 0.8 and MySQL'
(cd "$project" && DATABASE_URL="$mysql/reylo_fresh_sqlx08" MIGRATE_URL="$mysql/reylo_fresh_sqlx08_migrate" cargo run -q)
echo '::endgroup::'
