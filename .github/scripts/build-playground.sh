#!/usr/bin/env bash
# Builds the docs site's playground engine on an x86_64 Linux runner, as engine/build.sh builds it
# (Postgres's engine as one WebAssembly component, which jco transpiles), and packs it into
# <out>/reylo-playground-postgres.tar.gz: `engine.js` and its `engine.core*.wasm` modules, nothing
# else, which the site's build fetches from the release (site/scripts/playground-engine.ts).
# It needs rustup and the root's install (jco), and fetches wasi-sdk 34 by its SHA-256.
#
#   .github/scripts/build-playground.sh <out>
set -euo pipefail

[ $# -eq 1 ] || { echo "usage: build-playground.sh <out>" >&2; exit 2; }
mkdir -p "$1"
out=$(cd "$1" && pwd)

WASI_SDK=wasi-sdk-34.0-x86_64-linux
WASI_SDK_SHA256=b761e3a0721dbae9c09a0059e5fdb2bf917d1b4a8a7b430fb3b5aafb0984b2c4
tmp=${RUNNER_TEMP:-$(mktemp -d)}

rustup target add wasm32-wasip2
curl -fsSL -o "$tmp/$WASI_SDK.tar.gz" "https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-34/$WASI_SDK.tar.gz"
echo "$WASI_SDK_SHA256  $tmp/$WASI_SDK.tar.gz" | sha256sum -c -
mkdir -p engine/.wasi-sdk && tar -xzf "$tmp/$WASI_SDK.tar.gz" -C engine/.wasi-sdk
export WASI_SDK_PATH="$PWD/engine/.wasi-sdk/$WASI_SDK"

engine/build.sh
(cd target/playground/postgres && tar -czf "$out/reylo-playground-postgres.tar.gz" engine.js engine.core*.wasm)
tar -tzvf "$out/reylo-playground-postgres.tar.gz"
