#!/usr/bin/env bash
# Builds the `reylo` binary for one platform of npm/platforms.json on the runner binaries.yml gives
# it, natively: each platform has a runner of its own architecture, and the two that are not native
# are made with the runner's own toolchain (macOS on Intel, from Apple silicon) or with musl's
# (`musl-gcc` on Ubuntu, for Linux on musl). Linux on glibc is built in the platform's container,
# manylinux_2_28 (AlmaLinux 8), so that the binary starts on glibc 2.28, as Node 24 itself does. Writes `<out>/engine-<platform>/reylo` (`reylo.exe` on
# Windows), which scripts/build.ts packages with `--binaries <out>`.
#
#   .github/scripts/build-binary.sh <platform> <out>
#
# The build is the one scripts/build.ts makes on a developer's machine: the release profile,
# stripped of its symbol table (RELEASE_ENV there), with the SQLite flags of .cargo/config.toml.
set -euo pipefail

platform="${1:?usage: build-binary.sh <platform> <out>}"
out="${2:?usage: build-binary.sh <platform> <out>}"
target="$(node -p "(require('./npm/platforms.json').find((p) => p.platform === process.argv[1]) ?? {}).target ?? ''" "$platform")"
[ -n "$target" ] || { echo "::error::npm/platforms.json names no platform $platform"; exit 1; }

export CARGO_PROFILE_RELEASE_STRIP=symbols
case "$target" in
  *-apple-darwin)
    # The oldest macOS the binary starts on, for the C the crates compile as for rustc's own code.
    export MACOSX_DEPLOYMENT_TARGET=11.0
    ;;
  *-linux-gnu)
    # The container's toolchain, and what the build reads beside it: libclang for bindgen, and
    # python3, unzip and patch for the prepare step.
    if command -v dnf > /dev/null; then
      dnf install -y -q clang-devel python3 unzip patch > /dev/null
    fi
    ;;
  *-linux-musl)
    # musl's compiler wrapper for the C (libpg_query, SQLite, ring) and the linker, and a static
    # binary, as Rust's musl targets link by default: it runs on any Linux of its CPU.
    sudo apt-get update -qq && sudo apt-get install -y -qq --no-install-recommends musl-tools > /dev/null
    triple="${target//-/_}"
    export "CC_$triple=musl-gcc" "AR_$triple=ar"
    ;;
  *-pc-windows-msvc)
    # The C runtime linked in, so that the binary starts on a Windows without the Visual C++
    # redistributable, whose absence stops a program before it can say why (STATUS_DLL_NOT_FOUND).
    export RUSTFLAGS="-C target-feature=+crt-static"
    # bindgen reads libpg_query's header with the runner's LLVM.
    export LIBCLANG_PATH="${LIBCLANG_PATH:-C:\\Program Files\\LLVM\\bin}"
    ;;
esac

# rustup, where the runner's image has none (the arm64 images are younger than the others).
if ! command -v rustup > /dev/null; then
  case "$target" in
    *-windows-*)
      host=x86_64-pc-windows-msvc
      [ "$(uname -m)" = aarch64 ] && host=aarch64-pc-windows-msvc
      curl -fsSL -o "$RUNNER_TEMP/rustup-init.exe" "https://static.rust-lang.org/rustup/dist/$host/rustup-init.exe"
      "$RUNNER_TEMP/rustup-init.exe" -y --profile minimal --default-toolchain stable
      ;;
    *) curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable ;;
  esac
  export PATH="$HOME/.cargo/bin:$PATH"
fi
case "$target" in
  *-pc-windows-msvc)
    [ -f "$LIBCLANG_PATH/libclang.dll" ] || { echo "::error::no libclang.dll in $LIBCLANG_PATH, which bindgen reads libpg_query's header with: install LLVM, or name its bin directory in LIBCLANG_PATH"; exit 1; }
    # The prepare step patches SQLite with python3, which Windows names python.
    if ! command -v python3 > /dev/null; then
      mkdir -p "$RUNNER_TEMP/python3"
      printf '#!/bin/sh\nexec python "$@"\n' > "$RUNNER_TEMP/python3/python3"
      chmod +x "$RUNNER_TEMP/python3/python3"
      export PATH="$RUNNER_TEMP/python3:$PATH"
    fi
    ;;
esac

rustup target add "$target"
engine/build.sh --prepare
cargo build --release --locked -p reylo --target "$target"

exe=reylo
case "$target" in *-windows-*) exe=reylo.exe ;; esac
mkdir -p "$out/engine-$platform"
cp "${CARGO_TARGET_DIR:-target}/$target/release/$exe" "$out/engine-$platform/$exe"
# The runner runs what it built, macOS on Intel under Rosetta where the runner has it: the binary
# must start, print reylo's version, and answer a ping as each database's engine.
bin="$out/engine-$platform/$exe"
run() { if [ "$platform" = darwin-x64 ]; then arch -x86_64 "$bin" "$@"; else "$bin" "$@"; fi; }
if [ "$platform" = darwin-x64 ] && ! arch -x86_64 /usr/bin/true 2> /dev/null; then
  echo "::notice::$platform: this runner has no Rosetta, so the binary is not run here"
else
  printed="$(run --version)"
  version="$(node -p "require('./package.json').version")"
  [ "$printed" = "reylo $version" ] || { echo "::error::$platform: the binary prints \"$printed\", and reylo is $version"; exit 1; }
  for db in postgres mysql sqlite duckdb; do
    answer="$(echo '{"op":"ping"}' | run engine "$db" call)"
    case "$answer" in *'"status":"ok"'*"\"database\":\"$db\""*) ;; *) echo "::error::$platform: reylo engine $db answered a ping with $answer"; exit 1 ;; esac
  done
  echo "$platform: $printed, and each database's engine answers"
fi
# A glibc binary starts on the oldest glibc whose version names every symbol it takes.
case "$target" in
  *-linux-gnu)
    newest="$(objdump -T "$bin" | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -1)"
    case "$newest" in
      GLIBC_2.[0-9] | GLIBC_2.1[0-9] | GLIBC_2.1[0-9].* | GLIBC_2.2[0-8] | GLIBC_2.2[0-8].* | GLIBC_2.[0-9].*) echo "$platform: needs $newest at most" ;;
      *) echo "::error::$platform: the binary needs $newest, newer than glibc 2.28"; exit 1 ;;
    esac
    ;;
esac
ls -l "$bin"

# Touching this file runs binaries.yml on the pull request, for its macOS binary; dropped before review.
