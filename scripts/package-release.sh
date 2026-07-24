#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: $0 <binary-path> <archive-path>" >&2
  exit 2
fi

binary_path=$1
archive_path=$2

if [[ ! -f "$binary_path" ]]; then
  echo "binary not found: $binary_path" >&2
  exit 1
fi

package_dir=$(mktemp -d)
trap 'rm -rf "$package_dir"' EXIT

mkdir -p "$(dirname "$archive_path")"
cp "$binary_path" "$package_dir/"
cp README.md LICENSE "$package_dir/"
tar -C "$package_dir" -czf "$archive_path" .

archive_dir=$(dirname "$archive_path")
archive_name=$(basename "$archive_path")
if command -v sha256sum >/dev/null 2>&1; then
  (
    cd "$archive_dir"
    sha256sum "$archive_name" >"$archive_name.sha256"
  )
else
  (
    cd "$archive_dir"
    shasum -a 256 "$archive_name" >"$archive_name.sha256"
  )
fi
