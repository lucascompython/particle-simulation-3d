#!/bin/bash

TARGET=""
BUILD_WASM=false
NATIVE_OPT=false
PUBLIC_URL=""
CI_MODE=false

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
        --ci)
            CI_MODE=true
            ;;
        native)
            NATIVE_OPT=true
            ;;
        *-*-*)  # matches typical rust targets like x86_64-unknown-linux-gnu
            TARGET="$1"
            ;;
        *)
            echo "Unknown parameter: $1"
            echo "Usage: $0 [--target=<target>] [--wasm] [--native] [--public-url=<url>] [--ci]"
            exit 1
            ;;
    esac
    shift
done

TRUNK_CMD="trunk"
if [ "$CI_MODE" = true ]; then
    TRUNK_CMD="./trunk"
    echo "CI mode enabled: Using local trunk binary at $TRUNK_CMD"
    if [ ! -x "$TRUNK_CMD" ]; then
        echo "Error: CI mode specified, but '$TRUNK_CMD' not found or not executable."
        exit 1
    fi
fi


if [ -z "$TARGET" ] && [ "$BUILD_WASM" = false ]; then
    echo "Error: At least a target platform or the --wasm flag is required"
    echo "Usage: $0 [--target=<target>] [--wasm] [--native] [--public-url=<url>] [--ci]"
    echo "Examples:"
    echo "  $0 --target=x86_64-unknown-linux-gnu"
    echo "  $0 x86_64-unknown-linux-gnu --native"
    echo "  $0 --wasm"
    echo "  $0 --wasm --public-url=https://example.com"
    echo "  $0 --wasm --ci"
    exit 1
fi

BASE_RUSTFLAGS="-Csymbol-mangling-version=v0 -Zlocation-detail=none -Zfmt-debug=none"

mv .cargo/.config.toml .cargo/config.toml

if [ "$BUILD_WASM" = true ]; then
    echo "Building particle-simulation for web..."
    WASM_RUSTFLAGS="$BASE_RUSTFLAGS -C target-feature=-nontrapping-fptoint"
    if [ -n "$PUBLIC_URL" ]; then
        echo "Using public URL: $PUBLIC_URL"
        RUSTFLAGS="$WASM_RUSTFLAGS" "$TRUNK_CMD" build --release --public-url "$PUBLIC_URL"
    else
        RUSTFLAGS="$WASM_RUSTFLAGS" "$TRUNK_CMD" build --release
    fi
    if [ $? -ne 0 ]; then
        echo "Error: trunk build failed."
        mv .cargo/config.toml .cargo/.config.toml
        exit 1
    fi
fi

if [ -n "$TARGET" ]; then
    NATIVE_RUSTFLAGS="$BASE_RUSTFLAGS"
    if [ "$NATIVE_OPT" = true ]; then
        echo "Building particle-simulation for $TARGET with native CPU optimizations..."
        NATIVE_RUSTFLAGS="$NATIVE_RUSTFLAGS -C target-cpu=native"
    else
        echo "Building particle-simulation for $TARGET..."
    fi

    NATIVE_RUSTFLAGS="$NATIVE_RUSTFLAGS -Clink-args=-fuse-ld=lld -Clink-args=-Wl,--icf=all"

    RUSTFLAGS="$NATIVE_RUSTFLAGS" cargo +nightly build --target "$TARGET" --release

    if [ $? -ne 0 ]; then
        echo "Error: cargo build failed."
        mv .cargo/config.toml .cargo/.config.toml
        exit 1
    fi
fi

mv .cargo/config.toml .cargo/.config.toml

echo "Build finished successfully."
