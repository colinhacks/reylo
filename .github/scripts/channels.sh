#!/usr/bin/env bash
# Writes what the package managers install a release from, out of the release's SHA256SUMS: the
# Homebrew formula, <out>/Formula/reylo.rb, and the Scoop manifest, <out>/bucket/reylo.json, each
# naming the release's `reylo-<platform>.tar.gz` for its platforms with the checksum SHA256SUMS
# gives it. release.yml writes them into the tap and the bucket once a release is out, and
# channels.yml installs from them first.
#
#   .github/scripts/channels.sh <version> <SHA256SUMS> <out> [<base url>]
#
# <base url> holds the assets, the release's own download URL by default; channels.yml names a
# directory it serves on the machine, to install a run's binaries before any release has them.
set -euo pipefail

version="${1:?usage: channels.sh <version> <SHA256SUMS> <out> [<base url>]}"
sums="${2:?usage: channels.sh <version> <SHA256SUMS> <out> [<base url>]}"
out="${3:?usage: channels.sh <version> <SHA256SUMS> <out> [<base url>]}"
version="${version#v}"
base="${4:-https://github.com/colinhacks/reylo/releases/download/v$version}"
base="${base%/}"

desc="Infer types for SQL queries from the database itself"

sha() {
  local hash
  hash="$(awk -v f="reylo-$1.tar.gz" '$2 == f || $2 == "*" f { print $1 }' "$sums")"
  [[ "$hash" =~ ^[0-9a-f]{64}$ ]] || { echo "::error::$sums has no checksum for reylo-$1.tar.gz" >&2; exit 1; }
  printf '%s' "$hash"
}

mkdir -p "$out/Formula" "$out/bucket"

# Homebrew: macOS and Linux, each on Apple silicon or arm64 and on Intel. Linux takes the glibc
# build, which starts on glibc 2.28, as Homebrew's own glibc and every distribution it supports have.
cat > "$out/Formula/reylo.rb" << EOF
# Written by colinhacks/reylo's release workflow (.github/scripts/channels.sh) for each release.
class Reylo < Formula
  desc "$desc"
  homepage "https://reylo.dev"
  version "$version"
  license "MIT"

  on_macos do
    on_arm do
      url "$base/reylo-darwin-arm64.tar.gz"
      sha256 "$(sha darwin-arm64)"
    end
    on_intel do
      url "$base/reylo-darwin-x64.tar.gz"
      sha256 "$(sha darwin-x64)"
    end
  end

  on_linux do
    on_arm do
      url "$base/reylo-linux-arm64.tar.gz"
      sha256 "$(sha linux-arm64)"
    end
    on_intel do
      url "$base/reylo-linux-x64.tar.gz"
      sha256 "$(sha linux-x64)"
    end
  end

  def install
    bin.install "reylo"
  end

  test do
    assert_match "reylo #{version}", shell_output("#{bin}/reylo --version")
    (testpath/"app.db").write ""
    system bin/"reylo", "run", "create table users (id integer primary key, email text not null) strict",
           "--url", "sqlite:#{testpath}/app.db"
    assert_match '"cardinality":"many"',
                 shell_output("#{bin}/reylo analyze --sql 'select email from users' " \\
                              "--url sqlite:#{testpath}/app.db --format json")
  end
end
EOF

# Scoop: Windows on x64 and arm64. checkver and autoupdate let Scoop's own tooling find a newer
# release too, though the release workflow writes each one.
jq -n \
  --arg version "$version" \
  --arg desc "$desc" \
  --arg x64 "$base/reylo-win32-x64.tar.gz" --arg x64sha "$(sha win32-x64)" \
  --arg arm64 "$base/reylo-win32-arm64.tar.gz" --arg arm64sha "$(sha win32-arm64)" \
  '{
    version: $version,
    description: $desc,
    homepage: "https://reylo.dev",
    license: "MIT",
    architecture: {
      "64bit": { url: $x64, hash: $x64sha },
      arm64: { url: $arm64, hash: $arm64sha }
    },
    bin: "reylo.exe",
    checkver: { github: "https://github.com/colinhacks/reylo" },
    autoupdate: {
      architecture: {
        "64bit": { url: "https://github.com/colinhacks/reylo/releases/download/v$version/reylo-win32-x64.tar.gz" },
        arm64: { url: "https://github.com/colinhacks/reylo/releases/download/v$version/reylo-win32-arm64.tar.gz" }
      },
      hash: { url: "$baseurl/SHA256SUMS" }
    }
  }' > "$out/bucket/reylo.json"

echo "wrote $out/Formula/reylo.rb and $out/bucket/reylo.json for $version from $base"
