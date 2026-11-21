Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

function Resolve-Target {
    $arch = $null
    $runtimeInfo = 'System.Runtime.InteropServices.RuntimeInformation' -as [type]
    if ($null -ne $runtimeInfo) {
        $arch = $runtimeInfo::OSArchitecture.ToString()
    }

    if ([string]::IsNullOrWhiteSpace($arch)) {
        if (-not [string]::IsNullOrWhiteSpace($env:PROCESSOR_ARCHITEW6432)) {
            $arch = $env:PROCESSOR_ARCHITEW6432
        } else {
            $arch = $env:PROCESSOR_ARCHITECTURE
        }
    }

    if ([string]::IsNullOrWhiteSpace($arch)) {
        throw 'Unable to determine Windows architecture.'
    }

    switch ($arch.ToUpperInvariant()) {
        'X64' { return 'x86_64-pc-windows-msvc' }
        'AMD64' { return 'x86_64-pc-windows-msvc' }
        'ARM64' { return 'aarch64-pc-windows-msvc' }
        default { throw "Unsupported Windows architecture: $arch" }
    }
}

function Resolve-InstallDir {
    if (-not [string]::IsNullOrWhiteSpace($env:CODEINT_INSTALL_DIR)) {
        return $env:CODEINT_INSTALL_DIR
    }
    return Join-Path (Join-Path $HOME '.local') 'bin'
}

function Resolve-ReleaseSource {
    if (-not [string]::IsNullOrWhiteSpace($env:CODEINT_ARTIFACTS_DIR)) {
        return @{
            Mode = 'Local'
            Base = $env:CODEINT_ARTIFACTS_DIR
        }
    }

    if (-not [string]::IsNullOrWhiteSpace($env:CODEINT_RELEASE_BASE_URL)) {
        $rawBase = $env:CODEINT_RELEASE_BASE_URL
        if ($rawBase -match '^https?://') {
            return @{
                Mode = 'Http'
                Base = $rawBase.TrimEnd('/')
            }
        }
        if ($rawBase -match '^file://') {
            $uri = [System.Uri]::new($rawBase)
            return @{
                Mode = 'Local'
                Base = $uri.LocalPath
            }
        }
        return @{
            Mode = 'Local'
            Base = $rawBase
        }
    }

    if (-not [string]::IsNullOrWhiteSpace($env:CODEINT_VERSION)) {
        return @{
            Mode = 'Http'
            Base = "https://github.com/avidianity/codeint/releases/download/$($env:CODEINT_VERSION)"
        }
    }

    return @{
        Mode = 'Http'
        Base = 'https://github.com/avidianity/codeint/releases/latest/download'
    }
}

function Copy-OrDownloadArtifact {
    param(
        [Parameter(Mandatory = $true)][hashtable]$Source,
        [Parameter(Mandatory = $true)][string]$FileName,
        [Parameter(Mandatory = $true)][string]$DestinationPath
    )

    if ($Source.Mode -eq 'Local') {
        $localPath = Join-Path $Source.Base $FileName
        if (-not (Test-Path -LiteralPath $localPath -PathType Leaf)) {
            throw "Release artifact not found: $localPath"
        }
        Copy-Item -LiteralPath $localPath -Destination $DestinationPath -Force
        return
    }

    if ($PSVersionTable.PSVersion.Major -lt 6) {
        [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor [System.Net.SecurityProtocolType]::Tls12
    }

    $url = "$($Source.Base)/$FileName"
    Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $DestinationPath
}

function Find-ExpectedHash {
    param(
        [Parameter(Mandatory = $true)][string]$ChecksumsPath,
        [Parameter(Mandatory = $true)][string]$ArchiveName
    )

    $lines = @(Get-Content -LiteralPath $ChecksumsPath)
    foreach ($line in $lines) {
        if ($line -match '^\s*([0-9a-fA-F]{64})\s+\*?(.+?)\s*$') {
            $entryHash = $Matches[1].ToLowerInvariant()
            $entryName = $Matches[2]
            if ($entryName -eq $ArchiveName) {
                return $entryHash
            }
        }
    }

    throw "SHA256SUMS does not contain an entry for $ArchiveName"
}

$target = Resolve-Target
$archiveName = "codeint-$target.zip"
$installDir = Resolve-InstallDir
$source = Resolve-ReleaseSource

$tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ('codeint-install-' + [System.Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $tempRoot -Force | Out-Null

try {
    $checksumsPath = Join-Path $tempRoot 'SHA256SUMS'
    $archivePath = Join-Path $tempRoot $archiveName
    $extractDir = Join-Path $tempRoot 'extract'

    Copy-OrDownloadArtifact -Source $source -FileName 'SHA256SUMS' -DestinationPath $checksumsPath
    Copy-OrDownloadArtifact -Source $source -FileName $archiveName -DestinationPath $archivePath

    $expectedHash = Find-ExpectedHash -ChecksumsPath $checksumsPath -ArchiveName $archiveName
    $actualHash = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLowerInvariant()

    if ($actualHash -ne $expectedHash) {
        throw "SHA-256 checksum mismatch for ${archiveName}: expected $expectedHash, got $actualHash"
    }

    Expand-Archive -LiteralPath $archivePath -DestinationPath $extractDir -Force

    $extractedBinary = Join-Path $extractDir 'codeint.exe'
    if (-not (Test-Path -LiteralPath $extractedBinary -PathType Leaf)) {
        throw "Archive $archiveName does not contain codeint.exe"
    }

    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
    $installedBinary = Join-Path $installDir 'codeint.exe'
    Copy-Item -LiteralPath $extractedBinary -Destination $installedBinary -Force

    Write-Output "Installed codeint to $installedBinary"
} finally {
    if (Test-Path -LiteralPath $tempRoot) {
        Remove-Item -LiteralPath $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}
