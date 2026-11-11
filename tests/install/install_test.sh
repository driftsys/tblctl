#!/bin/sh
# Tests for install.sh: target detection, asset naming, checksum verification,
# and extraction from a cargo-dist shaped archive. No network.
#
# Run with: sh tests/install/install_test.sh   (or `just install-test`)
# shellcheck disable=SC3043,SC2329
set -eu

root="$(cd "$(dirname "$0")/../.." && pwd)"
export TBLCTL_INSTALL_NO_MAIN=1
# shellcheck source=install.sh
. "$root/install.sh"
set +e

pass=0
fail=0
ok() { pass=$((pass + 1)); }
ko() { fail=$((fail + 1)); echo "FAIL: $*" >&2; }
expect_eq() { if [ "$1" = "$2" ]; then ok; else ko "$3: expected '$1', got '$2'"; fi; }

with_uname() {
  UNAME_S="$1" UNAME_M="$2"
  uname() { case "$1" in -s) echo "$UNAME_S" ;; -m) echo "$UNAME_M" ;; esac; }
}

with_uname Linux x86_64;   expect_eq x86_64-unknown-linux-musl  "$(detect_target)" "linux x86_64"
with_uname Linux aarch64;  expect_eq aarch64-unknown-linux-musl "$(detect_target)" "linux aarch64"
with_uname Darwin x86_64;  expect_eq x86_64-apple-darwin        "$(detect_target)" "macos x86_64"
with_uname Darwin arm64;   expect_eq aarch64-apple-darwin       "$(detect_target)" "macos arm64"
with_uname Windows_NT x86_64
if (detect_target) >/dev/null 2>&1; then ko "unsupported OS accepted"; else ok; fi
with_uname Linux riscv64
if (detect_target) >/dev/null 2>&1; then ko "unsupported arch accepted"; else ok; fi
unset -f uname

# Asset names must match dist-workspace.toml (unix-archive = ".tar.gz").
expect_eq "tblctl-x86_64-unknown-linux-musl.tar.gz" "$(asset_name x86_64-unknown-linux-musl)" "asset name"
if grep -q '^unix-archive = ".tar.gz"' "$root/dist-workspace.toml"; then ok; else ko "dist-workspace.toml unix-archive is not .tar.gz"; fi

export TBLCTL_VERSION=v9.9.9
expect_eq v9.9.9 "$(get_version)" "pinned version"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/tblctl-x86_64-apple-darwin"
printf '#!/bin/sh\necho tblctl 9.9.9\n' > "$tmp/tblctl-x86_64-apple-darwin/tblctl"
(cd "$tmp" && tar czf a.tar.gz tblctl-x86_64-apple-darwin)
if command -v sha256sum >/dev/null 2>&1; then (cd "$tmp" && sha256sum a.tar.gz > a.tar.gz.sha256)
else (cd "$tmp" && shasum -a 256 a.tar.gz > a.tar.gz.sha256); fi
if (cd "$tmp" && verify_checksum a.tar.gz.sha256) >/dev/null 2>&1; then ok; else ko "valid checksum rejected"; fi
cp "$tmp/a.tar.gz" "$tmp/good.tar.gz"
echo tampered >> "$tmp/a.tar.gz"
if (cd "$tmp" && verify_checksum a.tar.gz.sha256) >/dev/null 2>&1; then ko "tampered archive accepted"; else ok; fi

# The binary is found inside the cargo-dist <name>-<target>/ directory.
mkdir "$tmp/x" && tar xzf "$tmp/good.tar.gz" -C "$tmp/x"
expect_eq "$tmp/x/tblctl-x86_64-apple-darwin/tblctl" "$(find "$tmp/x" -type f -name "$BINARY" | head -1)" "binary lookup"

# Piped invocation (`curl … | sh`) must parse without errors.
if sh -n "$root/install.sh"; then ok; else ko "install.sh has syntax errors"; fi

echo "install_test: $pass passed, $fail failed"
[ "$fail" -eq 0 ]
