; The payload is the already verified portable package. Keep user data outside it.
#ifndef PayloadDir
  #error PayloadDir is required
#endif
#ifndef ProductVersion
  #error ProductVersion is required
#endif
#ifndef FileVersion
  #error FileVersion is required
#endif
#ifndef InstallerId
  #define InstallerId "pingvis.Nida.Windows"
#endif
#ifndef ProductName
  #define ProductName "Nida Windows"
#endif

[Setup]
AppId={#InstallerId}
AppName={#ProductName}
AppVersion={#ProductVersion}
AppPublisher=Nida contributors
AppPublisherURL=https://github.com/pingvis/Nida-Windows
AppSupportURL=https://github.com/pingvis/Nida-Windows/issues
DefaultDirName={localappdata}\Programs\{#ProductName}
DefaultGroupName={#ProductName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
SetupIconFile={#PayloadDir}\nida.ico
UninstallDisplayIcon={app}\nida.ico
VersionInfoVersion={#FileVersion}
#ifdef TestInstaller
; Disposable upgrade fixtures exercise the same files and installer behavior.
; Avoid recompressing the full Chromium payload with LZMA twice for each run.
Compression=zip
#else
Compression=lzma2
#endif
SolidCompression=yes
WizardStyle=modern
SetupLogging=yes
CloseApplications=no
RestartApplications=no
LicenseFile={#PayloadDir}\licenses\FFmpeg.txt
InfoBeforeFile=installer-notes.txt

[Tasks]
Name: desktopicon; Description: "Create a desktop shortcut"; Flags: unchecked

[Files]
Source: "{#PayloadDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "installer-notes.txt"; DestDir: "{app}"

[Icons]
Name: "{autoprograms}\{#ProductName}"; Filename: "{app}\varda.exe"; WorkingDir: "{%USERPROFILE}"; IconFilename: "{app}\nida.ico"
Name: "{autodesktop}\{#ProductName}"; Filename: "{app}\varda.exe"; WorkingDir: "{%USERPROFILE}"; IconFilename: "{app}\nida.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\varda.exe"; Description: "Launch {#ProductName}"; WorkingDir: "{%USERPROFILE}"; Flags: nowait postinstall skipifsilent unchecked

; No wildcard deletion or user-data cleanup. Inno uninstalls its logged files;
; the user's .varda workspaces, recordings and other unknown files are retained.
