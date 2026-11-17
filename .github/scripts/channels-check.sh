#!/usr/bin/env bash
# Installs reylo through every channel a platform has, as a user would, and runs what each installed
# (.github/scripts/channels-smoke.sh): on macOS, install.sh and the Homebrew formula; on Linux,
# install.sh in each of the platform's images (npm/platforms.json), the Homebrew formula on glibc and
# the container image on musl; on Windows, install.ps1 under PowerShell 7 and Windows PowerShell 5.1,
# and the Scoop manifest; and mise on each runner, on a published release. channels.yml runs it on
# each platform's own runner.
#
#   .github/scripts/channels-check.sh <platform> <assets> <version> [<images>]
#
# <assets> holds a release's `reylo-<platform>.tar.gz` and SHA256SUMS, with the formula and the
# manifest channels.sh wrote from them, and <images> is the platform's `images` from
# npm/platforms.json, as JSON. With REYLO_DOWNLOAD_URL set, every installer reads the assets from it,
# a server of the run's own; without it, from the GitHub release of <version>.
set -euo pipefail

platform="${1:?usage: channels-check.sh <platform> <assets> <version> [<images>]}"
assets="$(cd "${2:?usage: channels-check.sh <platform> <assets> <version> [<images>]}" && pwd)"
version="${3:?usage: channels-check.sh <platform> <assets> <version> [<images>]}"
images="${4:-null}"
smoke="$PWD/.github/scripts/channels-smoke.sh"
work="$(mktemp -d)"
[ -n "${REYLO_DOWNLOAD_URL:-}" ] || export REYLO_VERSION="$version"

# A download URL on 127.0.0.1 is <assets>, served here for as long as the check runs.
if [[ "${REYLO_DOWNLOAD_URL:-}" =~ ^http://127\.0\.0\.1:([0-9]+)/?$ ]]; then
  python=python3
  command -v python3 > /dev/null || python=python
  "$python" -m http.server "${BASH_REMATCH[1]}" --bind 127.0.0.1 --directory "$assets" > "$work/server.log" 2>&1 &
  server=$!
  trap 'kill "$server" 2> /dev/null || true' EXIT
  up=
  for _ in $(seq 30); do
    if curl -sf -o /dev/null "$REYLO_DOWNLOAD_URL/SHA256SUMS"; then up=1; break; fi
    sleep 1
  done
  [ -n "$up" ] || { cat "$work/server.log"; echo "::error::the assets' server did not start"; exit 1; }
fi

step() { echo "::group::$*"; }
done_() { echo "::endgroup::"; }

# An installer's output, shown, which must say it installed this platform's build, so a glibc check
# that fell back to musl fails.
installed() {
  cat "$1"
  grep -qF "Installed reylo $version ($platform)" "$1" || { echo "::error::the installer did not install reylo $version for $platform"; exit 1; }
}

homebrew() {
  step "Homebrew"
  if ! command -v brew > /dev/null; then
    if [ -x /home/linuxbrew/.linuxbrew/bin/brew ]; then
      eval "$(/home/linuxbrew/.linuxbrew/bin/brew shellenv)"
    else
      NONINTERACTIVE=1 bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
      eval "$(/home/linuxbrew/.linuxbrew/bin/brew shellenv)"
    fi
  fi
  export HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_INSTALL_CLEANUP=1 HOMEBREW_NO_ENV_HINTS=1 HOMEBREW_NO_ANALYTICS=1
  brew tap-new --no-git reylocheck/test > /dev/null
  cp "$assets/Formula/reylo.rb" "$(brew --repository)/Library/Taps/reylocheck/homebrew-test/Formula/reylo.rb"
  brew install reylocheck/test/reylo
  brew test reylocheck/test/reylo
  # The audit reads the formula's URLs, so it runs on a release's own.
  if [ -z "${REYLO_DOWNLOAD_URL:-}" ]; then
    brew trust reylocheck/test 2> /dev/null || true
    brew audit --strict --online reylocheck/test/reylo
  fi
  sh "$smoke" "$(brew --prefix)/bin/reylo" "$version"
  done_
}

# mise, from the release's own assets on GitHub through its github: backend, pinned in a project's
# mise.toml as a Rust project would pin it. Only on a published release, which mise reads from GitHub.
mise_check() {
  [ -z "${REYLO_DOWNLOAD_URL:-}" ] || return 0
  step "mise"
  if ! command -v mise > /dev/null; then
    case "$platform" in
      win32-*) "$HOME/scoop/shims/scoop.cmd" install mise ;;
      *) curl -fsSL https://mise.run | MISE_INSTALL_PATH="$work/bin/mise" sh ;;
    esac
  fi
  local mise
  mise="$(command -v mise || echo "$work/bin/mise")"
  [ -x "$mise" ] || mise="$HOME/scoop/shims/mise.exe"
  mkdir -p "$work/mise-project"
  printf '[tools]\n"github:reylosql/reylo" = "%s"\n' "$version" > "$work/mise-project/mise.toml"
  (cd "$work/mise-project" && "$mise" trust --yes mise.toml && "$mise" install && sh "$smoke" "$("$mise" which reylo)" "$version")
  done_
}

case "$platform" in
  darwin-*)
    step "install.sh"
    REYLO_INSTALL_DIR="$work/sh" sh site/public/install.sh > "$work/out" || { cat "$work/out"; exit 1; }
    installed "$work/out"
    sh "$smoke" "$work/sh/reylo" "$version"
    done_
    homebrew
    mise_check
    ;;

  linux-*)
    # Each image as it comes, but for curl, which Debian's slim image lacks and install.sh needs one
    # of (or wget), and tar, which Amazon Linux's lacks; Debian 11's security updates have moved to archive.debian.org, where apt is sent
    # when the usual mirror no longer has them. The host's network, so a server of the run's own on
    # 127.0.0.1 is reachable.
    for image in $(jq -r '.[]' <<< "$images"); do
      step "install.sh in $image"
      docker run --rm --network host -v "$PWD:/w" -w /w -e REYLO_VERSION -e REYLO_DOWNLOAD_URL "$image" sh -c '
        set -e
        if ! command -v curl > /dev/null && ! command -v wget > /dev/null; then
          get() { apt-get update -qq > /dev/null && apt-get install -y -qq --no-install-recommends curl ca-certificates > /dev/null; }
          get || { sed -i "s|deb.debian.org/debian-security|archive.debian.org/debian-security|" /etc/apt/sources.list && get; }
        fi
        # Amazon Linux 2023 has no tar, which install.sh refuses to go on without.
        command -v tar > /dev/null || dnf install -y -q tar gzip > /dev/null
        REYLO_INSTALL_DIR=/tmp/reylo sh site/public/install.sh
        sh .github/scripts/channels-smoke.sh /tmp/reylo/reylo "$0"
      ' "$version" > "$work/out" || { cat "$work/out"; exit 1; }
      installed "$work/out"
      done_
    done
    case "$platform" in
      *-musl)
        # The container image, built from this platform's archive as release.yml builds it.
        step "Container image"
        arch=amd64
        case "$platform" in linux-arm64-*) arch=arm64 ;; esac
        (cd "$assets" && grep " reylo-$platform.tar.gz\$" SHA256SUMS | sha256sum -c -)
        mkdir -p "$work/image/$arch"
        tar -xzf "$assets/reylo-$platform.tar.gz" -C "$work/image/$arch"
        docker build -f .github/docker/Dockerfile --build-arg VERSION="$version" -t reylo-check "$work/image"
        got="$(docker run --rm reylo-check --version)"
        [ "$got" = "reylo $version" ] || { echo "::error::the image's reylo --version printed '$got'"; exit 1; }
        mkdir -p "$work/db" && : > "$work/db/app.db"
        docker run --rm -v "$work/db:/work" reylo-check run "create table users (id integer primary key, email text not null) strict" --url sqlite:app.db
        docker run --rm -v "$work/db:/work" reylo-check analyze --sql "select 1 as one, email from users" --url sqlite:app.db --format json > "$work/out"
        cat "$work/out"
        grep -q '"cardinality":"many"' "$work/out"
        done_
        ;;
      *)
        homebrew
        mise_check
        ;;
    esac
    ;;

  win32-*)
    # As a user runs it, `irm … | iex`, which reads the script as one string.
    for shell in pwsh powershell; do
      step "install.ps1 under $shell"
      REYLO_INSTALL_DIR="$(cygpath -w "$work/$shell")" REYLO_NO_MODIFY_PATH=1 \
        "$shell" -NoProfile -ExecutionPolicy Bypass -Command "Get-Content -Raw site/public/install.ps1 | Invoke-Expression" > "$work/out" || { cat "$work/out"; exit 1; }
      installed "$work/out"
      sh "$smoke" "$work/$shell/reylo.exe" "$version"
      done_
    done
    step "Scoop"
    pwsh -NoProfile -Command "Invoke-RestMethod https://get.scoop.sh -OutFile '$(cygpath -w "$work/scoop.ps1")'; & '$(cygpath -w "$work/scoop.ps1")' -RunAsAdmin"
    "$HOME/scoop/shims/scoop.cmd" install "$(cygpath -w "$assets/bucket/reylo.json")"
    sh "$smoke" "$HOME/scoop/shims/reylo.exe" "$version"
    done_
    mise_check
    ;;

  *)
    echo "::error::channels-check.sh knows no platform $platform"
    exit 1
    ;;
esac
rm -rf "$work"
