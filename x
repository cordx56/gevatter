#!/bin/sh

export RUSTC_VERSION="${RUSTC_VERSION:-"$(cat stable-version)"}"
exec cargo run --bin bootstrap -- "$@"
