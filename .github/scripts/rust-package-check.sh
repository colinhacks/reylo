#!/usr/bin/env bash
# Reylo's Rust crates as crates.io would serve them, used the way a project outside this repository
# uses them: the Rust counterpart of install-check.ts.
#
# 1. `cargo package` packs every published crate and builds each one from its own tarball, against
#    the tarballs of the others, so a crate that reads a file its package doesn't carry, or names
#    a dependency crates.io can't resolve, fails here rather than for whoever installs it.
# 2. A new project in a directory of its own depends on `reylo` and `reylo-build` by version alone,
#    each crate resolved to its packed copy, and its statements are typed at compile time against
#    a database made from its schema, by the macro and by a build script, then run.
#
# REYLO_TEST_PG names the Postgres server. Run it after `engine/build.sh --prepare`.
set -euo pipefail
pg=${REYLO_TEST_PG:?REYLO_TEST_PG names the Postgres server}
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

project=$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/reylo-fresh.XXXXXX")
trap 'rm -rf "$project"' EXIT
version=$(awk '$1 == "reylo" { print $2 }' <<<"$crates")
mkdir -p "$project/src" "$project/queries"
{
  cat <<EOF
[package]
name = "fresh"
version = "0.0.0"
edition = "2021"
publish = false

[dependencies]
reylo = { version = "=$version", features = ["postgres"] }
sqlx = { version = "0.9", default-features = false, features = ["postgres", "runtime-tokio"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }

[build-dependencies]
reylo-build = "=$version"

# Each of Reylo's crates is the tarball cargo package wrote, as crates.io would serve it.
[patch.crates-io]
EOF
  while read -r name crate_version; do
    echo "$name = { path = \"$root/target/package/$name-$crate_version\" }"
  done <<<"$crates"
} > "$project/Cargo.toml"

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
cat > "$project/src/main.rs" <<'EOF'
#[reylo::query("select id, name, nickname from users where id = :id")]
mod get_user {}

#[reylo::query("select count(*) as n from users")]
mod count_users {}

mod queries {
    include!(concat!(env!("OUT_DIR"), "/queries.rs"));
}

#[tokio::main]
async fn main() -> Result<(), sqlx::Error> {
    let pool = sqlx::PgPool::connect(&std::env::var("DATABASE_URL").expect("DATABASE_URL")).await?;
    let bob: Option<get_user::Row> = get_user::Query { id: 2 }.run(&pool).await?;
    let nickname: Option<String> = bob.expect("bob").nickname;
    assert_eq!(nickname, None);
    let n: i64 = count_users::Query.run(&pool).await?.n;
    assert_eq!(n, 2);
    let titles: Vec<queries::titles::Row> = queries::titles::Query.run(&pool).await?;
    let title: &Option<String> = &titles[1].title;
    assert_eq!(title, &None);
    println!("a project outside the workspace built Reylo from its packages and ran its queries");
    Ok(())
}
EOF

psql "$pg" -v ON_ERROR_STOP=1 -q -c 'drop database if exists reylo_fresh' -c 'create database reylo_fresh'
psql "$server/reylo_fresh" -v ON_ERROR_STOP=1 -q -f "$project/schema.sql"
echo '::group::a new project, built from the packages and run'
(cd "$project" && DATABASE_URL="$server/reylo_fresh" cargo run -q)
echo '::endgroup::'
