# Run from Windows x64 Developer PowerShell 7. Leaves the GUI open.
[CmdletBinding()]
param(
    [string]$Url,
    [switch]$SkipBuild,
    [ValidateRange(1024, 65535)][int]$Port = 8088
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if (!$IsWindows) { throw 'Run this launcher on Windows using x64 Developer PowerShell 7.' }
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$output = Join-Path $root 'target/debug'
$exe = Join-Path $output 'varda.exe'
$workspace = Join-Path $root "target/browser-debug-workspace/$([guid]::NewGuid().ToString('N'))"

# Never connect to an already running application's API and change its show.
$listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Any, $Port)
try { $listener.Start() } finally { $listener.Stop() }

Push-Location $root
try {
    if (!$env:FFMPEG_DIR -or !$env:LIBCLANG_PATH) {
        if ($SkipBuild) { throw 'Set FFMPEG_DIR and LIBCLANG_PATH, or run without -SkipBuild.' }
        & "$PSScriptRoot/ci/setup-windows-build.ps1" -ToolsDirectory (Join-Path $root "target/debug-tools/$([guid]::NewGuid().ToString('N'))")
    }
    if (!$SkipBuild) {
        Get-Command cargo, cmake, python, cl.exe -ErrorAction Stop | Out-Null
        if (!$env:NIDA_CEF_ROOT) {
            & "$PSScriptRoot/ci/setup-windows-cef.ps1" -ToolsDirectory (Join-Path $root 'target/cef-sdk')
        }
        & cargo build --locked --no-default-features --features screen-capture,windows-browser
        if ($LASTEXITCODE -ne 0) { throw 'Nida debug build failed.' }
        & "$PSScriptRoot/ci/build-windows-browser.ps1" -OutputDirectory $output
    }
    foreach ($file in @($exe, (Join-Path $output 'browser/nida-browser-host.exe'))) {
        if (!(Test-Path -LiteralPath $file -PathType Leaf)) { throw "Missing debug binary: $file" }
    }
    New-Item -ItemType Directory -Force -Path $workspace | Out-Null
    if (!$Url) {
        $demo = @'
<!doctype html><meta charset="utf-8">
<style>html,body{margin:0;background:transparent;color:white;font:32px sans-serif}
main{margin:48px;padding:28px;background:#15213bd9;border-radius:20px;width:650px}
h1{font-size:34px;color:#7ee7ff}p{animation:enter .4s ease-out}@keyframes enter{from{transform:translateY(20px);opacity:0}}</style>
<main><h1>Browser source debug</h1><section id="chat"></section></main>
<script>let n=0;function tick(){let p=document.createElement('p');p.textContent='Viewer '+(++n)+': Live chat at '+new Date().toLocaleTimeString();chat.append(p);while(chat.children.length>5)chat.firstChild.remove()}tick();setInterval(tick,2000)</script>
'@
        $Url = 'data:text/html;charset=utf-8,' + [Uri]::EscapeDataString($demo)
    }
    $start = [Diagnostics.ProcessStartInfo]::new($exe)
    $start.UseShellExecute = $false
    $start.WorkingDirectory = $root
    $start.Environment['PATH'] = "$(Join-Path $env:FFMPEG_DIR 'bin');$env:PATH"
    foreach ($arg in @('--workspace', $workspace, '--port', "$Port", '--no-camera', '--no-screen-capture', '--no-ndi', '--no-osc')) {
        $start.ArgumentList.Add($arg)
    }
    $process = [Diagnostics.Process]::Start($start)
    $api = "http://127.0.0.1:$Port/api"
    $ready = $false
    $deadline = [DateTime]::UtcNow.AddSeconds(90)
    while ([DateTime]::UtcNow -lt $deadline) {
        if ($process.HasExited) { throw "Nida exited during startup (code $($process.ExitCode))." }
        try {
            $channels = Invoke-RestMethod "$api/scene/channels" -TimeoutSec 2
            $ready = $true
            break
        } catch { Start-Sleep -Milliseconds 250 }
    }
    if (!$ready) { throw 'Nida is open, but its API did not become ready within 90 seconds.' }
    if (@($channels).Count) { $channel = @($channels)[0].uuid }
    else { $channel = (Invoke-RestMethod "$api/channels" -Method Post -TimeoutSec 10).uuid }
    $body = @{ url = $Url; width = 1280; height = 720; fps = 30 } | ConvertTo-Json
    $null = Invoke-RestMethod "$api/channels/$channel/decks/html" -Method Post -ContentType 'application/json' -Body $body -TimeoutSec 15
    Write-Host "Nida debug is open (PID $($process.Id)). Double-click Inline HTML in the first channel, then select Source for URL, size, transparency and Reload controls."
    Write-Host "Debug show workspace: $workspace"
} finally { Pop-Location }
