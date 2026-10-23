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
$requiredFiles = @(
    'Release/libcef.dll', 'Release/libcef.lib', 'Release/bootstrap.exe',
    'Resources/icudtl.dat', 'include/cef_version.h',
    'cmake/FindCEF.cmake', 'libcef_dll/CMakeLists.txt'
)
# Check the restored SDK before touching the archive: an SDK cache deliberately
# excludes the redundant .tar.bz2 download and incomplete staging directories.
$marker = Join-Path $cef 'nida-cef-inputs.json'
$ready = $false
if (Test-Path -LiteralPath $marker) {
    try {
        $inputs = Get-Content -LiteralPath $marker -Raw | ConvertFrom-Json
        $ready = $inputs.version -eq $version -and $inputs.sha256 -eq $sha256
        foreach ($required in $requiredFiles) {
            if (!(Test-Path -LiteralPath (Join-Path $cef $required) -PathType Leaf)) { $ready = $false }
        }
    } catch {
        $ready = $false
        Write-Host '[CEF] Invalid SDK completion marker; rebuilding the SDK.'
    }
}
if ($ready) {
    Write-Host '[CEF] Validated cached SDK; skipping download, archive hashing and extraction.'
} else {
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
}
if (!$ready) {
    Write-Host "[CEF] Extracting SDK to: $cef"
    $staging = Join-Path $ToolsDirectory "$name.extracting"
    # These are fixed children of the resolved tools directory, never arbitrary
    # caller-supplied deletion targets. Retry partial extraction in a fresh tree.
    foreach ($target in @($staging, $cef)) {
        if ([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($target)) -ne $ToolsDirectory.TrimEnd('\', '/')) {
            throw "CEF extraction path is outside the tools directory: $target"
        }
        if (Test-Path -LiteralPath $target) { Remove-Item -LiteralPath $target -Recurse -Force }
    }
    New-Item -ItemType Directory -Path $staging | Out-Null
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = (Get-Command python -CommandType Application -ErrorAction Stop | Select-Object -First 1).Source
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardInput = $true
    foreach ($argument in @('-u', (Join-Path $PSScriptRoot 'extract-windows-cef.py'), $archive, $staging, $name)) {
        $start.ArgumentList.Add($argument)
    }
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $start
    $processStarted = $false
    $timer = [Diagnostics.Stopwatch]::StartNew()
    Write-Host "[CEF] Extractor: $($start.FileName); hard timeout: 600s"
    try {
        if (!$process.Start()) { throw 'Could not start CEF extractor.' }
        $processStarted = $true
        $process.StandardInput.Close()
        while (!$process.WaitForExit(30000)) {
            Write-Host ("[CEF] Extraction process running ({0:n0}s elapsed, {1:n1}s CPU)" -f $timer.Elapsed.TotalSeconds, $process.TotalProcessorTime.TotalSeconds)
            if ($timer.Elapsed.TotalSeconds -ge 600) { throw 'CEF extraction exceeded 600 seconds; aborting instead of hanging the build.' }
        }
        if ($process.ExitCode -ne 0) { throw "CEF extraction failed (Python exit code $($process.ExitCode))." }
    } finally {
        if ($processStarted -and !$process.HasExited) {
            $process.Kill($true)
            $process.WaitForExit()
        }
        $process.Dispose()
    }
    Move-Item -LiteralPath (Join-Path $staging $name) -Destination $cef
    Remove-Item -LiteralPath $staging
} else {
    Write-Host "[CEF] Reusing extracted SDK: $cef"
}
Write-Host '[CEF] Checking required SDK/runtime files'
foreach ($required in $requiredFiles) {
    if (!(Test-Path -LiteralPath (Join-Path $cef $required))) { throw "Incomplete CEF SDK: $required" }
}
@{ version = $version; url = $url; sha256 = $sha256 } | ConvertTo-Json |
    Set-Content -LiteralPath (Join-Path $cef 'nida-cef-inputs.json') -Encoding utf8
$env:NIDA_CEF_ROOT = $cef
if ($env:GITHUB_ENV) { "NIDA_CEF_ROOT=$cef" >> $env:GITHUB_ENV }
Write-Host "CEF SDK ready: $version"
