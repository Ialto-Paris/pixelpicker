#!/usr/bin/env bash
# The checks CI doesn't run by itself: run them before pushing. From the repository's root:
#
#   scripts/check.sh            format, clippy and tests
#   scripts/check.sh --release  also the release build and its start-up test
#
# Pixel Picker is a Windows program: run this on Windows (Git Bash), with the MSVC toolchain.
set -euo pipefail

release=0
for arg in "$@"; do
  case "$arg" in
    --release) release=1 ;;
    -h | --help) sed -n '2,7p' "$0"; exit 0 ;;
    *) echo "unknown option: $arg (see --help)" >&2; exit 2 ;;
  esac
done

export CARGO_INCREMENTAL=0
step() { printf '\n== %s\n' "$*"; }

step "format"
cargo fmt --all -- --check

step "clippy"
cargo clippy --all-targets --locked -- -D warnings

step "tests"
cargo test --locked

if [ "$release" = 1 ]; then
  step "release build"
  cargo build --release --locked
  step "start-up test"
  powershell.exe -NoProfile -ExecutionPolicy Bypass -File .github/scripts/smoke-test.ps1 \
    target/release/PixelPicker.exe
fi

printf '\nAll checks passed.\n'
