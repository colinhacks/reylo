#!/usr/bin/env bash
# Builds each database's engine as one WebAssembly component (`wasm32-wasip2`, from wasip2/): the
# engine and the program that connects it to its database (native/), behind the exports `call` and
# `serve`, transpiled by jco into its npm package's `engine/` directory, which the loader runs
# through jco's preview2-shim (src/engine.ts). One artifact for every platform: the connections it
# opens are WASI sockets, which the shim opens as Node's.
#
#   engine/build.sh --prepare            the inputs below, and nothing else
#   engine/build.sh                      every database's component
#   engine/build.sh postgres [sqlite …]  the components named
#
# Inputs: rustup's `wasm32-wasip2` target, wasi-sdk 34 (set WASI_SDK_PATH, or unpack it under
# `.wasi-sdk/`) and jco from the root's install. The `pg_query` crate is the Postgres 18 parser:
# upstream's `pg-18` branch (pganalyze/pg_query.rs, pull request 79, "Release 18.0.0", not on
# crates.io yet) at the commit `postgres/vendor/pg_query-source.sh` pins, with the libpg_query commit
# its submodule names (tag 18.1.0, the grammar of Postgres 18.6), each fetched from GitHub by commit
# and checked against a SHA-256, and the patch from `postgres/vendor/` applied into
# `postgres/.patched/pg_query`. SQLite is the 3.53.4 amalgamation fetched into `sqlite/.sqlite/`
# with the hooks of `sqlite/csrc/patch-sqlite.py` applied. tokio-postgres 0.7.18 is fetched from
# crates.io into `wasip2/.patched/` with the patch beside it, which builds its runtime for wasip2.
# `./build.sh --prepare` stops after that, which is what a native `cargo test` needs, and what any
# cargo command in the workspace needs first (the workspace `[patch]` points at the copy).
set -euo pipefail
cd "$(dirname "$0")"
DATABASES=(postgres sqlite mysql duckdb)

SQLITE_ZIP="sqlite-amalgamation-3530400.zip"
SQLITE_URL="https://sqlite.org/2026/$SQLITE_ZIP"
SQLITE=sqlite/.sqlite
if [ ! -f "$SQLITE/sqlite-amalgamation-3530400/sqlite3.c" ]; then
  mkdir -p "$SQLITE"
  echo "fetching $SQLITE_URL"
  curl -fsSL -o "$SQLITE/$SQLITE_ZIP" "$SQLITE_URL"
  (cd "$SQLITE" && unzip -q -o "$SQLITE_ZIP")
fi
if [ ! -f "$SQLITE/sqlite3-patched.c" ] || [ sqlite/csrc/patch-sqlite.py -nt "$SQLITE/sqlite3-patched.c" ]; then
  python3 sqlite/csrc/patch-sqlite.py "$SQLITE/sqlite-amalgamation-3530400/sqlite3.c" "$SQLITE/sqlite3-patched.c"
fi
. postgres/vendor/pg_query-source.sh
PG_QUERY_PATCH="postgres/vendor/pg_query-18.0.0.patch"
PATCHED=postgres/.patched
sha256() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi
}
# What the copy was made from: a change of either commit or of the patch makes it again.
PG_QUERY_SOURCE="$PG_QUERY_COMMIT $LIBPG_QUERY_COMMIT $(sha256 "$PG_QUERY_PATCH")"
# tokio-postgres reads wasm32 as a browser, which has no sockets, and resolves host names on a
# thread, which wasip2 has not: the component builds it with the patch beside it.
TP=wasip2/.patched/tokio-postgres
TP_SHA256=a528f7d280f6d5b9cd149635c8705b0dd049754bc67d81d31fa25169a93809d3
TP_SOURCE="$TP_SHA256 $(sha256 wasip2/tokio-postgres-0.7.18.patch)"
if [ "$(cat $TP/.source 2>/dev/null || true)" != "$TP_SOURCE" ]; then
  rm -rf "$TP" && mkdir -p wasip2/.patched
  curl -fsSL -A reylo-build -o wasip2/.patched/tokio-postgres.crate https://static.crates.io/crates/tokio-postgres/tokio-postgres-0.7.18.crate
  if [ "$(sha256 wasip2/.patched/tokio-postgres.crate)" != "$TP_SHA256" ]; then
    echo "tokio-postgres 0.7.18: the crate's SHA-256 is $(sha256 wasip2/.patched/tokio-postgres.crate), expected $TP_SHA256" >&2
    exit 1
  fi
  tar -xzf wasip2/.patched/tokio-postgres.crate -C wasip2/.patched && mv wasip2/.patched/tokio-postgres-0.7.18 "$TP" && rm wasip2/.patched/tokio-postgres.crate
  (cd "$TP" && patch -s -p1 < ../../tokio-postgres-0.7.18.patch)
  echo "$TP_SOURCE" > "$TP/.source"
fi
if [ "$(cat $PATCHED/pg_query/.source 2>/dev/null || true)" != "$PG_QUERY_SOURCE" ]; then
  # Straight from GitHub, not through `cargo fetch`: the workspace's `[patch]` points at the
  # copy this makes, so cargo resolves nothing until it exists. A commit's archive holds no
  # submodule, so libpg_query is fetched beside it, at the commit the crate's tree names.
  rm -rf "$PATCHED" && mkdir -p $PATCHED/fetch
  fetch() {
    echo "fetching $1 at $2"
    curl -fsSL -o "$PATCHED/fetch/$1.tar.gz" "https://codeload.github.com/pganalyze/$1/tar.gz/$2"
    if [ "$(sha256 "$PATCHED/fetch/$1.tar.gz")" != "$3" ]; then
      echo "$1 at $2: the archive's SHA-256 is $(sha256 "$PATCHED/fetch/$1.tar.gz"), expected $3" >&2
      exit 1
    fi
    tar -xzf "$PATCHED/fetch/$1.tar.gz" -C $PATCHED/fetch
  }
  fetch pg_query.rs "$PG_QUERY_COMMIT" "$PG_QUERY_SHA256"
  fetch libpg_query "$LIBPG_QUERY_COMMIT" "$LIBPG_QUERY_SHA256"
  mv "$PATCHED/fetch/pg_query.rs-$PG_QUERY_COMMIT" $PATCHED/pg_query
  rmdir $PATCHED/pg_query/libpg_query
  mv "$PATCHED/fetch/libpg_query-$LIBPG_QUERY_COMMIT" $PATCHED/pg_query/libpg_query
  # What the crate's `build.rs` never reads: libpg_query's tests and tools, and the C++ protobuf
  # bindings, 30 MB of the 36.
  (cd $PATCHED/pg_query/libpg_query && rm -rf test tmp benchmark examples srcdata scripts patches protobuf/pg_query.pb.cc protobuf/pg_query.pb.h)
  rm -rf "$PATCHED/fetch"
  (cd $PATCHED/pg_query && patch -p1 -N < "../../../$PG_QUERY_PATCH")
  echo "$PG_QUERY_SOURCE" > $PATCHED/pg_query/.source
fi
if [ "${1:-}" = "--prepare" ]; then
  exit 0
fi

if [ $# -gt 0 ]; then WANTED=("$@"); else WANTED=("${DATABASES[@]}"); fi
for db in "${WANTED[@]}"; do
  case " ${DATABASES[*]} " in *" $db "*) ;; *) echo "no engine for $db: build.sh builds ${DATABASES[*]}" >&2; exit 2 ;; esac
done

if [ -z "${WASI_SDK_PATH:-}" ]; then
  WASI_SDK_PATH=$(ls -d .wasi-sdk/wasi-sdk-* 2>/dev/null | head -1 || true)
fi
if [ -z "$WASI_SDK_PATH" ] || [ ! -x "$WASI_SDK_PATH/bin/clang" ]; then
  echo "wasi-sdk not found: set WASI_SDK_PATH or unpack wasi-sdk 34 under engine/.wasi-sdk/" >&2
  exit 1
fi
WASI_SDK_PATH=$(cd "$WASI_SDK_PATH" && pwd)
SYSROOT="$WASI_SDK_PATH/share/wasi-sysroot"
STUBS="$(pwd)/postgres/vendor/stubs"
JCO="$(cd .. && pwd)/node_modules/@bytecodealliance/jco/dist/jco.js"
[ -f "$JCO" ] || { echo "jco not found: nub install at the repository's root" >&2; exit 1; }

# The components are published, so they name no path on the machine that built them: the checkout,
# the Cargo registry and wasi-sdk are written as /reylo, /cargo and /wasi-sdk wherever the Rust
# panic locations and the C sources' __FILE__ would record them.
REPO_ROOT=$(cd .. && pwd)
CARGO_HOME_DIR=$(cd "${CARGO_HOME:-$HOME/.cargo}" 2>/dev/null && pwd || echo "$HOME/.cargo")
PREFIX_MAPS="-ffile-prefix-map=$REPO_ROOT=/reylo -ffile-prefix-map=$CARGO_HOME_DIR=/cargo -ffile-prefix-map=$WASI_SDK_PATH=/wasi-sdk"
export CC_wasm32_wasip2="$WASI_SDK_PATH/bin/clang" AR_wasm32_wasip2="$WASI_SDK_PATH/bin/llvm-ar"
# The C flags for every C crate, ring's among them, which compiles with -nostdlibinc and so cannot
# take the stubs as an `-include`: the stubs come last on the include path, and what the include
# defined is defined on the command line. Exceptions are exnref, the exception-handling proposal as
# it was standardized, since jco and wasmtime refuse the legacy `try`.
export CFLAGS_wasm32_wasip2="--sysroot=$SYSROOT -idirafter $STUBS -DEHOSTDOWN=9001 -Dsigjmp_buf=jmp_buf -Dsigsetjmp(b,m)=setjmp(b) -Dsiglongjmp(b,v)=longjmp(b,v) -mllvm -wasm-enable-sjlj -mllvm -wasm-use-legacy-eh=false -D_WASI_EMULATED_SIGNAL -D_WASI_EMULATED_PROCESS_CLOCKS -D_WASI_EMULATED_MMAN -D_WASI_EMULATED_GETPID $PREFIX_MAPS"
export BINDGEN_EXTRA_CLANG_ARGS_wasm32_wasip2="--sysroot=$SYSROOT --target=wasm32-wasip2 -I $STUBS"
# tokio's wasip2 sockets are behind `tokio_unstable` (tokio 1.51). The shadow stack, where C and the
# Rust frames that spill keep their locals, is 8 MiB: the analysis of a statement as deep as
# libpg_query hands over (core/src/stack.rs) needs about 0.5 MiB of it.
export CARGO_TARGET_WASM32_WASIP2_RUSTFLAGS="--cfg tokio_unstable -C link-arg=-L$SYSROOT/lib/wasm32-wasip2 -C link-arg=-lwasi-emulated-signal -C link-arg=-lwasi-emulated-process-clocks -C link-arg=-lwasi-emulated-mman -C link-arg=-lwasi-emulated-getpid -C link-arg=-lsetjmp -C link-arg=-zstack-size=8388608 --remap-path-prefix=$REPO_ROOT=/reylo --remap-path-prefix=$CARGO_HOME_DIR=/cargo"
# SQLite's SQL functions that Node's own SQLite has and rusqlite's bundled build leaves out, so that
# the SQLite engine's function list, which it refuses a statement by, has them too.
export LIBSQLITE3_FLAGS="${LIBSQLITE3_FLAGS:-} -DSQLITE_ENABLE_MATH_FUNCTIONS -DSQLITE_ENABLE_PERCENTILE -DSQLITE_ENABLE_GEOPOLY"

# One build per database: each is the same crate with another database's program behind its
# exports, and wasip2/ is a workspace of its own, so that its tokio-postgres patch reaches no native
# build. The components are built without LTO: the depth a statement may nest was measured on that
# code, and LTO's larger frames spend more of the JavaScript engine's stack at the bound.
TARGET_DIR="${CARGO_TARGET_DIR:-$REPO_ROOT/target/wasip2}"
for db in "${WANTED[@]}"; do
  CARGO_TARGET_DIR="$TARGET_DIR/$db" cargo build --manifest-path wasip2/Cargo.toml --release --target wasm32-wasip2 --features "$db"
  OUT="../packages/$db/engine"
  rm -rf "$OUT" && mkdir -p "$OUT"
  node "$JCO" transpile "$TARGET_DIR/$db/wasm32-wasip2/release/reylo_engine_wasip2.wasm" -o "$OUT" --name engine --instantiation sync --bindgen-enable-wasm-exnref --no-typescript --quiet
  # The loader types the component itself (src/component.ts); jco writes declarations with sync instantiation all the same.
  rm -rf "$OUT/interfaces" "$OUT"/*.d.ts
  ls -l "$OUT"/*.wasm "$OUT"/engine.js
done
