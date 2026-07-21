#!/usr/bin/env sh
set -eu

REPO="${RENDERFLOW_REPO:-egohygiene/renderflow}"
VERSION="${RENDERFLOW_VERSION:-latest}"
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
    curl --proto '=https' --tlsv1.2 -fsSL "$src" -o "$dest"
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

  case "$os" in
    linux) os_part="unknown-linux-gnu" ;;
    darwin) os_part="apple-darwin" ;;
    *)
      err "unsupported operating system: $os"
      exit 1
      ;;
  esac

  case "$arch" in
    x86_64|amd64) arch_part="x86_64" ;;
    aarch64|arm64) arch_part="aarch64" ;;
    *)
      err "unsupported architecture: $arch"
      exit 1
      ;;
  esac

  printf '%s-%s' "$arch_part" "$os_part"
}

resolve_base_url() {
  if [ -n "${RENDERFLOW_DOWNLOAD_BASE_URL:-}" ]; then
    printf '%s' "${RENDERFLOW_DOWNLOAD_BASE_URL%/}"
    return
  fi

  if [ "$VERSION" = "latest" ]; then
    printf 'https://github.com/%s/releases/latest/download' "$REPO"
  else
    case "$VERSION" in
      v*) tag="$VERSION" ;;
      *) tag="v$VERSION" ;;
    esac
    printf 'https://github.com/%s/releases/download/%s' "$REPO" "$tag"
  fi
}

main() {
  target="$(detect_target)"
  base_url="$(resolve_base_url)"
  asset="renderflow-$target"
  checksum_asset="$asset.sha256"

  tmp_dir="$(mktemp -d)"
  trap 'rm -rf "$tmp_dir"' EXIT INT TERM

  bin_path="$tmp_dir/$asset"
  checksum_path="$tmp_dir/$checksum_asset"

  log "Installing Renderflow for target: $target"
  log "Downloading: $base_url/$asset"
  download "$base_url/$asset" "$bin_path"
  log "Downloading checksum: $base_url/$checksum_asset"
  download "$base_url/$checksum_asset" "$checksum_path"

  expected="$(awk '{print $1}' "$checksum_path")"
  actual="$(checksum_file "$bin_path")"
  if [ "$expected" != "$actual" ]; then
    err "checksum verification failed for $asset"
    err "expected: $expected"
    err "actual:   $actual"
    exit 1
  fi
  log "Checksum verification passed."

  if ! is_install_dir_writable "$INSTALL_DIR" && [ -z "${RENDERFLOW_INSTALL_DIR:-}" ]; then
    INSTALL_DIR="${HOME}/.local/bin"
    log "No write access to /usr/local/bin; falling back to $INSTALL_DIR"
  fi

  mkdir -p "$INSTALL_DIR"
  install -m 0755 "$bin_path" "$INSTALL_DIR/renderflow"
  log "Installed renderflow to $INSTALL_DIR/renderflow"

  if command -v "$INSTALL_DIR/renderflow" >/dev/null 2>&1; then
    "$INSTALL_DIR/renderflow" --version || true
  fi
}

main "$@"
