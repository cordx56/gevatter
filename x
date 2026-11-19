#!/bin/sh

dir="$(cd "$(dirname "$0")" && pwd)"
manifest="$dir/Cargo.toml"

export RUSTC_VERSION="${RUSTC_VERSION:-"$(cat stable-version)"}"
exec cargo run --manifest-path "$manifest" -p bootstrap --bin bootstrap -- "$@"
