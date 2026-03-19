#!/usr/bin/env bash
set -euo pipefail

warn() {
  echo "::warning::$*"
}

disable_sccache() {
  {
    echo "SCCACHE_GHA_ENABLED=false"
    echo "RUSTC_WRAPPER="
    echo "SCCACHE_DIR="
  } >> "$GITHUB_ENV"
}

cache_dir_for_runner() {
  case "$RUNNER_OS" in
    Linux)
      echo "$HOME/.cache/sccache"
      ;;
    macOS)
      echo "$HOME/Library/Caches/sccache"
      ;;
    *)
      return 1
      ;;
  esac
}

target_triple_for_runner() {
  case "$RUNNER_OS/$RUNNER_ARCH" in
    Linux/X64)
      echo "x86_64-unknown-linux-musl"
      ;;
    Linux/ARM64)
      echo "aarch64-unknown-linux-musl"
      ;;
    macOS/X64)
      echo "x86_64-apple-darwin"
      ;;
    macOS/ARM64)
      echo "aarch64-apple-darwin"
      ;;
    *)
      return 1
      ;;
  esac
}

download_asset() {
  local url="$1"
  local destination="$2"
  curl \
    --fail \
    --silent \
    --show-error \
    --location \
    --retry 5 \
    --retry-all-errors \
    --retry-delay 2 \
    --output "$destination" \
    "$url"
}

gha_cache_available() {
  [[ -n "${ACTIONS_RUNTIME_TOKEN:-}" ]] && [[ -n "${ACTIONS_CACHE_URL:-}" || -n "${ACTIONS_RESULTS_URL:-}" ]]
}

sha256_file() {
  python3 - "$1" <<'PY'
import hashlib
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
print(hashlib.sha256(path.read_bytes()).hexdigest())
PY
}

main() {
  local tag="${SCCACHE_SETUP_VERSION:-v0.12.0}"
  local version="${tag#v}"
  local triple archive archive_url checksum_url install_dir archive_path checksum_path expected actual binary_path binary_dir cache_dir

  triple="$(target_triple_for_runner)"
  archive="sccache-v${version}-${triple}.tar.gz"
  archive_url="https://github.com/mozilla/sccache/releases/download/${tag}/${archive}"
  checksum_url="${archive_url}.sha256"
  install_dir="${RUNNER_TEMP}/sccache-${version}-${triple}"
  archive_path="${install_dir}/${archive}"
  checksum_path="${archive_path}.sha256"

  if ! gha_cache_available; then
    warn "GitHub Actions cache environment was not detected"
    return 1
  fi

  rm -rf "$install_dir"
  mkdir -p "$install_dir"

  download_asset "$archive_url" "$archive_path"
  download_asset "$checksum_url" "$checksum_path"

  expected="$(tr -d '\r\n' < "$checksum_path")"
  actual="$(sha256_file "$archive_path")"
  if [[ "$expected" != "$actual" ]]; then
    warn "sccache checksum verification failed for ${archive}"
    return 1
  fi

  tar -xzf "$archive_path" -C "$install_dir"
  binary_path="$(find "$install_dir" -type f -name sccache -perm -u+x | head -n 1 || true)"
  if [[ -z "$binary_path" ]]; then
    warn "sccache binary was not found after extracting ${archive}"
    return 1
  fi

  binary_dir="$(dirname "$binary_path")"
  cache_dir="$(cache_dir_for_runner)"
  mkdir -p "$cache_dir"

  echo "$binary_dir" >> "$GITHUB_PATH"
  export PATH="$binary_dir:$PATH"
  export SCCACHE_GHA_ENABLED=true
  export RUSTC_WRAPPER=sccache
  export SCCACHE_DIR="$cache_dir"

  {
    echo "SCCACHE_GHA_ENABLED=true"
    echo "RUSTC_WRAPPER=sccache"
    echo "SCCACHE_DIR=$cache_dir"
  } >> "$GITHUB_ENV"

  sccache --version
  sccache --start-server
  sccache --show-stats >/dev/null 2>&1
}

if ! main; then
  warn "sccache setup failed; continuing without compiler caching"
  disable_sccache
fi
