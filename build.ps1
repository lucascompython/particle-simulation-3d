param(
    [switch]$native,
    [string]$publicUrl = "",
    [switch]$wasm,
    [switch]$wasmRayon,
    [switch]$ci
)

$TARGET = "x86_64-pc-windows-msvc"

$trunkCmd = "trunk"
if ($ci) {
    $trunkCmd = ".\trunk"
    Write-Host "CI mode enabled: Using local trunk binary at $trunkCmd"
    if (-not (Test-Path $trunkCmd)) {
        Write-Error "Error: CI mode specified, but '$trunkCmd' not found."
        exit 1
    }
}


$env:RUSTFLAGS = "-Csymbol-mangling-version=v0 -Zlocation-detail=none -Zfmt-debug=none"


Move-Item -Path ".cargo/.config.toml" -Destination ".cargo/config.toml" -Force


if ($wasm -or $publicUrl -ne "" -or $wasmRayon) { # Check wasmRayon too
    Write-Host "Building particle-simulation for web..."

    $originalRustFlags = $env:RUSTFLAGS
    $env:RUSTFLAGS += " -C target-feature=-nontrapping-fptoint"

    $trunkArgs = @("build", "--release")
    if ($wasmRayon) {
        Write-Host "Enabling wasm-rayon feature and atomics..."
        $trunkArgs += "--features", "wasm-rayon"
        $env:RUSTFLAGS += ",+atomics"
    }
    if ($publicUrl -ne "") {
        Write-Host "Using public URL: $publicUrl"
        $trunkArgs += "--public-url", $publicUrl
    }

    try {
        & $trunkCmd $trunkArgs

        if ($LASTEXITCODE -ne 0) {
            throw "trunk build failed with exit code $LASTEXITCODE"
        }
    }
    catch {
        Write-Error "Error during trunk build: $_"

        $env:RUSTFLAGS = $originalRustFlags
        Move-Item -Path ".cargo/config.toml" -Destination ".cargo/.config.toml" -Force
        exit 1
    }
    finally {

        $env:RUSTFLAGS = $originalRustFlags
    }
}

$nativeRustFlags = $env:RUSTFLAGS
if ($native) {
    Write-Host "Building particle-simulation for $TARGET with native CPU optimizations..."
    $nativeRustFlags += " -C target-cpu=native"
} else {
    Write-Host "Building particle-simulation for $TARGET..."
}


$nativeRustFlags += " -Clink-args=-fuse-ld=lld -Clink-args=-Wl,--icf=all"


$env:RUSTFLAGS = $nativeRustFlags

try {
    cargo +nightly build --target $TARGET --release

    if ($LASTEXITCODE -ne 0) {
        throw "cargo build failed with exit code $LASTEXITCODE"
    }
}
catch {
    Write-Error "Error during cargo build: $_"

    Move-Item -Path ".cargo/config.toml" -Destination ".cargo/.config.toml" -Force
    exit 1
}


Move-Item -Path ".cargo/config.toml" -Destination ".cargo/.config.toml" -Force

Write-Host "Build finished successfully."

exit $LASTEXITCODE
