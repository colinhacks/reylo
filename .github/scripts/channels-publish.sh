#!/usr/bin/env bash
# Publishes a release to the package managers, from the GitHub release's own assets: the Homebrew
# formula into the tap, the Scoop manifest into the bucket (each written by channels.sh from the
# release's SHA256SUMS), and the container image, built from the release's musl archives, to GHCR.
# release.yml's `finish` runs it once the release exists; channels.yml has installed through each
# channel from the same binaries before anything was staged.
#
#   .github/scripts/channels-publish.sh <version>
#
# The tap and the bucket are written only when <version> is the repository's latest release, so a
# prerelease, or a dispatch that rewrites an older release's notes, never moves them back; the image
# takes every version's tag, and `latest` and `<major>.<minor>` only on the latest release. Running it
# again publishes the same files and tags, and commits nothing new.
#
# Environment: GH_TOKEN (the job's token, with packages: write), HOMEBREW_TAP_KEY and SCOOP_BUCKET_KEY
# (deploy keys with write access to the tap and the bucket), GITHUB_REPOSITORY and RUNNER_TEMP.
set -euo pipefail

version="${1:?usage: channels-publish.sh <version>}"
version="${version#v}"
tap=reylosql/homebrew-tap
bucket=reylosql/scoop-bucket
image="ghcr.io/${GITHUB_REPOSITORY,,}"
work="$RUNNER_TEMP/channels"
rm -rf "$work"
mkdir -p "$work/assets"
trap 'rm -rf "$work"' EXIT

gh release download "v$version" -R "$GITHUB_REPOSITORY" -D "$work/assets" -p 'reylo-*.tar.gz' -p SHA256SUMS
(cd "$work/assets" && sha256sum -c SHA256SUMS)
.github/scripts/channels.sh "$version" "$work/assets/SHA256SUMS" "$work/out"

latest="$(gh api "repos/$GITHUB_REPOSITORY/releases/latest" --jq .tag_name)"

# One repository's file, committed and pushed with that repository's deploy key, only where it changed.
push_file() {
  local repo="$1" key="$2" file="$3"
  [ -n "$key" ] || { echo "::error::no deploy key for $repo: its secret on the release-approval environment is empty"; exit 1; }
  printf '%s\n' "$key" > "$work/key"
  chmod 600 "$work/key"
  echo "github.com ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl" > "$work/known_hosts"
  export GIT_SSH_COMMAND="ssh -i $work/key -o IdentitiesOnly=yes -o UserKnownHostsFile=$work/known_hosts"
  rm -rf "$work/repo"
  git clone --quiet --depth 1 "git@github.com:$repo.git" "$work/repo"
  mkdir -p "$work/repo/$(dirname "$file")"
  cp "$work/out/$file" "$work/repo/$file"
  if [ -z "$(git -C "$work/repo" status --porcelain)" ]; then
    echo "$repo already has $file for $version"
  else
    git -C "$work/repo" add "$file"
    git -C "$work/repo" -c user.name="github-actions[bot]" -c user.email="41898282+github-actions[bot]@users.noreply.github.com" \
      commit --quiet -m "reylo $version"
    git -C "$work/repo" push --quiet origin HEAD
    echo "$repo: $file is reylo $version"
  fi
  rm -f "$work/key"
  unset GIT_SSH_COMMAND
}

if [ "$latest" = "v$version" ]; then
  push_file "$tap" "${HOMEBREW_TAP_KEY:-}" Formula/reylo.rb
  push_file "$bucket" "${SCOOP_BUCKET_KEY:-}" bucket/reylo.json
else
  echo "v$version is not the latest release ($latest), so the tap and the bucket keep theirs"
fi

# The image of both CPUs, from the release's static musl archives. A Dockerfile that only copies needs
# no emulation to build for the other CPU.
for cpu in x64 arm64; do
  arch=amd64
  [ "$cpu" = arm64 ] && arch=arm64
  mkdir -p "$work/image/$arch"
  tar -xzf "$work/assets/reylo-linux-$cpu-musl.tar.gz" -C "$work/image/$arch"
done
tags=(--tag "$image:$version")
if [ "$latest" = "v$version" ]; then
  minor="$(cut -d. -f1-2 <<< "$version")"
  tags+=(--tag "$image:$minor" --tag "$image:latest")
fi
printf '%s' "$GH_TOKEN" | docker login ghcr.io --username "$GITHUB_ACTOR" --password-stdin
docker buildx create --use --name reylo-release > /dev/null
docker buildx build --platform linux/amd64,linux/arm64 --file .github/docker/Dockerfile \
  --build-arg VERSION="$version" "${tags[@]}" --push "$work/image"
docker logout ghcr.io > /dev/null
echo "pushed ${tags[*]}"
