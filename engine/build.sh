#!/usr/bin/env bash
# Builds each database's engine to a wasm32-wasip1 artifact of its own, the `engine.wasm` of its
# npm package: `packages/postgres/engine.wasm`, `packages/sqlite/engine.wasm` and
# `packages/mysql/engine.wasm`, from the entry crates in `wasm/`.
#
#   engine/build.sh --prepare            the inputs below, and nothing else
#   engine/build.sh                      every database's artifact, in one cargo invocation
#   engine/build.sh postgres [sqlite …]  the artifacts named
#
# Inputs: a Rust toolchain with the `wasm32-wasip1` target, and wasi-sdk 34 (set WASI_SDK_PATH,
# or unpack it under `.wasi-sdk/`). The `pg_query` crate is the Postgres 18 parser: upstream's
# `pg-18` branch (pganalyze/pg_query.rs, pull request 79, "Release 18.0.0", not on crates.io yet)
# at the commit `postgres/vendor/pg_query-source.sh` pins, with the libpg_query commit its submodule names (tag 18.1.0, the
# grammar of Postgres 18.6), each fetched from GitHub by commit and checked against a SHA-256, and
# the patch from `postgres/vendor/` applied into `postgres/.patched/pg_query`. SQLite is the 3.53.4
# amalgamation fetched into `sqlite/.sqlite/` with the hooks of `sqlite/csrc/patch-sqlite.py`
# applied; this script prepares both. `./build.sh --prepare` stops after that, which is what a
# native `cargo test` needs, and what any cargo command in the workspace needs first (the workspace
# `[patch]` points at the copy).
set -euo pipefail
cd "$(dirname "$0")"
DATABASES=(postgres sqlite mysql)

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


export CC_wasm32_wasip1="$WASI_SDK_PATH/bin/clang"
export AR_wasm32_wasip1="$WASI_SDK_PATH/bin/llvm-ar"
# The artifacts are published, so they name no path on the machine that built them: the
# checkout, the Cargo registry and wasi-sdk are written as /reylo, /cargo and /wasi-sdk wherever
# the Rust panic locations and the C sources' __FILE__ would record them. A build from any checkout
# then gives the same bytes, as long as it builds every artifact in one run: `build.sh sqlite`
# alone gives other bytes, since cargo unifies the features of the crates the three share.
REPO_ROOT=$(cd .. && pwd)
CARGO_HOME_DIR=$(cd "${CARGO_HOME:-$HOME/.cargo}" 2>/dev/null && pwd || echo "$HOME/.cargo")
PREFIX_MAPS="-ffile-prefix-map=$REPO_ROOT=/reylo -ffile-prefix-map=$CARGO_HOME_DIR=/cargo -ffile-prefix-map=$WASI_SDK_PATH=/wasi-sdk"
export CFLAGS_wasm32_wasip1="--sysroot=$SYSROOT -I $STUBS -include $STUBS/wasi_extra.h -mllvm -wasm-enable-sjlj -D_WASI_EMULATED_SIGNAL -D_WASI_EMULATED_PROCESS_CLOCKS -D_WASI_EMULATED_MMAN -D_WASI_EMULATED_GETPID $PREFIX_MAPS"
export BINDGEN_EXTRA_CLANG_ARGS_wasm32_wasip1="--sysroot=$SYSROOT --target=wasm32-wasip1 -I $STUBS"
# The shadow stack, where C and the Rust frames that spill keep their locals, is 8 MiB rather than
# the 1 MiB rustc links by default: the analysis of a statement as deep as libpg_query hands over
# (src/stack.rs) needs about 0.5 MiB of it. libpg_query cannot find the bounds of a WebAssembly
# stack and keeps its default budget of 100 kB of it, which stops a statement nested about twice
# that deep while the JavaScript engine's own stack, which every wasm call also spends, has room.
# `--compress-relocations` writes each index and address the linker fills in at its own length
# rather than padded to five bytes, which needs the artifact stripped (`CARGO_PROFILE_RELEASE_STRIP`
# below); together they leave out a quarter of each artifact and change no instruction.
export CARGO_TARGET_WASM32_WASIP1_RUSTFLAGS="-C link-arg=-L$SYSROOT/lib/wasm32-wasip1 -C link-arg=-lwasi-emulated-signal -C link-arg=-lwasi-emulated-process-clocks -C link-arg=-lwasi-emulated-mman -C link-arg=-lwasi-emulated-getpid -C link-arg=-lsetjmp -C link-arg=-zstack-size=8388608 -C link-arg=--compress-relocations --remap-path-prefix=$REPO_ROOT=/reylo --remap-path-prefix=$CARGO_HOME_DIR=/cargo"

# The artifacts asked for, every database's by default.
if [ $# -gt 0 ]; then WANTED=("$@"); else WANTED=("${DATABASES[@]}"); fi
PACKAGES=()
for db in "${WANTED[@]}"; do
  case " ${DATABASES[*]} " in
    *" $db "*) PACKAGES+=(-p "reylo-engine-$db-wasm") ;;
    *) echo "no engine for $db: build.sh builds ${DATABASES[*]}" >&2; exit 2 ;;
  esac
done

# The entry crates are members of the workspace in `..`, so the artifacts land in the workspace's
# `target/`. The workspace's release profile is `opt-level = 3` for Reylo Framework's server; the
# wasm the loader ships is built for size. It is built without LTO, as the one artifact of a single
# `["cdylib", "rlib"]` crate always was (cargo runs no LTO for a crate that is also an rlib): LTO
# inlines across crates into larger frames, every wasm frame is a frame on the JavaScript engine's
# own stack, and the depth a statement may nest (`core/src/stack.rs`) and the loader's depth tests
# were measured on code built this way; with LTO a MySQL statement at the bound overflows Node's
# stack. One invocation for every artifact, so the crates they share are built once. The artifacts
# are stripped of the name section, the name of every function, which only a stack trace reads: a
# trap reports `wasm-function[N]` instead, which a build of the same commit without the strip names.
export CARGO_PROFILE_RELEASE_OPT_LEVEL=s CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_STRIP=symbols
TARGET_DIR="${CARGO_TARGET_DIR:-$(cd .. && pwd)/target}"
cargo build --release --target wasm32-wasip1 "${PACKAGES[@]}"
for db in "${WANTED[@]}"; do
  mkdir -p "../packages/$db"
  cp "$TARGET_DIR/wasm32-wasip1/release/reylo_engine_${db}_wasm.wasm" "../packages/$db/engine.wasm"
  ls -l "../packages/$db/engine.wasm"
done
