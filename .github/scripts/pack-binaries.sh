#!/usr/bin/env bash
# Packs the binaries binaries.yml built as a GitHub release carries them: `reylo-<platform>.tar.gz`
# for each platform of npm/platforms.json, holding `reylo` (`reylo.exe` on Windows) with its
# execute bit, which an artifact does not keep, and SHA256SUMS over every `reylo-*.tar.gz` in <out>,
# the playground's engine included when it is there. release.yml attaches what this writes, and
# channels.yml installs from it.
#
#   .github/scripts/pack-binaries.sh <binaries> <out>
#
# <binaries> holds `engine-<platform>/` for each platform, as the build's artifacts download.
set -euo pipefail

binaries="${1:?usage: pack-binaries.sh <binaries> <out>}"
out="${2:?usage: pack-binaries.sh <binaries> <out>}"
mkdir -p "$out"
for platform in $(jq -r '.[].platform' npm/platforms.json); do
  dir="$binaries/engine-$platform"
  exe=reylo
  case "$platform" in win32-*) exe=reylo.exe ;; esac
  [ -f "$dir/$exe" ] || { echo "::error::the build left no $exe for $platform"; exit 1; }
  chmod 755 "$dir/$exe"
  tar -czf "$out/reylo-$platform.tar.gz" -C "$dir" "$exe"
done
(cd "$out" && sha256sum reylo-*.tar.gz > SHA256SUMS && cat SHA256SUMS)
