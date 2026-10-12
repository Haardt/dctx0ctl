param([Parameter(Mandatory)][string]$Binary, [Parameter(Mandatory)][string]$Assets, [Parameter(Mandatory)][string]$Tag, [string]$Installer, [string]$FixtureScope = 'Real Windows executable and piped/pinned installer with deterministic transport fixtures')
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$repositoryRoot = Split-Path (Split-Path $PSScriptRoot)
$installer = if ($Installer) { $Installer } else { Join-Path $repositoryRoot 'install.ps1' }
$root = Join-Path ([IO.Path]::GetTempPath()) ('dctx0-windows-test-' + [Guid]::NewGuid().ToString('N'))
$results = @()
function Assert([bool]$Condition, [string]$Message) { if (-not $Condition) { throw $Message } }
function global:Invoke-WebRequest {
    param([string]$Uri, [switch]$UseBasicParsing, [int]$MaximumRedirection, [int]$TimeoutSec, [string]$OutFile)
    if ($env:DCTX0_TEST_MODE -eq 'download-failure') { throw 'Fixture download failure' }
    if ($Uri.EndsWith('/latest')) {
        $url = if ($env:DCTX0_TEST_MODE -eq 'bad-redirect') { 'https://example.com/release' } else { "https://github.com/Haardt/dctx0ctl/releases/tag/$env:DCTX0_TEST_TAG" }
        return [pscustomobject]@{BaseResponse=[pscustomobject]@{ResponseUri=[Uri]$url;RequestMessage=[pscustomobject]@{RequestUri=[Uri]$url}}}
    }
    $file = Join-Path $env:DCTX0_TEST_ASSETS ($Uri.Split('/')[-1])
    Copy-Item -LiteralPath $file -Destination $OutFile
}
function Run-Case([string]$Name, [scriptblock]$Action) {
    $watch = [Diagnostics.Stopwatch]::StartNew()
    & $Action
    $watch.Stop()
    $script:results += [pscustomobject]@{name=$Name;elapsed_ms=$watch.Elapsed.TotalMilliseconds}
}
function Failure-Preserves([scriptblock]$Action) {
    $file = Join-Path $env:DCTX0_INSTALL_DIR 'dctx0.exe'
    [IO.Directory]::CreateDirectory($env:DCTX0_INSTALL_DIR) | Out-Null
    [IO.File]::WriteAllText($file, 'preserve installation')
    $failed = $false
    try { & $Action } catch { $failed = $true }
    Assert $failed 'Expected installer rejection'
    Assert ([IO.File]::ReadAllText($file) -eq 'preserve installation') 'Existing installation changed'
    Assert (@(Get-ChildItem -LiteralPath $env:DCTX0_INSTALL_DIR -Filter '.dctx0-install-*').Count -eq 0) 'Staged binary leaked'
}
function Write-FixtureArchive([object[]]$Entries) {
    $path = Join-Path $env:DCTX0_TEST_ASSETS $asset
    Remove-Item -LiteralPath $path
    $zip = [IO.Compression.ZipFile]::Open($path, [IO.Compression.ZipArchiveMode]::Create)
    try {
        foreach ($entry in $Entries) {
            $stream = $zip.CreateEntry($entry.Name).Open()
            try { $stream.Write($entry.Bytes, 0, $entry.Bytes.Length) } finally { $stream.Dispose() }
        }
    } finally { $zip.Dispose() }
    [IO.File]::WriteAllText($manifest, (Get-FileHash $path).Hash.ToLowerInvariant() + "  $asset`n")
}
try {
    [IO.Directory]::CreateDirectory($root) | Out-Null
    $env:DCTX0_TEST_ASSETS = [IO.Path]::GetFullPath($Assets)
    $env:DCTX0_TEST_TAG = $Tag
    $env:DCTX0_INSTALL_DIR = Join-Path $root 'bin with spaces'
    $env:DCTX0_TEST_MODE = ''
    Remove-Item Env:DCTX0_REPOSITORY -ErrorAction SilentlyContinue
    $asset = "dctx0-$Tag-x86_64-pc-windows-msvc.zip"
    $manifest = Join-Path $env:DCTX0_TEST_ASSETS 'SHA256SUMS'
    $validManifest = (Get-FileHash (Join-Path $env:DCTX0_TEST_ASSETS $asset) -Algorithm SHA256).Hash.ToLowerInvariant() + "  $asset`n"
    [IO.File]::WriteAllText($manifest, $validManifest)
    Run-Case 'piped latest installation and embedded skills' {
        Invoke-Expression ([IO.File]::ReadAllText($installer))
        $installed = Join-Path $env:DCTX0_INSTALL_DIR 'dctx0.exe'
        Assert ((& $installed --version | Out-String).Trim() -eq ('dctx0 ' + $Tag.Substring(1))) 'Installed version differs'
        Assert ($LASTEXITCODE -eq 0) 'Installed executable failed'
        Assert ((& $installed skills list --json | Out-String).Contains('dctx0-language')) 'Embedded skills missing'
        Assert ((Get-FileHash $installed).Hash -eq (Get-FileHash $Binary).Hash) 'Installed bytes differ'
    }
    Run-Case 'pinned atomic replacement and idempotence' {
        & $installer -Version $Tag -BinDir $env:DCTX0_INSTALL_DIR
        & $installer -Version ($Tag.Substring(1)) -BinDir $env:DCTX0_INSTALL_DIR
    }
    foreach ($mode in @('download-failure','bad-redirect')) {
        $env:DCTX0_TEST_MODE = $mode
        Run-Case $mode { Failure-Preserves { & $installer } }
    }
    $env:DCTX0_TEST_MODE = ''
    foreach ($content in @(('0' * 64) + "  $asset`n",'missing', $validManifest + $validManifest)) {
        [IO.File]::WriteAllText($manifest, $content)
        Run-Case 'bad, missing or duplicate checksum' { Failure-Preserves { & $installer -Version $Tag } }
    }
    [IO.File]::WriteAllText($manifest, $validManifest)
    Run-Case 'invalid version' { Failure-Preserves { & $installer -Version 'v1.2.3;exit' } }
    $originalArchive = [IO.File]::ReadAllBytes((Join-Path $env:DCTX0_TEST_ASSETS $asset))
    $bytes = [IO.File]::ReadAllBytes($Binary)
    Run-Case 'archive paths are not extracted' {
        $outside = Join-Path $root 'escaped.txt'
        Write-FixtureArchive @(@{Name='dctx0.exe';Bytes=$bytes},@{Name=$outside.Replace('\','/');Bytes=[Text.Encoding]::UTF8.GetBytes('escape')})
        & $installer -Version $Tag
        Assert (-not (Test-Path -LiteralPath $outside)) 'Archive wrote outside destination'
    }
    Run-Case 'duplicate executable members preserve installation' {
        Write-FixtureArchive @(@{Name='dctx0.exe';Bytes=$bytes},@{Name='dctx0.exe';Bytes=$bytes})
        Failure-Preserves { & $installer -Version $Tag }
    }
    Run-Case 'missing executable preserves installation' {
        Write-FixtureArchive @(@{Name='other.exe';Bytes=$bytes})
        Failure-Preserves { & $installer -Version $Tag }
    }
    [IO.File]::WriteAllBytes((Join-Path $env:DCTX0_TEST_ASSETS $asset), $originalArchive)
    Run-Case 'executable version mismatch preserves installation' {
        $wrong = 'dctx0-v99.99.99-x86_64-pc-windows-msvc.zip'
        [IO.File]::WriteAllBytes((Join-Path $env:DCTX0_TEST_ASSETS $wrong), $originalArchive)
        [IO.File]::WriteAllText($manifest, (Get-FileHash (Join-Path $env:DCTX0_TEST_ASSETS $wrong)).Hash.ToLowerInvariant() + "  $wrong`n")
        Failure-Preserves { & $installer -Version v99.99.99 }
    }
    [IO.File]::WriteAllText($manifest, $validManifest)
    Run-Case 'directory destination' {
        Remove-Item (Join-Path $env:DCTX0_INSTALL_DIR 'dctx0.exe')
        [IO.Directory]::CreateDirectory((Join-Path $env:DCTX0_INSTALL_DIR 'dctx0.exe')) | Out-Null
        $failed = $false; try { & $installer -Version $Tag } catch { $failed = $true }
        Assert $failed 'Directory destination accepted'
    }
    Run-Case 'help and truncated pipe input' {
        & $installer -Help
        $text = [IO.File]::ReadAllText($installer)
        $failed = $false; try { Invoke-Expression $text.Substring(0, $text.Length - 3) } catch { $failed = $true }
        Assert $failed 'Truncated installer executed'
    }
    [pscustomobject]@{status='PASS';scope=$FixtureScope;checks=$results} | ConvertTo-Json -Depth 8
} finally {
    Remove-Item Function:global:Invoke-WebRequest -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
    foreach ($name in @('DCTX0_TEST_ASSETS','DCTX0_TEST_TAG','DCTX0_TEST_MODE','DCTX0_INSTALL_DIR')) { Remove-Item "Env:$name" -ErrorAction SilentlyContinue }
}
