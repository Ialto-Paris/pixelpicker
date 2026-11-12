#!/usr/bin/env bash
# Packages the release build into dist/, for CI artifacts and releases:
#   dist/PixelPicker-windows-<arch>.exe
# Run after `cargo build --release`, from the repository's root.
set -euo pipefail

case "$(uname -m)" in
  x86_64 | amd64) arch=x64 ;;
  arm64 | aarch64) arch=arm64 ;;
  *) echo "Unknown machine: $(uname -m)" >&2; exit 1 ;;
esac

case "${RUNNER_OS:-$(uname -s)}" in
  Windows | MINGW* | MSYS*) ;;
  *) echo "Pixel Picker is a Windows program: package it on Windows" >&2; exit 1 ;;
esac

rm -rf dist
mkdir -p dist
cp target/release/PixelPicker.exe "dist/PixelPicker-windows-$arch.exe"
ls -l dist
