#!/bin/sh
# Checks a reylo that a channel installed: that it is the version expected, and that it types a query
# on SQLite, a database the binary carries, so the check needs no server. Plain sh, so that it runs
# in every image channels.yml installs in, Alpine's included.
#
#   .github/scripts/channels-smoke.sh <reylo> <version>
set -eu

reylo="${1:?usage: channels-smoke.sh <reylo> <version>}"
version="${2:?usage: channels-smoke.sh <reylo> <version>}"

got="$("$reylo" --version)"
[ "$got" = "reylo $version" ] || { echo "channels-smoke: $reylo --version printed '$got', not 'reylo $version'" >&2; exit 1; }

# A relative path, which every platform's reylo reads the same way.
dir="$(mktemp -d)"
cd "$dir"
: > app.db
"$reylo" run "create table users (id integer primary key, email text not null) strict" --url sqlite:app.db
out="$("$reylo" analyze --sql "select 1 as one, email from users" --url sqlite:app.db --format json)"
echo "$out"
case "$out" in
  *'"cardinality":"many"'*'"name":"email"'*) ;;
  *) echo "channels-smoke: $reylo analyze did not type the query" >&2; exit 1 ;;
esac
cd /
rm -rf "$dir"
echo "channels-smoke: $got works"
