param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("win-x64", "linux-arm64", "linux-amd64")]
    [string]$Suffix,

    [string]$SourceRuntime = "",
    [string]$OutputDir = "",
    [switch]$Force
)

$ErrorActionPreference = "Stop"

$repoRoot = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
if ([string]::IsNullOrWhiteSpace($SourceRuntime)) {
    $SourceRuntime = [System.IO.Path]::Combine($repoRoot, "runtime")
}
if ([string]::IsNullOrWhiteSpace($OutputDir)) {
    $OutputDir = [System.IO.Path]::Combine($repoRoot, "dist")
}

$SourceRuntime = [System.IO.Path]::GetFullPath($SourceRuntime)
$OutputDir = [System.IO.Path]::GetFullPath($OutputDir)
$manifest = [System.IO.Path]::Combine($SourceRuntime, "SHA256SUMS.$Suffix.txt")
if (-not (Test-Path -LiteralPath $manifest -PathType Leaf)) {
    throw "Missing runtime checksum manifest: $manifest"
}

$staging = [System.IO.Path]::Combine($repoRoot, ".tmp", "staging-$Suffix")
if (Test-Path -LiteralPath $staging) {
    Remove-Item -LiteralPath $staging -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $staging | Out-Null

foreach ($line in Get-Content -LiteralPath $manifest -Encoding UTF8) {
    $trimmed = $line.Trim()
    if ([string]::IsNullOrWhiteSpace($trimmed) -or $trimmed.StartsWith("#")) {
        continue
    }
    $parts = $trimmed -split "\s+", 2
    if ($parts.Count -ne 2) {
        throw "Invalid checksum line: $line"
    }
    $expected = $parts[0].ToUpperInvariant()
    $relative = $parts[1].Trim().Replace("/", [System.IO.Path]::DirectorySeparatorChar)
    $asset = [System.IO.Path]::Combine($SourceRuntime, $relative)
    if (-not (Test-Path -LiteralPath $asset -PathType Leaf)) {
        throw "Missing runtime asset: $asset"
    }
    $actual = (Get-FileHash -LiteralPath $asset -Algorithm SHA256).Hash.ToUpperInvariant()
    if ($actual -ne $expected) {
        throw "SHA-256 mismatch for $asset`nexpected: $expected`nactual:   $actual"
    }
    $destination = [System.IO.Path]::Combine($staging, $relative)
    $destinationDirectory = Split-Path -Parent $destination
    New-Item -ItemType Directory -Force -Path $destinationDirectory | Out-Null
    Copy-Item -LiteralPath $asset -Destination $destination
}

Copy-Item -LiteralPath $manifest -Destination ([System.IO.Path]::Combine($staging, "SHA256SUMS.$Suffix.txt"))

$archive = [System.IO.Path]::Combine($OutputDir, "runtime-$Suffix.zip")
New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null
if (Test-Path -LiteralPath $archive) {
    if ($Force) {
        Remove-Item -LiteralPath $archive -Force
    }
    else {
        throw "Archive already exists: $archive (use -Force to overwrite)"
    }
}
Compress-Archive -Path (Join-Path $staging "*") -DestinationPath $archive -CompressionLevel Optimal
if (-not (Test-Path -LiteralPath $archive -PathType Leaf)) {
    throw "Compress-Archive did not create: $archive"
}
Get-FileHash -LiteralPath $archive -Algorithm SHA256 | ForEach-Object {
    "$($_.Hash.ToLowerInvariant())  runtime-$Suffix.zip"
} | Set-Content -LiteralPath "$archive.sha256" -Encoding UTF8

Write-Output "Runtime archive created: $archive"
