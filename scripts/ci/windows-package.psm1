Set-StrictMode -Version Latest

function Resolve-WindowsReleaseVersion {
    param([string]$RequestedVersion, [Parameter(Mandatory)][string]$PackageVersion, [string]$TagVersion)
    $pattern = '^\d+\.\d+\.\d+(-[a-z0-9-]+(\.[a-z0-9-]+)*)?$'
    $requested = ([string]$RequestedVersion).Trim().ToLowerInvariant() -replace '^v', ''
    $tag = ([string]$TagVersion).Trim().ToLowerInvariant() -replace '^v', ''
    if ($requested -and $tag -and $requested -cne $tag) { throw 'Release version must match the selected tag.' }
    $version = if ($tag) { $tag } elseif ($requested) { $requested } else { $PackageVersion.ToLowerInvariant() }
    if ($version -cnotmatch $pattern) { throw 'Use a release version such as 0.9.1-beta.4 (an optional leading v is accepted).' }
    if (($version -split '-')[0] -ne ($PackageVersion -split '-')[0]) { throw 'The numeric release version must match the package version.' }
    $version
}

function Write-PackageChecksum {
    param([Parameter(Mandatory)][string]$Path)
    $name = [IO.Path]::GetFileName($Path)
    $hash = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $name" | Set-Content -LiteralPath "$Path.sha256" -Encoding ascii
}

function ConvertFrom-DumpbinDependencies {
    param([string[]]$Lines)
    @($Lines | ForEach-Object {
        # /DEPENDENTS includes both ordinary and delay-loaded dependencies.
        if ($_ -match '^\s+([A-Za-z0-9_.+-]+\.dll)\s*$') { $Matches[1].ToLowerInvariant() }
    } | Sort-Object -Unique)
}

function Get-PeDependencies {
    param([string]$Path)
    $headers = @(& dumpbin.exe /nologo /headers $Path)
    if ($LASTEXITCODE -ne 0 -or !($headers -match '8664 machine')) {
        throw "Not a readable x64 Windows binary: $Path"
    }
    $lines = @(& dumpbin.exe /nologo /dependents $Path)
    if ($LASTEXITCODE -ne 0) { throw "Cannot inspect dependencies: $Path" }
    ConvertFrom-DumpbinDependencies $lines
}

function Find-PackageDependency {
    param([string]$Name, [string]$FfmpegBin, [string[]]$SearchDirectories, [string]$SystemDirectory)
    # Never mix FFmpeg DLLs from another SDK with this SDK's tools and headers.
    $ffmpegImport = $Name -match '^(avcodec|avdevice|avfilter|avformat|avutil|swscale|swresample|postproc)-\d+\.dll$'
    $directories = if ($ffmpegImport) {
        @($FfmpegBin)
    } else { @($FfmpegBin) + $SearchDirectories }
    foreach ($directory in $directories) {
        if (!$directory) { continue }
        $candidate = Join-Path $directory $Name
        if (Test-Path -LiteralPath $candidate -PathType Leaf) { return (Get-Item -LiteralPath $candidate).FullName }
    }
    if ($ffmpegImport) { throw "Missing runtime dependency $Name in the selected FFmpeg SDK." }
    # API-set contracts are implemented by Windows. Visual C++ runtime DLLs
    # must be included app-local, even when installed on the build machine.
    if ($Name -match '^(api-ms-win-|ext-ms-win-)') { return $null }
    if ($Name -notmatch '^(vcruntime|msvcp1|concrt|vcomp|msvcr1)' -and
        (Test-Path -LiteralPath (Join-Path $SystemDirectory $Name) -PathType Leaf)) { return $null }
    throw "Missing runtime dependency $Name. Supply its matching DLL (or the MSVC x64 CRT directory)."
}

function Copy-PackageDependencies {
    param([string]$Stage, [string]$FfmpegBin, [string[]]$SearchDirectories)
    $pending = [Collections.Generic.Queue[string]]::new()
    Get-ChildItem -LiteralPath $Stage -File | Where-Object { $_.Extension -in '.exe', '.dll' } |
        ForEach-Object { $pending.Enqueue($_.FullName) }
    $visited = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    while ($pending.Count) {
        $path = $pending.Dequeue()
        if (!$visited.Add([IO.Path]::GetFileName($path))) { continue }
        foreach ($name in @(Get-PeDependencies $path)) {
            $destination = Join-Path $Stage $name
            if (Test-Path -LiteralPath $destination) { continue }
            $source = Find-PackageDependency $name $FfmpegBin $SearchDirectories (Join-Path $env:SystemRoot 'System32')
            if ($source) {
                Copy-Item -LiteralPath $source -Destination $destination
                $pending.Enqueue($destination)
            }
        }
    }
}

Export-ModuleMember -Function Resolve-WindowsReleaseVersion, Write-PackageChecksum, ConvertFrom-DumpbinDependencies, Get-PeDependencies, Find-PackageDependency, Copy-PackageDependencies
