#!/bin/sh
# Install tblctl — downloads a release binary for the current platform from
# GitHub and verifies its SHA-256 checksum.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/driftsys/tblctl/main/install.sh | sh
#
# Environment variables:
#   TBLCTL_INSTALL_DIR  Installation directory (default: $HOME/.local/bin)
#   TBLCTL_VERSION      Release tag to install, e.g. v0.0.1 (default: latest)

# `local` (SC3043) is not in POSIX, but every real /bin/sh — dash, busybox
# ash — supports it.
# shellcheck disable=SC3043
set -eu

REPO="driftsys/tblctl"
INSTALL_DIR="${TBLCTL_INSTALL_DIR:-$HOME/.local/bin}"
BINARY="tblctl"

detect_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux)
      case "$arch" in
        x86_64)  echo "x86_64-unknown-linux-musl" ;;
        aarch64 | arm64) echo "aarch64-unknown-linux-musl" ;;
        *) echo "error: unsupported architecture: $arch" >&2; return 1 ;;
      esac
      ;;
    Darwin)
      case "$arch" in
        x86_64) echo "x86_64-apple-darwin" ;;
        arm64)  echo "aarch64-apple-darwin" ;;
        *) echo "error: unsupported architecture: $arch" >&2; return 1 ;;
      esac
      ;;
    *) echo "error: unsupported OS: $os (on Windows use install.ps1)" >&2; return 1 ;;
  esac
}

get_version() {
  if [ -n "${TBLCTL_VERSION:-}" ]; then
    printf '%s\n' "$TBLCTL_VERSION"
    return
  fi

  if version=$(gh release view --repo "$REPO" --json tagName -q .tagName 2>/dev/null); then
    printf '%s\n' "$version"
    return
  fi

  # gh unavailable — fall back to the REST API. Capture the fallible curl on
  # its own line so `set -e` catches a failure (pipefail is not POSIX).
  body=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest") \
    || { echo "error: could not reach GitHub API" >&2; exit 1; }
  printf '%s\n' "$body" | grep '"tag_name"' | head -1 | cut -d'"' -f4
}

verify_checksum() {
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 -c "$1"
  elif command -v sha256sum >/dev/null 2>&1; then
    sha256sum -c "$1"
  else
    echo "error: SHA-256 verification requires shasum or sha256sum; install one and retry" >&2
    return 1
  fi
}

asset_name() {
  printf '%s-%s.tar.gz\n' "$BINARY" "$1"
}

main() {
  local target version tarball url bin
  target="$(detect_target)"
  version="$(get_version)"
  [ -n "$version" ] || { echo "error: could not determine the release to install" >&2; exit 1; }
  tarball="$(asset_name "$target")"
  url="https://github.com/$REPO/releases/download/$version/$tarball"

  echo "Installing $BINARY $version ($target)" >&2
  echo "  to: $INSTALL_DIR" >&2

  # `tmpdir` is global: the EXIT trap fires at global scope, where a
  # function-local would be unbound under `set -u`.
  tmpdir="$(mktemp -d)"
  trap 'rm -rf "${tmpdir:-}"' EXIT

  curl -fsSL "$url" -o "$tmpdir/$tarball" \
    || { echo "error: download failed — no $tarball in release $version?" >&2; exit 1; }
  curl -fsSL "$url.sha256" -o "$tmpdir/$tarball.sha256" \
    || { echo "error: checksum download failed" >&2; exit 1; }

  echo "Verifying checksum..." >&2
  (cd "$tmpdir" && verify_checksum "$tarball.sha256")

  tar xzf "$tmpdir/$tarball" -C "$tmpdir"
  # cargo-dist archives hold the binary under a <name>-<target>/ directory.
  bin="$(find "$tmpdir" -type f -name "$BINARY" | head -1)"
  [ -n "$bin" ] || { echo "error: $BINARY not found in $tarball" >&2; exit 1; }

  mkdir -p "$INSTALL_DIR"
  mv "$bin" "$INSTALL_DIR/$BINARY"
  chmod +x "$INSTALL_DIR/$BINARY"

  echo "Installed $INSTALL_DIR/$BINARY" >&2

  if ! echo "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
    echo "" >&2
    echo "Add to your PATH:" >&2
    echo "  export PATH=\"$INSTALL_DIR:\$PATH\"" >&2
  fi
}

# Tests source this file with TBLCTL_INSTALL_NO_MAIN=1 to load the functions.
[ "${TBLCTL_INSTALL_NO_MAIN:-}" = 1 ] || main
