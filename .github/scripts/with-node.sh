#!/bin/sh
# Runs a command with Node 24 on PATH inside a Linux image that may ship none (AlmaLinux 8, Amazon
# Linux 2023), as binaries.yml's install check runs in each: where the image has no `node`, Node's
# own build for its CPU, fetched from nodejs.org at the version below and checked against the
# release's SHA256SUMS.
#
#   .github/scripts/with-node.sh <command> [args…]
set -eu

if ! command -v node > /dev/null; then
  version=24.22.2
  case "$(uname -m)" in
    x86_64) arch=x64 ;;
    aarch64) arch=arm64 ;;
    *) echo "with-node.sh: no Node build for $(uname -m)" >&2; exit 1 ;;
  esac
  if ! command -v tar > /dev/null || ! command -v gzip > /dev/null; then
    dnf install -y -q tar gzip > /dev/null
  fi
  file="node-v$version-linux-$arch.tar.gz"
  dir="$(mktemp -d)"
  curl -fsSL -o "$dir/$file" "https://nodejs.org/dist/v$version/$file"
  curl -fsSL -o "$dir/SHASUMS256.txt" "https://nodejs.org/dist/v$version/SHASUMS256.txt"
  (cd "$dir" && grep " $file\$" SHASUMS256.txt | sha256sum -c -)
  tar -xzf "$dir/$file" -C /opt
  PATH="/opt/node-v$version-linux-$arch/bin:$PATH"
  export PATH
fi
exec "$@"
