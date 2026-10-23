# Uses a separately compiled test AppId: never installs over the user's Nida.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Installer,
    [Parameter(Mandatory)][string]$UpgradeInstaller
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$registry = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\pingvis.Nida.Windows.InstallerTest_is1'
if (Test-Path -LiteralPath $registry) { throw 'A previous isolated installer test is registered; refusing to replace it.' }
$temporary = Join-Path ([IO.Path]::GetTempPath()) ('Nida installer test ' + [guid]::NewGuid().ToString('N'))
$program = Join-Path $temporary 'Installed app'
$workspace = Join-Path $temporary 'My show/.varda'
New-Item -ItemType Directory -Path $workspace -Force | Out-Null
$sentinel = Join-Path $workspace 'operator-data.txt'
'Preserve this show' | Set-Content -LiteralPath $sentinel
# A fresh CI profile may not have created these known folders yet.
$startShortcut = Join-Path ([Environment]::GetFolderPath('Programs', 'DoNotVerify')) 'Nida Windows Installer Test.lnk'
$desktopShortcut = Join-Path ([Environment]::GetFolderPath('DesktopDirectory', 'DoNotVerify')) 'Nida Windows Installer Test.lnk'
if ((Test-Path -LiteralPath $startShortcut) -or (Test-Path -LiteralPath $desktopShortcut)) { throw 'Installer test shortcuts already exist.' }
function Install-Test($path, $log, [switch]$Upgrade) {
    $path = (Resolve-Path -LiteralPath $path).Path
    # Installer identity is checked before running, using its version resource.
    if ((Get-Item -LiteralPath $path).VersionInfo.ProductName.Trim() -ne 'Nida Windows Installer Test') { throw 'Only an isolated test installer may be exercised automatically.' }
    $arguments = @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/SP-', '/NOCLOSEAPPLICATIONS', '/TASKS=desktopicon', ('/LOG="' + (Join-Path $temporary $log) + '"'))
    if (!$Upgrade) { $arguments += '/DIR="' + $program + '"' }
    $process = Start-Process -FilePath $path -ArgumentList $arguments -WindowStyle Hidden -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw "Installer failed ($($process.ExitCode)); see $temporary" }
    $installed = Get-ItemProperty -LiteralPath $registry
    if ($installed.InstallLocation.TrimEnd('\') -ne $program) { throw 'Unexpected registered installation location.' }
    if (!(Test-Path -LiteralPath $startShortcut) -or !(Test-Path -LiteralPath $desktopShortcut)) { throw 'Expected shortcuts were not installed.' }
    $shell = New-Object -ComObject WScript.Shell
    foreach ($shortcut in @($startShortcut, $desktopShortcut)) {
        $link = $shell.CreateShortcut($shortcut)
        if ($link.TargetPath -ne (Join-Path $program 'varda.exe') -or $link.WorkingDirectory -ne $env:USERPROFILE -or $link.Arguments) { throw 'Shortcut must launch the installed app from the user profile without replacing the workspace.' }
        if ($link.IconLocation -ne ((Join-Path $program 'nida.ico') + ',0')) { throw 'Shortcut must use the installed Nida icon.' }
    }
    if ($installed.DisplayIcon -ne (Join-Path $program 'nida.ico')) { throw 'Uninstall entry must use the installed Nida icon.' }
    & (Join-Path $PSScriptRoot 'test-windows-package.ps1') -PackageDirectory $program
}
function Uninstall-Test {
    if (!(Test-Path -LiteralPath $registry)) { return }
    $registered = Get-ItemProperty -LiteralPath $registry
    if ($registered.InstallLocation.TrimEnd('\') -ne $program) { throw 'Refusing to uninstall a different installation.' }
    $uninstaller = Join-Path $program 'unins000.exe'
    $process = Start-Process -FilePath $uninstaller -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', ('/LOG="' + (Join-Path $temporary 'uninstall.log') + '"')) -WindowStyle Hidden -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw "Test uninstall failed; see $temporary" }
}
try {
    Install-Test $Installer 'install.log'
    $firstVersion = (Get-ItemProperty -LiteralPath $registry).DisplayVersion
    $personal = Join-Path $program 'operator-note.txt'
    'Personal file is not installer-owned' | Set-Content -LiteralPath $personal
    $owned = Join-Path $program 'installer-notes.txt'
    'Outdated installer-owned content' | Set-Content -LiteralPath $owned
    Install-Test $UpgradeInstaller 'upgrade.log' -Upgrade
    $nextVersion = (Get-ItemProperty -LiteralPath $registry).DisplayVersion
    if ($firstVersion -eq $nextVersion) { throw 'The upgrade fixture must have a different display version.' }
    if ((Get-Content -LiteralPath $owned -Raw) -notmatch '^Nida Windows - experimental core build') { throw 'Upgrade did not replace an installer-owned file.' }
    if ((Get-Content -LiteralPath $sentinel) -ne 'Preserve this show' -or !(Test-Path -LiteralPath $personal)) { throw 'Upgrade removed user data.' }
    Uninstall-Test
    if ((Test-Path -LiteralPath $registry) -or (Test-Path -LiteralPath (Join-Path $program 'varda.exe')) -or (Test-Path -LiteralPath $startShortcut) -or (Test-Path -LiteralPath $desktopShortcut)) { throw 'Uninstall left installer-owned app, shortcuts or registration behind.' }
    if ((Get-Content -LiteralPath $sentinel) -ne 'Preserve this show' -or !(Test-Path -LiteralPath $personal)) { throw 'Uninstall removed user data.' }
    Write-Host 'Installer passed install, version upgrade, shortcuts, installed-payload smoke tests and uninstall; user data was preserved.'
} finally {
    Uninstall-Test
    # Keep logs and the user-data fixtures for inspection. Only our isolated
    # installed product is uninstalled; no recursive removal of an app folder.
    Write-Host "Installer test logs and fixtures: $temporary"
}
