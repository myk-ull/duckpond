; Duck Pond installer for Windows (NSIS).
; Per-user install: no admin prompt. Build with:  makensis installer/windows/duckpond.nsi
; Expects the release exe at target/x86_64-pc-windows-gnu/release/duckpond.exe

Unicode true
!define APPNAME "Duck Pond"
!define EXE "DuckPond.exe"
!define VERSION "0.1.0"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\DuckPond"

Name "${APPNAME}"
OutFile "..\..\dist\DuckPondSetup.exe"
InstallDir "$LOCALAPPDATA\Programs\DuckPond"
RequestExecutionLevel user
SetCompressor /SOLID lzma
Icon "..\..\assets\app.ico"
UninstallIcon "..\..\assets\app.ico"
BrandingText "Duck Pond ${VERSION}"
VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "${APPNAME}"
VIAddVersionKey "FileDescription" "${APPNAME} installer"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "Monet's Water Lilies (1906) is public domain; quack credits in CREDITS.md"

!include "MUI2.nsh"
!define MUI_ICON "..\..\assets\app.ico"
!define MUI_UNICON "..\..\assets\app.ico"
!define MUI_WELCOMEPAGE_TITLE "Welcome to Duck Pond"
!define MUI_WELCOMEPAGE_TEXT "Duck Pond turns the icons on your desktop into little ducks swimming on Monet's water lilies.$\r$\n$\r$\nDouble-click a duck to open its file, right-click for the usual menu, click the water to feed them, and double-click the water to send everyone home.$\r$\n$\r$\nYour real desktop icons are hidden while the pond runs and come back when you quit or uninstall."
!define MUI_FINISHPAGE_RUN "$INSTDIR\${EXE}"
!define MUI_FINISHPAGE_RUN_TEXT "Start the pond now"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "Install"
  ; upgrading: close the running pond politely first (it restores the icons)
  IfFileExists "$INSTDIR\${EXE}" 0 +3
    ExecWait '"$INSTDIR\${EXE}" --quit'
    Sleep 500

  SetOutPath "$INSTDIR"
  File /oname=${EXE} "..\..\target\x86_64-pc-windows-gnu\release\duckpond.exe"
  File /oname=CREDITS.md "..\..\assets\quacks\CREDITS.md"
  WriteUninstaller "$INSTDIR\Uninstall.exe"

  CreateShortcut "$SMPROGRAMS\${APPNAME}.lnk" "$INSTDIR\${EXE}" "" "$INSTDIR\${EXE}" 0

  ; show up in Settings > Apps
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayName" "${APPNAME}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINST_KEY}" "Publisher" "Duck Pond"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\${EXE}"
  WriteRegStr HKCU "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoRepair" 1
  ; open at login (the app also manages this from its tray menu)
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "DuckPond" '"$INSTDIR\${EXE}"'
SectionEnd

Section "Uninstall"
  ; stop the pond and make sure the real desktop icons are back
  ExecWait '"$INSTDIR\${EXE}" --quit'
  Sleep 500
  ExecWait '"$INSTDIR\${EXE}" --restore'

  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "DuckPond"
  DeleteRegKey HKCU "${UNINST_KEY}"
  Delete "$SMPROGRAMS\${APPNAME}.lnk"
  Delete "$INSTDIR\${EXE}"
  Delete "$INSTDIR\CREDITS.md"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  ; settings (remembered duck spots) live in %APPDATA%\DuckPond and are kept
SectionEnd
