# Run Cargo with all compiler output, downloads, and temporary files on D:.
# Example: ./scripts/cargo.ps1 test --workspace
$ErrorActionPreference = 'Stop'
$env:CARGO_TARGET_DIR = 'D:/BuildCache/sivgo-glory/target'
$env:CARGO_HOME = 'D:/BuildCache/cargo'
$env:TMP = 'D:/BuildCache/sivgo-glory/tmp'
$env:TEMP = $env:TMP
New-Item -ItemType Directory -Force -Path $env:CARGO_TARGET_DIR, $env:CARGO_HOME, $env:TMP | Out-Null
Push-Location (Split-Path -Parent $PSScriptRoot)
try {
    $metadataJson = & cargo metadata --no-deps --format-version 1
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    $metadata = $metadataJson | ConvertFrom-Json
    if ([IO.Path]::GetFullPath($metadata.target_directory) -ne [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)) {
        throw "Unexpected Cargo target directory: $($metadata.target_directory)"
    }
    & cargo @args
    exit $LASTEXITCODE
} finally {
    Pop-Location
}
