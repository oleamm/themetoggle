<#
.SYNOPSIS
    Builds the release binary and stages it as dist\ThemeToggle.exe.

.DESCRIPTION
    Local counterpart of the CI release job. It refuses to publish when the
    binary is stale, when it is not a WINDOWS_GUI executable (a console build
    would flash a console window for a tray app), or when the build identity
    baked in by build.rs is missing from the binary.
#>
param(
    [switch]$UseExistingBuild
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
$sourceExe = Join-Path $repoRoot 'target\release\themetoggle.exe'
$distDir = Join-Path $repoRoot 'dist'
$canonicalExe = Join-Path $distDir 'ThemeToggle.exe'

$running = Get-Process -Name themetoggle -ErrorAction SilentlyContinue
if ($running) {
    $details = ($running | ForEach-Object { "PID=$($_.Id)" }) -join ', '
    throw "Close ThemeToggle before publishing ($details)."
}

if (-not $UseExistingBuild) {
    if (-not (Test-Path -LiteralPath $cargo -PathType Leaf)) {
        throw "cargo was not found: $cargo"
    }
    Push-Location $repoRoot
    try {
        & $cargo build --release
        if ($LASTEXITCODE -ne 0) {
            throw "cargo build failed with exit code $LASTEXITCODE"
        }
    }
    finally {
        Pop-Location
    }
}

if (-not (Test-Path -LiteralPath $sourceExe -PathType Leaf)) {
    throw "cargo reported success but the binary is missing: $sourceExe"
}

$inputs = Get-ChildItem -LiteralPath (Join-Path $repoRoot 'src') -Recurse -File
$inputs += Get-ChildItem -LiteralPath (Join-Path $repoRoot 'assets') -Recurse -File
$inputs += Get-Item -LiteralPath (Join-Path $repoRoot 'Cargo.toml')
$inputs += Get-Item -LiteralPath (Join-Path $repoRoot 'build.rs')
$inputs += Get-Item -LiteralPath (Join-Path $repoRoot 'app.manifest')
$newestInput = $inputs | Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1
if ((Get-Item -LiteralPath $sourceExe).LastWriteTimeUtc -lt $newestInput.LastWriteTimeUtc) {
    throw "Release binary is stale. Newest input: $($newestInput.FullName)"
}

$bytes = [System.IO.File]::ReadAllBytes($sourceExe)
$peOffset = [BitConverter]::ToInt32($bytes, 0x3c)
$subsystem = [BitConverter]::ToUInt16($bytes, $peOffset + 92)
if ($subsystem -ne 2) {
    throw "Expected WINDOWS_GUI PE subsystem 2, got $subsystem"
}

$buildIdFile = Get-ChildItem -LiteralPath (Join-Path $repoRoot 'target\release\build') -Recurse -File -Filter 'build-id.txt' |
    Sort-Object LastWriteTimeUtc -Descending |
    Select-Object -First 1
if (-not $buildIdFile) {
    throw 'build.rs did not produce a build-id.txt'
}
$buildId = (Get-Content -LiteralPath $buildIdFile.FullName -Raw).Trim()
if (-not ([System.Text.Encoding]::ASCII.GetString($bytes)).Contains($buildId)) {
    throw "The binary does not contain the expected build id: $buildId"
}

New-Item -ItemType Directory -Path $distDir -Force | Out-Null
Copy-Item -LiteralPath $sourceExe -Destination $canonicalExe -Force

[PSCustomObject]@{
    Path = $canonicalExe
    BuildId = $buildId
    SizeBytes = (Get-Item -LiteralPath $canonicalExe).Length
    Sha256 = (Get-FileHash -LiteralPath $canonicalExe -Algorithm SHA256).Hash
    PESubsystem = $subsystem
} | Format-List
