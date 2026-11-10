# justfile — the one definition of every gate. CI (.github/workflows/ci.yml)
# runs these recipes and holds no copy of any gate command.

set shell := ["bash", "-euo", "pipefail", "-c"]

# List recipes.
[private]
default:
    @just --list

# Reformat the connective tissue (prim) and Rust (cargo fmt) in place.
fmt:
    prim .
    cargo fmt --all

# Connective-tissue gate, no writes: prim formatting + floor lint.
check:
    prim fmt --check .
    prim lint .

# Rust formatting gate, no writes.
fmt-check:
    cargo fmt --all --check

# Clippy with warnings denied.
lint:
    cargo clippy --all-targets --locked -- -D warnings

# Compile; --locked refuses to rewrite a stale Cargo.lock.
compile:
    cargo build --locked

# Run the test suite.
test:
    cargo test --all --locked

# Lint the install scripts.
shellcheck:
    shellcheck -x install.sh bootstrap .githooks/pre-push-gate.sh tests/install/install_test.sh

# Test install.sh against a local fake release (no network).
install-test:
    sh tests/install/install_test.sh

# Fail when the running rustc is not the version rust-toolchain.toml pins.
toolchain-check:
    #!/usr/bin/env bash
    set -euo pipefail
    pinned="$(sed -n 's/^channel[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' rust-toolchain.toml)"
    if ! printf '%s' "$pinned" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$'; then
        echo "toolchain-check: rust-toolchain.toml must pin an exact version, got '$pinned'" >&2
        exit 1
    fi
    running="$(rustc --version | cut -d' ' -f2)"
    if [ "$running" != "$pinned" ]; then
        echo "toolchain-check: pinned $pinned, rustc reports $running" >&2
        echo "toolchain-check: install rustup or clear RUSTUP_TOOLCHAIN / rustup override" >&2
        exit 1
    fi
    echo "toolchain-check: rustc $running matches the pin."

# Lint commit messages not yet on origin/<base>.
lint-commits base="main":
    #!/usr/bin/env bash
    set -euo pipefail
    ref="origin/{{ base }}"
    if ! git rev-parse --verify --quiet "$ref" >/dev/null; then
        echo "lint-commits: $ref is missing — run 'git fetch origin'." >&2
        exit 1
    fi
    if [ -z "$(git rev-list -n 1 "$ref"..HEAD)" ]; then
        echo "lint-commits: no commits in $ref..HEAD — nothing to lint"
    else
        git std lint --range "$ref"..HEAD
    fi

# The full local gate.
build: toolchain-check check fmt-check compile test lint shellcheck install-test

# Fast gate run by the pre-push hook.
pre-push: lint-commits toolchain-check check fmt-check lint

# Commit-message lint plus the full gate. Run before opening a PR.
verify: lint-commits build

# Cut a release commit + tag locally with git-std (changelog, version bump).
# Requires a clean main equal to origin/main whose `ci` run passed. Pass
# git-std options, e.g. `just release --first-release`. Publish with
# `just release-push`.
release *ARGS: toolchain-check
    #!/usr/bin/env bash
    set -euo pipefail
    [ "$(git rev-parse --abbrev-ref HEAD)" = main ] || { echo "release: check out main first" >&2; exit 1; }
    [ -z "$(git status --porcelain)" ] || { echo "release: the working tree is not clean" >&2; exit 1; }
    git fetch origin main --quiet
    [ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] || { echo "release: main is not equal to origin/main" >&2; exit 1; }
    ci="$(gh run list --workflow ci --commit "$(git rev-parse HEAD)" \
        --json status,conclusion --jq '.[0] | if . == null then "no run" elif .status != "completed" then .status else .conclusion end')"
    [ "$ci" = success ] || { echo "release: ci has not passed on HEAD ($ci)" >&2; exit 1; }
    git std bump {{ ARGS }}
    cargo metadata --locked --format-version 1 >/dev/null
    echo "release: review the commit and tag, then run 'just release-push'"

# Push the release commit and its tag; the tag triggers release.yml.
release-push:
    git push --follow-tags origin main

# Set up a fresh clone.
install:
    ./bootstrap

# Remove build artifacts.
clean:
    rm -rf target
