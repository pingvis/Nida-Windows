# Exact SDK/runtime pair for the experimental browser source. No floating latest.
[CmdletBinding()]
param([Parameter(Mandatory)][string]$ToolsDirectory)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
if (!$IsWindows) { throw 'CEF setup requires Windows x64.' }

# CI does not render PowerShell's interactive progress bar. Run slow stages in
# a worker so the main thread can emit ordinary log lines while they work.
function Invoke-CefSetupStage {
    param(
        [string]$Label,
        [scriptblock]$Action,
        [object[]]$Arguments = @(),
        [scriptblock]$ProgressDetail = { '' },
        [int]$HeartbeatSeconds = 30
    )
    $timer = [Diagnostics.Stopwatch]::StartNew()
    Write-Host "[CEF] Starting: $Label"
    $worker = Start-ThreadJob -ScriptBlock $Action -ArgumentList $Arguments
    try {
        while ($true) {
            $finished = Wait-Job -Job $worker -Timeout $HeartbeatSeconds
            if ($finished) { break }
            $detail = & $ProgressDetail
            Write-Host ("[CEF] {0}: still working ({1:n0}s elapsed) {2}" -f $Label, $timer.Elapsed.TotalSeconds, $detail)
        }
        Receive-Job -Job $worker -ErrorAction Stop
        if ($worker.State -ne 'Completed') { throw "CEF stage failed: $Label" }
        Write-Host ("[CEF] Completed: {0} ({1:n1}s)" -f $Label, $timer.Elapsed.TotalSeconds)
    } finally {
        if ($worker.State -in @('Running', 'NotStarted')) { Stop-Job -Job $worker }
        Remove-Job -Job $worker -Force
    }
}

$version = '152.0.15+gb3d72fb+chromium-152.0.7977.169'
$name = "cef_binary_${version}_windows64"
$url = "https://cef-builds.spotifycdn.com/$name.tar.bz2"
$sha256 = 'c0640c1df5ae2d84705d06f7398798a907745504f730fa7b2abb98ccb878d162'
New-Item -ItemType Directory -Force -Path $ToolsDirectory | Out-Null
$ToolsDirectory = (Resolve-Path -LiteralPath $ToolsDirectory).Path
$archive = Join-Path $ToolsDirectory "$name.tar.bz2"
$cef = Join-Path $ToolsDirectory $name
Write-Host "[CEF] SDK version: $version"
if (!(Test-Path -LiteralPath $archive)) {
    Write-Host "[CEF] Download URL: $url"
    Invoke-CefSetupStage -Label 'Download SDK (10 minute timeout)' -Arguments @($url, $archive) -Action {
        param($DownloadUrl, $Destination)
        $ErrorActionPreference = 'Stop'
        $ProgressPreference = 'SilentlyContinue'
        Invoke-WebRequest -Uri $DownloadUrl -OutFile $Destination -TimeoutSec 600
    } -ProgressDetail {
        if (Test-Path -LiteralPath $archive) {
            '{0:n1} MiB downloaded' -f ((Get-Item -LiteralPath $archive).Length / 1MB)
        }
    }
} else {
    Write-Host "[CEF] Reusing downloaded archive: $archive"
}
Write-Host ('[CEF] Archive size: {0:n1} MiB' -f ((Get-Item -LiteralPath $archive).Length / 1MB))
Invoke-CefSetupStage -Label 'Verify SDK SHA256' -Arguments @($archive, $sha256) -Action {
    param($ArchivePath, $ExpectedHash)
    $ErrorActionPreference = 'Stop'
    if ((Get-FileHash -LiteralPath $ArchivePath -Algorithm SHA256).Hash.ToLowerInvariant() -ne $ExpectedHash) {
        throw 'CEF SDK checksum mismatch.'
    }
}
# Always validate the archive even when reusing an extracted SDK.
if (!(Test-Path -LiteralPath $cef)) {
    Write-Host "[CEF] Extracting SDK to: $cef"
    Invoke-CefSetupStage -Label 'Extract SDK with tar' -Arguments @($archive, $ToolsDirectory) -Action {
        param($ArchivePath, $Destination)
        $ErrorActionPreference = 'Stop'
        & tar -xjf $ArchivePath -C $Destination
        if ($LASTEXITCODE -ne 0) { throw "Could not extract CEF SDK (tar exit code $LASTEXITCODE)." }
    }
} else {
    Write-Host "[CEF] Reusing extracted SDK: $cef"
}
Write-Host '[CEF] Checking required SDK/runtime files'
foreach ($required in @('Release/libcef.dll', 'Release/bootstrap.exe', 'Resources/icudtl.dat', 'include/cef_version.h')) {
    if (!(Test-Path -LiteralPath (Join-Path $cef $required))) { throw "Incomplete CEF SDK: $required" }
}
@{ version = $version; url = $url; sha256 = $sha256 } | ConvertTo-Json |
    Set-Content -LiteralPath (Join-Path $cef 'nida-cef-inputs.json') -Encoding utf8
$env:NIDA_CEF_ROOT = $cef
if ($env:GITHUB_ENV) { "NIDA_CEF_ROOT=$cef" >> $env:GITHUB_ENV }
Write-Host "CEF SDK ready: $version"
