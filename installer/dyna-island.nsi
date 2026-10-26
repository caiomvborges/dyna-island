; Dyna Island installer (NSIS 3). Per-user install, no admin rights needed.
;
; The name shown to people is "Dyna Island"; the install folder,
; the uninstall key, the Run value and the data folders are all "DynaIsland".
;
; Build (Linux/macOS/Windows with makensis):
;   makensis -DEXE=/abs/path/dyna-island.exe -DOUTDIR=/abs/path/dist installer/dyna-island.nsi
; tools/package.sh does this for you.

; Native 64-bit installer (Dyna Island itself is x64-only).
Target amd64-unicode
SetCompressor /SOLID lzma

!define APPNAME "Dyna Island"
!define VERSION "0.2.4"
!define PUBLISHER "Dyna Island"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\DynaIsland"
!define RUN_KEY "Software\Microsoft\Windows\CurrentVersion\Run"

!ifndef EXE
  !error "Pass -DEXE=<path to dyna-island.exe>"
!endif
!ifndef OUTDIR
  !define OUTDIR "."
!endif

Name "${APPNAME}"
OutFile "${OUTDIR}/Dyna-Island-Setup-${VERSION}.exe"
RequestExecutionLevel user
InstallDir "$LOCALAPPDATA\Programs\DynaIsland"
InstallDirRegKey HKCU "Software\DynaIsland" "InstallDir"
BrandingText "${APPNAME} ${VERSION}"

VIProductVersion "0.2.4.0"
VIAddVersionKey "ProductName" "${APPNAME}"
VIAddVersionKey "CompanyName" "${PUBLISHER}"
VIAddVersionKey "FileDescription" "Dyna Island installer"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "Copyright (C) 2026 ${PUBLISHER}"

!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "WinMessages.nsh"

; Ask a running Dyna Island to quit (it saves its data on WM_CLOSE), wait up to 3 s,
; then force-close with the fully qualified system taskkill.
!macro StopIsland
  FindWindow $0 "DynaIsland.Island"
  ${If} $0 <> 0
    SendMessage $0 ${WM_CLOSE} 0 0 /TIMEOUT=2000
    StrCpy $1 0
    ${DoWhile} $0 <> 0
      ${If} $1 >= 30
        ${Break}
      ${EndIf}
      Sleep 100
      FindWindow $0 "DynaIsland.Island"
      IntOp $1 $1 + 1
    ${Loop}
  ${EndIf}
  nsExec::Exec '"$SYSDIR\taskkill.exe" /IM dyna-island.exe /F'
  Sleep 200
!macroend
!define MUI_ICON "../crates/dyna-island/res/dyna-island.ico"
!define MUI_UNICON "../crates/dyna-island/res/dyna-island.ico"
; The island on the welcome and finish pages, the icon in the header (tools/make_icon.py).
!define MUI_WELCOMEFINISHPAGE_BITMAP "art/nsis-welcome.bmp"
!define MUI_HEADERIMAGE
!define MUI_HEADERIMAGE_RIGHT
!define MUI_HEADERIMAGE_BITMAP "art/nsis-header.bmp"
!define MUI_ABORTWARNING
!define MUI_COMPONENTSPAGE_SMALLDESC
!define MUI_FINISHPAGE_RUN "$INSTDIR\dyna-island.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Launch Dyna Island now"
!define MUI_WELCOMEPAGE_TITLE "Welcome to Dyna Island"
!define MUI_WELCOMEPAGE_TEXT "Dyna Island puts a living notch at the top of your screen: music controls, a focus timer, tasks, notes, a dock for your apps, your agenda and more.$\r$\n$\r$\nIt installs just for you and doesn't need administrator rights."

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "Dyna Island" SecMain
  SectionIn RO
  ; Close a running copy so the exe can be replaced.
  !insertmacro StopIsland
  SetOutPath "$INSTDIR"
  File "/oname=dyna-island.exe" "${EXE}"
  ; The licenses of what dyna-island.exe is built from.
  File "../THIRD-PARTY-NOTICES.txt"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  CreateShortcut "$SMPROGRAMS\${APPNAME}.lnk" "$INSTDIR\dyna-island.exe" "" "$INSTDIR\dyna-island.exe" 0
  WriteRegStr HKCU "Software\DynaIsland" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayName" "${APPNAME}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINST_KEY}" "Publisher" "${PUBLISHER}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\dyna-island.exe"
  WriteRegStr HKCU "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr HKCU "${UNINST_KEY}" "QuietUninstallString" '"$INSTDIR\Uninstall.exe" /S'
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoRepair" 1
  WriteRegDWORD HKCU "${UNINST_KEY}" "EstimatedSize" 7680
SectionEnd

Section "Start Dyna Island with Windows" SecAutostart
  ; Same value the app's own "Start with Windows" toggle writes.
  DeleteRegValue HKCU "${RUN_KEY}" "Poni"
  WriteRegStr HKCU "${RUN_KEY}" "DynaIsland" '"$INSTDIR\dyna-island.exe"'
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecMain} "The Dyna Island app and a Start Menu shortcut."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecAutostart} "Open Dyna Island automatically when you sign in. You can change this later from its menu."
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Section "Uninstall"
  !insertmacro StopIsland
  Delete "$INSTDIR\dyna-island.exe"
  Delete "$INSTDIR\THIRD-PARTY-NOTICES.txt"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\${APPNAME}.lnk"
  DeleteRegValue HKCU "${RUN_KEY}" "DynaIsland"
  DeleteRegKey HKCU "${UNINST_KEY}"
  DeleteRegKey HKCU "Software\DynaIsland"
  ; Cache and log go too.
  RMDir /r "$LOCALAPPDATA\DynaIsland"
  ; Your settings and data (tasks, notes, focus history) in %APPDATA%\DynaIsland are kept.
SectionEnd
