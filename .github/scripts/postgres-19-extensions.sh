#!/bin/sh
# Build the extensions the oracle's Postgres cases and the loader's tests call, for Postgres 19, and stage their
# files under <stage> by the paths they install to: copying <stage> over / installs them. PGDG packages pgvector and
# PostGIS for 18 and not yet for 19, so both are built from their releases here, and so are the five JSON Schema
# validators of .github/scripts/json-schema-validators.sh. The two pgrx ones come from the newest commit of each that
# builds on Postgres 19, with cargo-pgrx 0.19.3, the first that knows 19; their releases pin older pgrx versions.
#
#   sh .github/scripts/postgres-19-extensions.sh <stage> <work> <pg_config>
#
# <work> holds the sources, cargo-pgrx and the build trees.
set -eu
stage=$(mkdir -p "$1" && cd "$1" && pwd)
work=$(mkdir -p "$2" && cd "$2" && pwd)
pg_config=$3
"$pg_config" --version | grep -q '^PostgreSQL 19' || { echo "$pg_config is not Postgres 19" >&2; exit 1; }

clone() { # url dir commit
  [ -d "$2/.git" ] || git clone -q "$1" "$2"
  git -C "$2" checkout -q "$3"
}

# pgvector, for the pgvector_types case and the spec's example of a vector column. Its default `-march=native`
# would tie the cached build to the runner's CPU, and a runner without its instructions crashes the server.
curl -fsSL https://github.com/pgvector/pgvector/archive/refs/tags/v0.8.7.tar.gz | tar xz -C "$work"
make -C "$work/pgvector-0.8.7" PG_CONFIG="$pg_config" OPTFLAGS="" > /dev/null
make -C "$work/pgvector-0.8.7" PG_CONFIG="$pg_config" OPTFLAGS="" DESTDIR="$stage" install > /dev/null

# PostGIS 3.7.0, the first release that builds on Postgres 19, with the extension alone: no raster or topology
curl -fsSL https://download.osgeo.org/postgis/source/postgis-3.7.0.tar.gz | tar xz -C "$work"
(
  cd "$work/postgis-3.7.0"
  ./configure --with-pgconfig="$pg_config" --without-protobuf --without-raster --without-topology > /dev/null
  # The whole tree's make also writes topology's upgrade script, which fails with no topology built, so the
  # libraries, the extension and its scripts are built by their own targets.
  for d in deps liblwgeom libpgcommon postgis; do [ ! -f "$d/Makefile" ] || make -C "$d" -j"$(nproc)" > /dev/null; done
  make -C extensions postgis_extension_helper.sql > /dev/null
  make -C postgis DESTDIR="$stage" install > /dev/null
  make -C extensions/postgis DESTDIR="$stage" install > /dev/null
)

# postgres-json-schema (PL/pgSQL), is_jsonb_valid (C) and json_schema_validate (C), at the commits the cases were measured at
clone https://github.com/gavinwahl/postgres-json-schema.git "$work/postgres-json-schema" e25c758
clone https://github.com/furstenheim/is_jsonb_valid.git "$work/is_jsonb_valid" 1f53675
clone https://github.com/adunstan/json_schema_validate.git "$work/json_schema_validate" f771374
for d in postgres-json-schema is_jsonb_valid json_schema_validate; do
  make -C "$work/$d" PG_CONFIG="$pg_config" DESTDIR="$stage" install > /dev/null
done

# pg_jsonschema and jsonschema (on the boon crate), each built with cargo-pgrx 0.19.3 and a pg19 feature
pgrx=$work/pgrx-0.19.3
[ -x "$pgrx/bin/cargo-pgrx" ] || cargo install cargo-pgrx --version 0.19.3 --locked --root "$pgrx"
export PATH="$pgrx/bin:$PATH" PGRX_HOME="$work/pgrx-home"
cargo pgrx init --pg19 "$pg_config" > /dev/null
pgrx_package() { # dir
  python3 - "$1/Cargo.toml" <<'PY'
import re, sys
path = sys.argv[1]
s = open(path).read()
s = re.sub(r'(pgrx(?:-tests)?\s*=\s*)"[^"]*"', r'\1"=0.19.3"', s)
s = re.sub(r'(pgrx(?:-tests)?\s*=\s*\{[^}]*version\s*=\s*)"[^"]*"', r'\1"=0.19.3"', s)
if "pg19" not in s:
    s = re.sub(r'(pg18\s*=\s*\[[^\]]*\]\n)', lambda m: m.group(1) + m.group(1).replace("pg18", "pg19"), s, count=1)
# A crate inside another Cargo workspace would be taken for one of its members: give it its own.
if not re.search(r'^\[workspace\]', s, re.M):
    s += "\n[workspace]\n"
open(path, "w").write(s)
PY
  rm -f "$1/Cargo.lock"
  (cd "$1" && cargo pgrx package --pg-config "$pg_config" --out-dir "$stage" --no-default-features --features pg19)
}
clone https://github.com/supabase/pg_jsonschema.git "$work/pg_jsonschema" d051cb0
pgrx_package "$work/pg_jsonschema"
clone https://github.com/theory/pg-jsonschema-boon.git "$work/pg-jsonschema-boon" 400ddfe
pgrx_package "$work/pg-jsonschema-boon"

find "$stage" -type f | sort
