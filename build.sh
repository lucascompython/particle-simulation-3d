#!/bin/bash

TARGET=""
BUILD_WASM=false
NATIVE_OPT=false
PUBLIC_URL=""

while [[ "$#" -gt 0 ]]; do
    case $1 in
        --target=*)
            TARGET="${1#*=}"
            ;;
        --target)
            TARGET="$2"
            shift
            ;;
        --wasm)
            BUILD_WASM=true
            ;;
        --native)
            NATIVE_OPT=true
            ;;
        --public-url=*)
            PUBLIC_URL="${1#*=}"
            ;;
        --public-url)
            PUBLIC_URL="$2"
            shift
            ;;
        native)
            NATIVE_OPT=true
            ;;
        *-*-*)  # matches typical rust targets like x86_64-unknown-linux-gnu
            TARGET="$1"
            ;;
        *)
            echo "Unknown parameter: $1"
            echo "Usage: $0 [--target=<target>] [--wasm] [--native] [--public-url=<url>]"
            exit 1
            ;;
    esac
    shift
done

RUSTFLAGS="-Csymbol-mangling-version=v0 -Zlocation-detail=none -Zfmt-debug=none"

if [ -z "$TARGET" ] && [ "$BUILD_WASM" = false ]; then
    echo "Error: At least a target platform or the --wasm flag is required"
    echo "Usage: $0 [--target=<target>] [--wasm] [--native] [--public-url=<url>]"
    echo "Examples:"
    echo "  $0 --target=x86_64-unknown-linux-gnu"
    echo "  $0 x86_64-unknown-linux-gnu --native"
    echo "  $0 --wasm"
    echo "  $0 --wasm --public-url=https://example.com"
    exit 1
fi

mv .cargo/.config.toml .cargo/config.toml

if [ "$BUILD_WASM" = true ]; then
    echo "Building particle-simulation for web..."
    if [ -n "$PUBLIC_URL" ]; then
        echo "Using public URL: $PUBLIC_URL"
        RUSTFLAGS="$RUSTFLAGS -C target-feature=-nontrapping-fptoint" trunk build --release --public-url "$PUBLIC_URL"
    else
        RUSTFLAGS="$RUSTFLAGS -C target-feature=-nontrapping-fptoint" trunk build --release
    fi
fi

if [ -n "$TARGET" ]; then
    if [ "$NATIVE_OPT" = true ]; then
        echo "Building particle-simulation for $TARGET with native CPU optimizations..."
        RUSTFLAGS="$RUSTFLAGS -C target-cpu=native"
    else
        echo "Building particle-simulation for $TARGET..."
    fi

    RUSTFLAGS="$RUSTFLAGS -Clink-args=-fuse-ld=lld -Clink-args=-Wl,--icf=all" cargo +nightly build --target $TARGET --release
fi

mv .cargo/config.toml .cargo/.config.toml
