# Piped execution is enclosed in one parsed block: truncated input cannot start installation.
[CmdletBinding()]
param(
        [string]$Version = 'latest',
        [string]$BinDir = $(if ($env:DCTX0_INSTALL_DIR) { $env:DCTX0_INSTALL_DIR } elseif ($env:LOCALAPPDATA) { Join-Path $env:LOCALAPPDATA 'DCTX0\bin' } else { Join-Path $HOME '.local/bin' }),
        [switch]$Help
    )
& {
    Set-StrictMode -Version Latest
    $ErrorActionPreference = 'Stop'
    if ($Help) {
        Write-Output 'Install DCTX0 on Windows x64. Parameters: -Version v0.1.0 (default: latest stable), -BinDir DIRECTORY, -Help.'
        return
    }
    if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) { throw 'This installer requires Windows x64.' }
    $architecture = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
    if ($architecture -ne 'AMD64') { throw "Unsupported Windows architecture: $architecture" }
    $repository = if ($env:DCTX0_REPOSITORY) { $env:DCTX0_REPOSITORY } else { 'Haardt/dctx0ctl' }
    if ($repository -notmatch '^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$') { throw 'Invalid DCTX0_REPOSITORY.' }
    $tagPattern = '^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$'
    if ($Version -ne 'latest') {
        if (-not $Version.StartsWith('v')) { $Version = 'v' + $Version }
        if ($Version -notmatch $tagPattern -or $Version.Contains("`n") -or $Version.Contains("`r")) { throw 'Invalid release version.' }
    }
    $base = "https://github.com/$repository/releases"
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    $temporary = $null
    $staged = $null
    function Assert-Destination([string]$Path) {
        if (Test-Path -LiteralPath $Path) {
            $item = Get-Item -LiteralPath $Path -Force
            if ($item.PSIsContainer -or (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0)) {
                throw 'Destination dctx0.exe must be a regular file.'
            }
        }
    }
    try {
        $BinDir = [IO.Path]::GetFullPath($BinDir)
        $destination = Join-Path $BinDir 'dctx0.exe'
        Assert-Destination $destination
        if ($Version -eq 'latest') {
            $response = Invoke-WebRequest -Uri "$base/latest" -UseBasicParsing -MaximumRedirection 5 -TimeoutSec 180
            $resolved = if ($PSVersionTable.PSVersion.Major -ge 6) { $response.BaseResponse.RequestMessage.RequestUri.AbsoluteUri } else { $response.BaseResponse.ResponseUri.AbsoluteUri }
            if (-not $resolved.StartsWith("$base/tag/", [StringComparison]::Ordinal)) { throw 'Unexpected latest-release redirect.' }
            $Version = [Uri]::UnescapeDataString($resolved.Substring("$base/tag/".Length))
            if ($Version -notmatch $tagPattern -or $Version.Contains("`n") -or $Version.Contains("`r")) { throw 'Invalid latest-release version.' }
        }
        $asset = "dctx0-$Version-x86_64-pc-windows-msvc.zip"
        $temporary = Join-Path ([IO.Path]::GetTempPath()) ('dctx0-download-' + [Guid]::NewGuid().ToString('N'))
        [IO.Directory]::CreateDirectory($temporary) | Out-Null
        $manifest = Join-Path $temporary 'SHA256SUMS'
        $archivePath = Join-Path $temporary $asset
        Invoke-WebRequest -Uri "$base/download/$Version/SHA256SUMS" -UseBasicParsing -OutFile $manifest -TimeoutSec 180
        $checksumEntries = @(Get-Content -LiteralPath $manifest | ForEach-Object {
            $parts = $_.Trim() -split '\s+'
            if ($parts.Length -eq 2 -and $parts[1] -ceq $asset) { $parts[0] }
        })
        if ($checksumEntries.Count -ne 1 -or $checksumEntries[0] -cnotmatch '^[0-9a-f]{64}$') { throw "No unique valid checksum for $asset." }
        Invoke-WebRequest -Uri "$base/download/$Version/$asset" -UseBasicParsing -OutFile $archivePath -TimeoutSec 180
        if ((Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLowerInvariant() -cne $checksumEntries[0]) { throw 'Checksum mismatch; installation unchanged.' }
        [IO.Directory]::CreateDirectory($BinDir) | Out-Null
        $staged = Join-Path $BinDir ('.dctx0-install-' + [Guid]::NewGuid().ToString('N') + '.exe')
        Add-Type -AssemblyName System.IO.Compression.FileSystem
        $archive = [IO.Compression.ZipFile]::OpenRead($archivePath)
        try {
            $entries = @($archive.Entries | Where-Object { $_.FullName -ceq 'dctx0.exe' })
            if ($entries.Count -ne 1) { throw 'Archive has no unique dctx0.exe member.' }
            $source = $entries[0].Open()
            try {
                $output = [IO.File]::Open($staged, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write)
                try { $source.CopyTo($output) } finally { $output.Dispose() }
            } finally { $source.Dispose() }
        } finally { $archive.Dispose() }
        $installedVersion = (& $staged --version | Out-String).Trim()
        if ($LASTEXITCODE -ne 0 -or $installedVersion -cne ('dctx0 ' + $Version.Substring(1))) { throw 'Downloaded executable version does not match the release.' }
        Assert-Destination $destination
        if ([IO.File]::Exists($destination)) {
            [IO.File]::Replace($staged, $destination, [NullString]::Value)
        } else {
            [IO.File]::Move($staged, $destination)
        }
        $staged = $null
        Write-Output "Installed $installedVersion to $destination"
        Write-Output "Add $BinDir to your user PATH."
    } finally {
        if ($staged -and (Test-Path -LiteralPath $staged)) { Remove-Item -LiteralPath $staged -Force }
        if ($temporary -and (Test-Path -LiteralPath $temporary)) { Remove-Item -LiteralPath $temporary -Recurse -Force }
    }
}
