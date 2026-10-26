; Dyna Island installer (Inno Setup 6.5.2+, for the PNG wizard images).
;
; Releases ship installer/dyna-island.nsi (NSIS), which 0.1.x installs came from; CI builds this one too as a check.
;
; Build after `cargo build -p dyna-island --release`:
;   iscc installer\dyna-island.iss                      -> dist\Dyna-Island-Setup-0.2.4.exe
;   iscc /DAppVersion=0.2.0 installer\dyna-island.iss   (another version)
;
; Per-user install: no admin rights and no UAC prompt. The app goes to
; %LOCALAPPDATA%\Programs\DynaIsland; settings and data stay in %APPDATA%\DynaIsland and
; survive uninstalling (only the cache in %LOCALAPPDATA%\DynaIsland is removed).

#ifndef AppVersion
  #define AppVersion "0.2.4"
#endif
#ifndef ExeSource
  #define ExeSource "..\target\release\dyna-island.exe"
#endif
#define AppName "Dyna Island"
; Folder and data names (the app uses the same ones).
#define AppDir "DynaIsland"
#define AppPublisher "Dyna Island"
#define AppExe "dyna-island.exe"
; The island window's class (crates/dyna-island/src/window.rs), used to ask a running copy to quit.
#define IslandClass "DynaIsland.Island"

[Setup]
; Never change the AppId: upgrades and the uninstaller find the install by it.
AppId={{6F1D2B7A-3C84-4E59-A1D6-8B2E5F0C9A47}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#AppPublisher}
AppCopyright=Copyright (C) 2026 {#AppPublisher}
VersionInfoVersion={#AppVersion}
VersionInfoCompany={#AppPublisher}
VersionInfoDescription={#AppName} Setup
VersionInfoProductName={#AppName}
PrivilegesRequired=lowest
DefaultDirName={localappdata}\Programs\{#AppDir}
DisableDirPage=yes
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir=..\dist
OutputBaseFilename=Dyna-Island-Setup-{#AppVersion}
SetupIconFile=..\crates\dyna-island\res\dyna-island.ico
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
WizardStyle=modern
; The island on the finish page, the icon in the header (both from tools/make_icon.py).
; Setup picks the file closest to its image area: 100%, 150% and 200% scaling.
WizardImageFile=art\wizard-202x386.png,art\wizard-336x643.png,art\wizard-430x824.png
WizardSmallImageFile=art\wizard-small-58.png,art\wizard-small-77.png,art\wizard-small-97.png,art\wizard-small-116.png,art\wizard-small-124.png,art\wizard-small-143.png,art\wizard-small-159.png
Compression=lzma2/max
SolidCompression=yes
; A running copy is asked to quit in PrepareToInstall; Restart Manager is the safety net.
CloseApplications=yes
RestartApplications=no

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "autostart"; Description: "Start with Windows"; GroupDescription: "Startup:"

[Files]
Source: "{#ExeSource}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\THIRD-PARTY-NOTICES.txt"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"; Comment: "A living notch for Windows"

[Registry]
; Same value the app writes from its "Start with Windows" menu item (autostart.rs): the quoted exe path.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "{#AppDir}"; ValueData: """{app}\{#AppExe}"""; Flags: uninsdeletevalue; Tasks: autostart

[Run]
Filename: "{app}\{#AppExe}"; Description: "Launch {#AppName}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
; Make sure no copy is still running before its files are removed.
Filename: "{sys}\taskkill.exe"; Parameters: "/IM {#AppExe} /F"; Flags: runhidden waituntilterminated; RunOnceId: "StopIsland"

[UninstallDelete]
; Cache and log only (dyna-island.log); settings and data in %APPDATA%\DynaIsland are kept.
Type: filesandordirs; Name: "{localappdata}\{#AppDir}"

[Code]
const
  RunKey = 'Software\Microsoft\Windows\CurrentVersion\Run';
  WM_CLOSE = $0010;

{ Ask a running Dyna Island to quit (it saves its data on the way out); force it if it doesn't within 3 s. }
procedure StopIsland();
var
  I, ResultCode: Integer;
begin
  if FindWindowByClassName('{#IslandClass}') = 0 then
    Exit;
  PostMessage(FindWindowByClassName('{#IslandClass}'), WM_CLOSE, 0, 0);
  for I := 1 to 30 do
  begin
    if FindWindowByClassName('{#IslandClass}') = 0 then
      Exit;
    Sleep(100);
  end;
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/IM {#AppExe} /F', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  StopIsland();
  Result := '';
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  { Unticking "Start with Windows" on a reinstall turns it off, even if the app's menu turned it on. }
  if (CurStep = ssPostInstall) and not WizardIsTaskSelected('autostart') then
    RegDeleteValue(HKEY_CURRENT_USER, RunKey, '{#AppDir}');
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  { Runs before [UninstallRun], so a running copy gets the chance to save and quit cleanly. }
  if CurUninstallStep = usUninstall then
    StopIsland();
  { The app may have written the Run value itself; never leave it pointing at a removed exe. }
  if CurUninstallStep = usPostUninstall then
    RegDeleteValue(HKEY_CURRENT_USER, RunKey, '{#AppDir}');
end;
