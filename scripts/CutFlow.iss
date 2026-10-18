#ifndef Stage
#error Stage directory is required
#endif
#ifndef Output
#define Output "..\dist"
#endif
#ifndef AppVersion
#error AppVersion is required
#endif
[Setup]
AppId={{F1A6C8E4-6378-4819-A862-762A5C58B140}
AppName=CutFlow
AppVersion={#AppVersion}
AppPublisher=daodeqing
DefaultDirName=D:\CutFlow
DefaultGroupName=CutFlow
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#Output}
OutputBaseFilename=CutFlow-{#AppVersion}-windows-x64-setup
Compression=lzma2
SolidCompression=yes
UninstallDisplayIcon={app}\CutFlow.exe
SetupIconFile={#Stage}\CutFlow.ico
[Files]
Source: "{#Stage}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
[Icons]
Name: "{group}\CutFlow"; Filename: "{app}\CutFlow.exe"; IconFilename: "{app}\CutFlow.ico"
Name: "{autodesktop}\CutFlow"; Filename: "{app}\CutFlow.exe"; IconFilename: "{app}\CutFlow.ico"; Tasks: desktopicon
[Tasks]
Name: desktopicon; Description: "Create a desktop shortcut"; Flags: unchecked
[Run]
Filename: "{app}\CutFlow.exe"; Description: "Launch CutFlow"; Flags: nowait postinstall skipifsilent
; No user media, project, credential or download data is deleted on uninstall.
