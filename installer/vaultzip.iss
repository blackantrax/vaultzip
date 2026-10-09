; VaultZip installer (Inno Setup 6).
; Build with installer\build-installer.ps1, or directly:
;   iscc /DAppVersion=0.1.0 /DBuildDir=..\target\release installer\vaultzip.iss
;
; The AppId below identifies VaultZip to Windows for upgrades and uninstall.
; Never change it after the first public release.

#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif
#ifndef BuildDir
  #define BuildDir "..\target\release"
#endif
#define AppName "VaultZip"
#define AppPublisher "Omerta Security"
#define AppExe "vaultzip-gui.exe"

[Setup]
AppId={{FD04F7A8-FD50-495B-AE31-9FF2E19B3EFC}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppPublisher}
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
; Per-user install: no administrator rights, no UAC prompt.
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputBaseFilename=VaultZip-Setup-{#AppVersion}
SetupIconFile=..\assets\vaultzip.ico
UninstallDisplayIcon={app}\{#AppExe}
LicenseFile=..\LICENSE
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ChangesAssociations=yes
CloseApplications=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "french"; MessagesFile: "compiler:Languages\French.isl"

[Tasks]
Name: "contextmenu"; Description: "Add VaultZip to the Explorer right-click menu"; GroupDescription: "Integration:"
Name: "openwith"; Description: "List VaultZip in the Open with menu for ZIP files"; GroupDescription: "Integration:"
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Shortcuts:"; Flags: unchecked

[Files]
Source: "{#BuildDir}\vaultzip-gui.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#BuildDir}\vaultzip.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Registry]
; Makes VaultZip appear under Open with for .zip files. Windows does not let an
; installer silently take over the default app, so the user chooses that.
Root: HKA; Subkey: "Software\Classes\VaultZip.Archive"; ValueType: string; ValueName: ""; ValueData: "ZIP archive (VaultZip)"; Flags: uninsdeletekey; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\VaultZip.Archive\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\{#AppExe},0"; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\VaultZip.Archive\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\.zip\OpenWithProgids"; ValueType: string; ValueName: "VaultZip.Archive"; ValueData: ""; Flags: uninsdeletevalue; Tasks: openwith

[Run]
; The program registers its own right-click menu, so the installer and the
; Settings page in the app always write the same entries.
Filename: "{app}\{#AppExe}"; Parameters: "--install-shell --silent"; Flags: runhidden; Tasks: contextmenu
Filename: "{app}\{#AppExe}"; Description: "Launch VaultZip"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "{app}\{#AppExe}"; Parameters: "--uninstall-shell --silent"; Flags: runhidden; RunOnceId: "RemoveShellMenu"
