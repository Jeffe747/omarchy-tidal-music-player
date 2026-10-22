#!/usr/bin/env bash

# Check the published release against the exact tagged source, not just local pins.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

command -v gh >/dev/null || { echo "gh is required to inspect published assets" >&2; exit 1; }
tag="v$(jq -r .version manifest.json)"
repo="$(jq -r '.homepage | sub("^https://github.com/"; "")' manifest.json)"
tag_commit="$(git rev-list -n 1 "$tag")"
if [[ "$(git rev-parse HEAD)" != "$tag_commit" ]]; then
  echo "Checkout must be the exact published $tag commit ($tag_commit)" >&2
  exit 1
fi

release="$(gh release view "$tag" --repo "$repo" --json assets)"
for arch in x86_64 aarch64; do
  asset="tidal-daemon-$arch"
  digest="$(awk -v name="$asset" '$2 == name { print $1 }' SHA256SUMS)"
  if [[ ! "$digest" =~ ^[0-9a-f]{64}$ ]]; then
    echo "Missing or malformed checksum for $asset" >&2
    exit 1
  fi
  published="$(jq -er --arg name "$asset" \
    '[.assets[] | select(.name == $name)] | if length == 1 then .[0].digest else error("missing or duplicate release asset") end' \
    <<< "$release")"
  if [[ "$published" != "sha256:$digest" ]]; then
    echo "$asset differs from published release: $digest != $published" >&2
    exit 1
  fi
  if [[ "$arch" == x86_64 ]]; then
    selection="x86_64|amd64) expected=\"$digest\""
    if [[ "$(sha256sum bin/tidal-daemon | cut -d' ' -f1)" != "$digest" ]]; then
      echo "Bundled x86_64 binary differs from published release" >&2
      exit 1
    fi
  else
    selection="aarch64|arm64) expected=\"$digest\""
  fi
  if ! grep -Fq "$selection" Service.qml \
      || ! grep -Fq "asset=$asset; expected=$digest" Service.qml; then
    echo "Runtime selection or bootstrap pin differs from published $asset" >&2
    exit 1
  fi
done

checksums_digest="$(sha256sum SHA256SUMS | cut -d' ' -f1)"
published_checksums="$(jq -er '[.assets[] | select(.name == "SHA256SUMS")] | if length == 1 then .[0].digest else error("missing or duplicate release checksums") end' <<< "$release")"
if [[ "$published_checksums" != "sha256:$checksums_digest" ]]; then
  echo "Published SHA256SUMS differs from tagged source" >&2
  exit 1
fi

echo "Published $tag assets, bundled binary, both runtime pins and checksums match the tagged source."
