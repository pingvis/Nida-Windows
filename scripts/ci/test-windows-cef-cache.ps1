# Exercise cache hits without the download archive, and invalid-cache fallback.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$setup = Join-Path $PSScriptRoot 'setup-windows-cef.ps1'
$tokens = $null
$errors = $null
$ast = [Management.Automation.Language.Parser]::ParseFile($setup, [ref]$tokens, [ref]$errors)
if ($errors.Count) { throw ($errors | Out-String) }
$pins = @{}
foreach ($assignment in $ast.FindAll({ param($node) $node -is [Management.Automation.Language.AssignmentStatementAst] }, $true)) {
    if ($assignment.Left -is [Management.Automation.Language.VariableExpressionAst] -and $assignment.Left.VariablePath.UserPath -in @('version', 'sha256', 'requiredFiles')) {
        $pins[$assignment.Left.VariablePath.UserPath] = & ([scriptblock]::Create($assignment.Right.Extent.Text))
    }
}
$temporary = Join-Path ([IO.Path]::GetTempPath()) ('nida-cef-cache-' + [guid]::NewGuid().ToString('N'))
$name = "cef_binary_$($pins.version)_windows64"
$sdk = Join-Path $temporary $name
$previousRoot = $env:NIDA_CEF_ROOT
$previousGithubEnv = $env:GITHUB_ENV
try {
    $env:GITHUB_ENV = Join-Path $temporary 'github-env.txt'
    foreach ($file in $pins.requiredFiles) {
        $path = Join-Path $sdk $file
        New-Item -ItemType Directory -Force -Path (Split-Path $path) | Out-Null
        Set-Content -LiteralPath $path -Value 'fixture'
    }
    $marker = Join-Path $sdk 'nida-cef-inputs.json'
    @{ version = $pins.version; sha256 = $pins.sha256 } | ConvertTo-Json | Set-Content -LiteralPath $marker
    & $setup -ToolsDirectory $temporary
    if ($env:NIDA_CEF_ROOT -ne $sdk) { throw 'Cached SDK root was not exported.' }
    if ((Get-Content -LiteralPath $env:GITHUB_ENV) -ne "NIDA_CEF_ROOT=$sdk") { throw 'Cached SDK root was not exported to subsequent workflow steps.' }
    $archive = Join-Path $temporary "$name.tar.bz2"
    if (Test-Path -LiteralPath $archive) { throw 'Cache hit unexpectedly downloaded the archive.' }

    # A deliberately bad archive makes fallback fail at its checksum, without
    # a network call or native extraction. Returning success would mean the
    # incomplete/mismatched cached SDK had incorrectly been accepted.
    Set-Content -LiteralPath $archive -Value 'invalid archive fixture'
    foreach ($scenario in @('missing file', 'wrong pin', 'invalid marker')) {
        switch ($scenario) {
            'missing file' { Remove-Item -LiteralPath (Join-Path $sdk 'Release/libcef.lib') }
            'wrong pin' {
                Set-Content -LiteralPath (Join-Path $sdk 'Release/libcef.lib') -Value 'fixture'
                @{ version = $pins.version; sha256 = 'wrong' } | ConvertTo-Json | Set-Content -LiteralPath $marker
            }
            'invalid marker' { Set-Content -LiteralPath $marker -Value '{ invalid JSON' }
        }
        $caught = $false
        try { & $setup -ToolsDirectory $temporary } catch { $caught = $_.Exception.Message -like '*CEF SDK checksum mismatch*' }
        if (!$caught) { throw "Invalid cache was not rebuilt: $scenario" }
    }
    Write-Host 'PASS: archive-free SDK cache hit, workflow environment export, incomplete SDK, wrong pin and malformed marker fallback.'
} finally {
    $env:NIDA_CEF_ROOT = $previousRoot
    $env:GITHUB_ENV = $previousGithubEnv
    if ([IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($temporary)) -ne [IO.Path]::GetTempPath().TrimEnd('\', '/')) {
        throw 'Cache fixture cleanup path is outside the temporary directory.'
    }
    if (Test-Path -LiteralPath $temporary) { Remove-Item -LiteralPath $temporary -Recurse -Force }
}
