<#
.SYNOPSIS
    Install tblctl on Windows — downloads a release binary for x86_64.

.DESCRIPTION
    Mirrors the POSIX install.sh: queries the latest GitHub release (or a pinned
    tag via $env:TBLCTL_VERSION), downloads the zip + SHA-256 checksum, verifies
    the hash, extracts tblctl.exe, and places it under
    $env:TBLCTL_INSTALL_DIR (default $HOME\.local\bin). The user PATH is not
    modified; the required entry is printed when missing.

.EXAMPLE
    irm https://raw.githubusercontent.com/driftsys/tblctl/main/install.ps1 | iex
#>

[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$Repo = 'driftsys/tblctl'
$Binary = 'tblctl.exe'
$InstallDir = if ($env:TBLCTL_INSTALL_DIR) { $env:TBLCTL_INSTALL_DIR }
              else { Join-Path $HOME '.local\bin' }

function Get-Target {
    $arch = (Get-CimInstance -ClassName Win32_Processor).Architecture
    # Win32_Processor.Architecture: 0 = x86, 9 = x64, 12 = ARM64
    switch ($arch) {
        9       { return 'x86_64-pc-windows-msvc' }
        default {
            Write-Error "error: unsupported architecture (Win32_Processor.Architecture=$arch). Only x86_64 is supported today."
        }
    }
}

function Get-Version {
    if ($env:TBLCTL_VERSION) { return $env:TBLCTL_VERSION }
    $api = "https://api.github.com/repos/$Repo/releases/latest"
    $release = Invoke-RestMethod -Uri $api -UseBasicParsing
    return $release.tag_name
}

function Test-Checksum {
    param([string]$File, [string]$ChecksumFile)
    # The .sha256 file format is "<hash>  <basename>" (or "<hash> *<basename>").
    $expected = ((Get-Content -LiteralPath $ChecksumFile -Raw).Trim() -split '\s+', 2)[0]
    $actual = (Get-FileHash -LiteralPath $File -Algorithm SHA256).Hash.ToLower()
    if ($expected.ToLower() -ne $actual) {
        Write-Error "checksum mismatch: expected $expected, got $actual"
    }
}

function Main {
    $target = Get-Target
    $version = Get-Version
    $archive = "tblctl-$target.zip"
    $url = "https://github.com/$Repo/releases/download/$version/$archive"
    $checksumUrl = "$url.sha256"

    Write-Host "Installing tblctl $version ($target)"
    Write-Host "  to: $InstallDir"

    $tmpdir = Join-Path ([System.IO.Path]::GetTempPath()) "tblctl-install-$([System.Guid]::NewGuid())"
    New-Item -ItemType Directory -Path $tmpdir | Out-Null
    try {
        $archivePath = Join-Path $tmpdir $archive
        $checksumPath = "$archivePath.sha256"
        Invoke-WebRequest -Uri $url -OutFile $archivePath -UseBasicParsing
        Invoke-WebRequest -Uri $checksumUrl -OutFile $checksumPath -UseBasicParsing

        Write-Host 'Verifying checksum...'
        Test-Checksum -File $archivePath -ChecksumFile $checksumPath

        $extract = Join-Path $tmpdir 'x'
        Expand-Archive -LiteralPath $archivePath -DestinationPath $extract
        $bin = Get-ChildItem -Path $extract -Recurse -Filter $Binary | Select-Object -First 1
        if (-not $bin) { Write-Error "$Binary not found in $archive" }

        New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
        $dest = Join-Path $InstallDir $Binary
        Move-Item -Force -Path $bin.FullName -Destination $dest

        Write-Host "Installed $dest"

        $pathEntries = $env:PATH -split ';' | Where-Object { $_ -ne '' }
        $onPath = $pathEntries | ForEach-Object {
            try { (Resolve-Path -LiteralPath $_ -ErrorAction Stop).Path } catch { $_ }
        } | Where-Object { $_ -ieq (Resolve-Path -LiteralPath $InstallDir).Path }

        if (-not $onPath) {
            Write-Host ''
            Write-Host 'Add to your PATH (current session):'
            Write-Host "  `$env:PATH = `"$InstallDir;`$env:PATH`""
            Write-Host ''
            Write-Host 'Add to your PATH (persistent, user scope):'
            Write-Host "  [Environment]::SetEnvironmentVariable('Path', `"$InstallDir;`" + [Environment]::GetEnvironmentVariable('Path', 'User'), 'User')"
        }
    }
    finally {
        Remove-Item -Recurse -Force -Path $tmpdir -ErrorAction SilentlyContinue
    }
}

Main
