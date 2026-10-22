#!/bin/sh
# Build the five Postgres JSON Schema validators the oracle's json_schema_* cases call, each at the
# commit those cases were measured at, and stage their files under <stage> by the paths they install
# to: copying <stage> over / installs them. pg_jsonschema and jsonschema are pgrx extensions, each
# built with the cargo-pgrx version its Cargo.toml pins; the other three build with PGXS.
#
#   sh .github/scripts/json-schema-validators.sh <stage> <work> <pg_config>
#
# <work> holds the clones, cargo-pgrx and the build trees.
set -eu
stage=$(mkdir -p "$1" && cd "$1" && pwd)
work=$(mkdir -p "$2" && cd "$2" && pwd)
pg_config=$3
major=$("$pg_config" --version | sed -E 's/^PostgreSQL ([0-9]+).*/\1/')

clone() { # url dir commit
  [ -d "$2/.git" ] || git clone -q "$1" "$2"
  git -C "$2" checkout -q "$3"
}

pgrx_package() { # source dir, cargo-pgrx version
  dir=$1
  v=$2
  [ -x "$work/pgrx-$v/bin/cargo-pgrx" ] || cargo install cargo-pgrx --version "$v" --locked --root "$work/pgrx-$v"
  # A crate inside another Cargo workspace would be taken for one of its members: give it its own.
  grep -q '^\[workspace\]' "$dir/Cargo.toml" || printf '\n[workspace]\n' >> "$dir/Cargo.toml"
  (
    cd "$dir"
    export PATH="$work/pgrx-$v/bin:$PATH" PGRX_HOME="$work/pgrx-home-$v"
    cargo pgrx init "--pg$major" "$pg_config" > /dev/null
    cargo pgrx package --pg-config "$pg_config" --out-dir "$stage"
  )
}

# Supabase's pg_jsonschema
clone https://github.com/supabase/pg_jsonschema.git "$work/pg_jsonschema" v0.3.4
pgrx_package "$work/pg_jsonschema" 0.16.1

# jsonschema, on the boon crate (formerly Tembo's)
clone https://github.com/theory/pg-jsonschema-boon.git "$work/pg-jsonschema-boon" v0.1.10
pgrx_package "$work/pg-jsonschema-boon" 0.18.1

# postgres-json-schema (PL/pgSQL), is_jsonb_valid (C) and json_schema_validate (C)
clone https://github.com/gavinwahl/postgres-json-schema.git "$work/postgres-json-schema" e25c758
clone https://github.com/furstenheim/is_jsonb_valid.git "$work/is_jsonb_valid" 1f53675
clone https://github.com/adunstan/json_schema_validate.git "$work/json_schema_validate" f771374
for d in postgres-json-schema is_jsonb_valid json_schema_validate; do
  make -C "$work/$d" PG_CONFIG="$pg_config" DESTDIR="$stage" install > /dev/null
done
find "$stage" -type f | sort
