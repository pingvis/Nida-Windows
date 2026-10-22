[CmdletBinding()]
param(
    [string]$CefRoot = $env:NIDA_CEF_ROOT,
    [Parameter(Mandatory)][string]$OutputDirectory
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if (!$IsWindows -or !$CefRoot) { throw 'Use Windows x64 and run setup-windows-cef.ps1 first.' }
$root = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$CefRoot = (Resolve-Path -LiteralPath $CefRoot).Path
$inputs = Get-Content -LiteralPath (Join-Path $CefRoot 'nida-cef-inputs.json') -Raw | ConvertFrom-Json
if ($inputs.version -ne '152.0.15+gb3d72fb+chromium-152.0.7977.169') { throw 'CEF SDK does not match the supported API/runtime pair.' }
$build = Join-Path $root 'target/cef-host'
& cmake -S (Join-Path $root 'browser/cef') -B $build -A x64 "-DCEF_ROOT=$CefRoot"
if ($LASTEXITCODE -ne 0) { throw 'CEF helper CMake configuration failed.' }
& cmake --build $build --config Release --parallel 4
if ($LASTEXITCODE -ne 0) { throw 'CEF helper build failed.' }
& cmake --install $build --config Release --prefix $OutputDirectory
if ($LASTEXITCODE -ne 0) { throw 'CEF helper installation failed.' }
$browser = Join-Path $OutputDirectory 'browser'
Copy-Item -LiteralPath (Join-Path $CefRoot 'nida-cef-inputs.json') -Destination $browser
# ZIP files do not preserve Windows ACLs. The app repeats this narrow runtime
# grant at first browser use, including for extracted portable packages.
& icacls $browser /grant '*S-1-15-2-2:(OI)(CI)(RX)' /T /Q | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'CEF sandbox runtime ACL setup failed.' }

# Extract notices from the exact Chromium runtime being packaged, rather than a
# generic web link or a different version's credits.
& python (Join-Path $PSScriptRoot 'test-browser-host.py') (Join-Path $browser 'nida-browser-host.exe') --credits-output (Join-Path $browser 'Chromium-credits.html')
if ($LASTEXITCODE -ne 0) { throw 'Could not export exact Chromium third-party notices.' }
