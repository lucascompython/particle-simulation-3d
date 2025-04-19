param(
    [switch]$native,
    [string]$publicUrl = "",
    [switch]$wasm
)

$TARGET = "x86_64-pc-windows-msvc"

$env:RUSTFLAGS = "-Csymbol-mangling-version=v0 -Zlocation-detail=none -Zfmt-debug=none"

mv .cargo/.config.toml .cargo/config.toml

if ($wasm -or $publicUrl -ne "") {
    Write-Host "Building particle-simulation for web..."
    if ($publicUrl -ne "") {
        Write-Host "Using public URL: $publicUrl"
        $env:RUSTFLAGS += " -C target-feature=-nontrapping-fptoint"
        & trunk build --release --public-url $publicUrl
    } else {
        $env:RUSTFLAGS += " -C target-feature=-nontrapping-fptoint"
        & trunk build --release
    }
}

if ($native) {
    Write-Host "Building particle-simulation for $TARGET with native CPU optimizations..."
    $env:RUSTFLAGS += " -C target-cpu=native"
} else {
    Write-Host "Building particle-simulation for $TARGET..."
}

$env:RUSTFLAGS += " -Clink-args=-fuse-ld=lld -Clink-args=-Wl,--icf=all"

cargo +nightly build --target $TARGET --release

mv .cargo/config.toml .cargo/.config.toml

exit $LASTEXITCODE
