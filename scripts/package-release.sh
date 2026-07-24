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

mkdir -p "$(dirname "$archive_path")"
archive_dir=$(dirname "$archive_path")
archive_name=$(basename "$archive_path")
archive_dir=$(cd "$archive_dir" && pwd -P)
archive_path="$archive_dir/$archive_name"

case "$archive_name" in
  *.tar.gz)
    package_name=${archive_name%.tar.gz}
    archive_format=tar
    ;;
  *.zip)
    package_name=${archive_name%.zip}
    archive_format=zip
    ;;
  *)
    echo "unsupported archive format: $archive_name (expected .tar.gz or .zip)" >&2
    exit 2
    ;;
esac

work_dir=$(mktemp -d)
trap 'rm -rf "$work_dir"' EXIT
package_root="$work_dir/package/$package_name"
verify_root="$work_dir/verify"
binary_name=$(basename "$binary_path")

mkdir -p "$package_root" "$verify_root"
cp "$binary_path" "$package_root/"
cp README.md LICENSE "$package_root/"

if [[ "$archive_format" == tar ]]; then
  tar -C "$work_dir/package" -czf "$archive_path" "$package_name"
elif command -v 7z >/dev/null 2>&1; then
  (
    cd "$work_dir/package"
    7z a -bd -bso0 -bsp0 -tzip "$archive_path" "$package_name"
  )
elif command -v zip >/dev/null 2>&1; then
  (
    cd "$work_dir/package"
    zip -qr "$archive_path" "$package_name"
  )
else
  echo "7z or zip is required to create $archive_name" >&2
  exit 1
fi

if [[ "$archive_format" == tar ]]; then
  tar -C "$verify_root" -xzf "$archive_path"
elif command -v 7z >/dev/null 2>&1; then
  7z x -bd -bso0 -bsp0 -o"$verify_root" "$archive_path"
elif command -v unzip >/dev/null 2>&1; then
  unzip -q "$archive_path" -d "$verify_root"
else
  echo "7z or unzip is required to verify $archive_name" >&2
  exit 1
fi

packaged_binary="$verify_root/$package_name/$binary_name"
if [[ ! -f "$packaged_binary" ]]; then
  echo "packaged binary not found: $packaged_binary" >&2
  exit 1
fi
"$packaged_binary" --version

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
