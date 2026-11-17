#!/usr/bin/env bash
# Publishes the Rust crates to crates.io at a release's version, from its tagged commit, in the order
# they depend on each other: each version once, so a run cut off part of the way is run again as it
# is. The token is CARGO_REGISTRY_TOKEN, which cargo reads: in release.yml a token crates.io's trusted
# publishing gives the run (this repository, this workflow), which lasts 30 minutes; for a crate's
# first version, which no trusted publisher can make, a token of a maintainer's own, from their
# machine, with `--trust`, which then gives each crate its trusted publisher: release.yml in its
# `crates` environment, which only `main` can deploy to, so every later version publishes from the
# release and a workflow of that name on any other branch publishes nothing.
#
#   .github/scripts/crates-publish.sh [--trust] <version>
#
# Run it from the repository's root after `engine/build.sh --prepare`. `--no-verify`: the release's
# test run builds every crate from its package (test.yml's package job). `--allow-dirty`: the SQLite
# engine and the Postgres parser carry files the prepare step writes, which git ignores.
#
# crates.io lets an account publish 5 new crates at once and then one every 10 minutes, and new
# versions of existing crates 30 at once and then one a minute: a refusal for the rate is waited out
# and tried again.
set -euo pipefail
trust=false
if [ "${1:-}" = --trust ]; then
  trust=true
  shift
fi
version=${1:?the version to publish, as the tag names it without its v}
: "${CARGO_REGISTRY_TOKEN:?CARGO_REGISTRY_TOKEN names the crates.io token}"
agent="reylo-release (github.com/reylosql/reylo)"
# Each crate after those it depends on. The parser crates keep their own versions, which change only
# with what they copy.
crates=(reylo-pg-query reylo-sqlparser reylo-engine-build reylo-engine-core reylo-engine-postgres reylo-engine-sqlite reylo-engine-mysql reylo-engine reylo-migrate reylo-build reylo-macros reylo reylo-cli)

versions=$(cargo metadata --no-deps --format-version 1 --locked | python3 -c '
import json, sys
for p in json.load(sys.stdin)["packages"]:
    if p["publish"] is None:
        print(p["name"], p["version"])
')
listed=$(awk '{ print $1 }' <<<"$versions" | sort)
if [ "$listed" != "$(printf '%s\n' "${crates[@]}" | sort)" ]; then
  echo "the workspace publishes other crates than this script lists:" >&2
  echo "$versions" >&2
  exit 1
fi

# Every crate at the release's version before any is published, the parser crates aside.
for crate in "${crates[@]}"; do
  crate_version=$(awk -v c="$crate" '$1 == c { print $2 }' <<<"$versions")
  case "$crate" in
    reylo-pg-query | reylo-sqlparser) ;;
    *)
      if [ "$crate_version" != "$version" ]; then
        echo "$crate is at $crate_version in this checkout, not $version" >&2
        exit 1
      fi
      ;;
  esac
done

# Whether crates.io holds a crate's version: only its 404 is a no, and any other answer is tried
# again, and stops the run if it lasts.
published() {
  local status
  for _ in 1 2 3 4 5; do
    status=$(curl -s -A "$agent" -o /dev/null -w '%{http_code}' "https://crates.io/api/v1/crates/$1/$2" || true)
    case "$status" in
      200) echo yes; return ;;
      404) echo no; return ;;
    esac
    echo "crates.io answered $status for $1 $2; trying again in 30 seconds" >&2
    sleep 30
  done
  echo "crates.io did not say whether $1 $2 is published" >&2
  exit 1
}

# The token goes to curl on its standard input, never on a command line.
api() {
  printf 'header = "Authorization: %s"\n' "$CARGO_REGISTRY_TOKEN" | curl -sS --config - -A "$agent" "$@"
}

for crate in "${crates[@]}"; do
  crate_version=$(awk -v c="$crate" '$1 == c { print $2 }' <<<"$versions")
  if [ "$(published "$crate" "$crate_version")" = yes ]; then
    echo "$crate $crate_version is on crates.io already"
  else
    echo "publishing $crate $crate_version"
    until out=$(cargo publish -p "$crate" --locked --no-verify --allow-dirty 2>&1); do
      if grep -qiE "too many (new crates|updates)|429" <<<"$out"; then
        echo "  crates.io's rate: trying again in 2 minutes"
        sleep 120
        continue
      fi
      echo "$out" >&2
      exit 1
    done
  fi
  if $trust; then
    existing=$(api "https://crates.io/api/v1/trusted_publishing/github_configs?crate=$crate")
    if python3 -c 'import json, sys; sys.exit(0 if any(c["workflow_filename"] == "release.yml" and c["environment"] == "crates" for c in json.load(sys.stdin)["github_configs"]) else 1)' <<<"$existing"; then
      echo "  release.yml publishes $crate already"
    else
      body=$(printf '{"github_config":{"crate":"%s","repository_owner":"reylosql","repository_name":"reylo","workflow_filename":"release.yml","environment":"crates"}}' "$crate")
      answer=$(api -X POST -H "Content-Type: application/json" --data "$body" https://crates.io/api/v1/trusted_publishing/github_configs)
      if ! grep -q '"github_config"' <<<"$answer"; then
        echo "crates.io refused release.yml as $crate's trusted publisher: $answer" >&2
        exit 1
      fi
      echo "  release.yml publishes $crate from now on"
    fi
  fi
done
echo "every crate is on crates.io at $version"
