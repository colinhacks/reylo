#!/bin/sh
# Installs the reylo command on macOS or Linux, from the binaries a GitHub release of reylosql/reylo
# carries, each checked against the release's SHA256SUMS before it is installed.
#
#   curl -fsSL https://reylo.dev/install.sh | sh
#
# REYLO_VERSION      The version to install, as 0.8.0; the latest release by default.
# REYLO_INSTALL_DIR  Where the binary goes; ~/.local/bin by default.
# REYLO_DOWNLOAD_URL A directory holding a release's assets, in place of the release's own.
#
# On Windows, install.ps1 beside this file does the same.
set -eu

# The whole script is one function, called on its last line, so that a download cut short runs nothing.
main() {
  repo=reylosql/reylo
  dir="${REYLO_INSTALL_DIR:-$HOME/.local/bin}"
  version="${REYLO_VERSION:-}"
  version="${version#v}"

  fail() {
    echo "reylo install: $*" >&2
    exit 1
  }

  # The machine, as the release names its assets: <os>-<cpu>, and on Linux -musl where glibc is missing
  # or older than 2.28, the oldest the glibc build starts on. The musl build is static, and runs on any
  # Linux of its CPU.
  case "$(uname -s)" in
    Darwin) os=darwin ;;
    Linux) os=linux ;;
    MINGW* | MSYS* | CYGWIN*) fail "on Windows, run in PowerShell: irm https://reylo.dev/install.ps1 | iex" ;;
    *) fail "no build for $(uname -s); reylo is built for macOS, Linux and Windows" ;;
  esac
  case "$(uname -m)" in
    x86_64 | amd64) cpu=x64 ;;
    arm64 | aarch64) cpu=arm64 ;;
    *) fail "no build for $(uname -m); reylo is built for x64 and arm64" ;;
  esac
  # A shell under Rosetta reports x86_64 on Apple silicon, whose own build is the one to install.
  if [ "$os" = darwin ] && [ "$cpu" = x64 ] && [ "$(sysctl -n sysctl.proc_translated 2> /dev/null || echo 0)" = 1 ]; then
    cpu=arm64
  fi
  platform="$os-$cpu"
  if [ "$os" = linux ]; then
    glibc="$(getconf GNU_LIBC_VERSION 2> /dev/null | sed -n 's/^glibc \([0-9]*\)\.\([0-9]*\).*/\1 \2/p')"
    if [ -z "$glibc" ] || [ "${glibc% *}" -lt 2 ] || { [ "${glibc% *}" -eq 2 ] && [ "${glibc#* }" -lt 28 ]; }; then
      platform="$platform-musl"
    fi
  fi

  # Every download goes through one function, with curl or else wget (which is all Alpine has).
  if command -v curl > /dev/null 2>&1; then
    fetch() { curl -fsSL --retry 3 -o "$2" "$1"; }
  elif command -v wget > /dev/null 2>&1; then
    fetch() { wget -q -O "$2" "$1"; }
  else
    fail "needs curl or wget to download the release"
  fi
  # Some minimal images, Amazon Linux 2023's among them, carry no tar.
  for tool in tar gzip; do
    command -v "$tool" > /dev/null 2>&1 || fail "needs $tool to unpack the release; install it first (dnf install $tool, apt-get install $tool)"
  done
  if command -v sha256sum > /dev/null 2>&1; then
    sha256() { sha256sum "$1" | cut -d' ' -f1; }
  elif command -v shasum > /dev/null 2>&1; then
    sha256() { shasum -a 256 "$1" | cut -d' ' -f1; }
  else
    fail "needs sha256sum or shasum to check the download"
  fi

  # The latest release's assets come from GitHub's latest/download, SHA256SUMS first. Should a release
  # land between the two downloads, the archive no longer matches the sums and nothing is installed.
  if [ -n "${REYLO_DOWNLOAD_URL:-}" ]; then
    base="${REYLO_DOWNLOAD_URL%/}"
  elif [ -n "$version" ]; then
    base="https://github.com/$repo/releases/download/v$version"
  else
    base="https://github.com/$repo/releases/latest/download"
  fi
  asset="reylo-$platform.tar.gz"

  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT INT TERM
  fetch "$base/SHA256SUMS" "$tmp/SHA256SUMS" || fail "cannot download $base/SHA256SUMS${version:+; is $version a released version?}"
  expected="$(awk -v f="$asset" '$2 == f || $2 == "*" f { print $1 }' "$tmp/SHA256SUMS")"
  [ -n "$expected" ] || fail "$base/SHA256SUMS lists no $asset"
  fetch "$base/$asset" "$tmp/$asset" || fail "cannot download $base/$asset"
  actual="$(sha256 "$tmp/$asset")"
  [ "$actual" = "$expected" ] || fail "$asset does not match SHA256SUMS (expected $expected, got $actual); nothing was installed"

  tar -xzf "$tmp/$asset" -C "$tmp"
  mkdir -p "$dir"
  # Into place with a rename, so a reylo that is running keeps its file and a failed copy leaves no half.
  cp "$tmp/reylo" "$dir/.reylo.$$"
  chmod 755 "$dir/.reylo.$$"
  mv -f "$dir/.reylo.$$" "$dir/reylo"

  echo "Installed $("$dir/reylo" --version) ($platform) at $dir/reylo"
  case ":$PATH:" in
    *":$dir:"*) ;;
    *) echo "$dir is not on your PATH: add it in your shell's profile, as with export PATH=\"$dir:\$PATH\"" ;;
  esac
}

main "$@"
