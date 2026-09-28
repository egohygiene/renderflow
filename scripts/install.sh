#!/usr/bin/env sh
set -eu

REPO="${RENDERFLOW_REPO:-egohygiene/renderflow}"
VERSION="${RENDERFLOW_VERSION:-}"
INSTALL_DIR="${RENDERFLOW_INSTALL_DIR:-/usr/local/bin}"

log() {
  printf '%s\n' "$*"
}

err() {
  printf 'renderflow installer: %s\n' "$*" >&2
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1
}

download() {
  src="$1"
  dest="$2"
  case "$src" in
    file://*)
      cp "${src#file://}" "$dest"
      return
      ;;
  esac

  if need_cmd curl; then
    curl --proto '=https' --tlsv1.2 --fail --silent --show-error --location "$src" --output "$dest"
  elif need_cmd wget; then
    wget -qO "$dest" "$src"
  else
    err "missing downloader (need curl or wget)"
    exit 1
  fi
}

checksum_file() {
  file="$1"
  if need_cmd sha256sum; then
    sha256sum "$file" | awk '{print $1}'
  elif need_cmd shasum; then
    shasum -a 256 "$file" | awk '{print $1}'
  else
    err "missing checksum tool (need sha256sum or shasum)"
    exit 1
  fi
}

is_install_dir_writable() {
  dir="$1"
  if [ -d "$dir" ]; then
    [ -w "$dir" ]
    return
  fi

  parent_dir="$(dirname "$dir")"
  [ -d "$parent_dir" ] && [ -w "$parent_dir" ]
}

detect_target() {
  os="$(uname -s | tr '[:upper:]' '[:lower:]')"
  arch="$(uname -m)"

  if [ "$os" != "linux" ] || [ "$arch" != "x86_64" ]; then
    err "no verified release binary for $os/$arch; only x86_64-unknown-linux-gnu is supported"
    exit 1
  fi
  libc="$(getconf GNU_LIBC_VERSION 2>/dev/null || true)"
  case "$libc" in
    "glibc "*) libc_version="${libc#glibc }" ;;
    *) err "this candidate requires GNU libc 2.39 or newer"; exit 1 ;;
  esac
  if ! printf '%s\n' "$libc_version" | awk -F. '{exit !($1 > 2 || ($1 == 2 && $2 >= 39))}'; then
    err "this candidate requires GNU libc 2.39 or newer (found $libc_version)"
    exit 1
  fi
  printf '%s' "x86_64-unknown-linux-gnu"
}

resolve_base_url() {
  if [ -n "${RENDERFLOW_DOWNLOAD_BASE_URL:-}" ]; then
    printf '%s' "${RENDERFLOW_DOWNLOAD_BASE_URL%/}"
    return
  fi

  printf 'https://github.com/%s/releases/download/%s' "$REPO" "$tag"
}

main() {
  case "$VERSION" in
    ""|latest|*[!a-zA-Z0-9.+-]*)
      err "set RENDERFLOW_VERSION to an exact release tag (for example v0.3.0-rc.1)"
      exit 1
      ;;
    v*) tag="$VERSION"; expected_version="${VERSION#v}" ;;
    *) tag="v$VERSION"; expected_version="$VERSION" ;;
  esac
  if ! printf '%s\n' "$expected_version" | LC_ALL=C grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z]+([.-][0-9A-Za-z]+)*)?(\+[0-9A-Za-z]+([.-][0-9A-Za-z]+)*)?$'; then
    err "RENDERFLOW_VERSION is not an exact SemVer release tag"
    exit 1
  fi
  target="$(detect_target)"
  base_url="$(resolve_base_url)"
  asset="renderflow-$target"
  checksum_asset="$asset.sha256"

  tmp_dir="$(mktemp -d)"
  trap 'rm -rf "$tmp_dir"' EXIT INT TERM

  bin_path="$tmp_dir/$asset"
  checksum_path="$tmp_dir/$checksum_asset"

  log "Installing Renderflow for target: $target"
  log "Candidate verified on Ubuntu 24.04 x86_64 GNU; other distro baselines remain unverified."
  log "Downloading: $base_url/$asset"
  download "$base_url/$asset" "$bin_path"
  log "Downloading checksum: $base_url/$checksum_asset"
  download "$base_url/$checksum_asset" "$checksum_path"

  if [ "$(wc -l < "$checksum_path" | tr -d ' ')" != "1" ]; then
    err "checksum file must have exactly one newline-terminated entry"
    exit 1
  fi
  read -r expected checksum_name extra < "$checksum_path"
  case "$expected" in
    *[!0-9a-f]*|"") err "checksum is not lowercase SHA-256"; exit 1 ;;
  esac
  if [ "${#expected}" -ne 64 ] || [ "$checksum_name" != "$asset" ] || [ -n "${extra:-}" ]; then
    err "checksum record does not identify the exact release asset"
    exit 1
  fi
  actual="$(checksum_file "$bin_path")"
  if [ "$expected" != "$actual" ]; then
    err "checksum verification failed for $asset"
    err "expected: $expected"
    err "actual:   $actual"
    exit 1
  fi
  log "Checksum verification passed."

  chmod 0755 "$bin_path"
  observed_version="$("$bin_path" --version)"
  if [ "$observed_version" != "renderflow $expected_version" ]; then
    err "downloaded binary version differs from pinned release: $observed_version"
    exit 1
  fi

  if ! is_install_dir_writable "$INSTALL_DIR" && [ -z "${RENDERFLOW_INSTALL_DIR:-}" ]; then
    INSTALL_DIR="${HOME}/.local/bin"
    log "No write access to /usr/local/bin; falling back to $INSTALL_DIR"
  fi

  mkdir -p "$INSTALL_DIR"
  install -m 0755 "$bin_path" "$INSTALL_DIR/renderflow"
  log "Installed renderflow to $INSTALL_DIR/renderflow"
  log "$observed_version"
}

main "$@"
