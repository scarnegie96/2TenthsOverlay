; 2Tenths installer (NSIS 3, Unicode).
;
;   makensis /DVERSION=0.5.0 /DEXE=target/release/tenths.exe app/installer/tenths.nsi
;
; Optional defines: /DOUTFILE=<path of the installer>, /DICON=<icon.ico>,
; /DVERSION_NUM=0.5.0.0 (four numbers; VERSION plus ".0" by default).
;
; It lays things out exactly as the installer of the Tauri build (0.4.x) did, so
; the old app's updater can run this one and it replaces the old install in place:
;
;   * per user, no administrator rights: files in $LOCALAPPDATA\Tenths (an
;     earlier choice of folder is kept: it is read from the uninstall entry);
;   * the program is tenths.exe, with the uninstaller next to it;
;   * the uninstall entry's key stays "Tenths" (below), though it's shown as 2Tenths;
;   * the Start menu shortcut is $SMPROGRAMS\2Tenths.lnk (an old Tenths.lnk is
;     replaced by it);
;   * the uninstall entry is HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Tenths.
;
; Switches (the Tauri updater runs "<installer> /P /UPDATE /R /ARGS <command line>"):
;   /S       silent: no windows at all (NSIS's own switch)
;   /P       passive: only the progress window, no questions
;   /R       start 2Tenths when the install is done (silent and passive; the
;            last page has a "Run Tenths" box otherwise)
;   /UPDATE  accepted and ignored
;   /ARGS …  accepted and ignored (everything after it is left alone)
;   /D=dir   NSIS's own: install into dir (must be last)
;
; Settings, layouts, laps and reports live in %APPDATA%\dev.tenths.overlay and
; %LOCALAPPDATA%\dev.tenths.overlay: neither installing nor uninstalling touches them.

Unicode true
SetCompressor /SOLID lzma
RequestExecutionLevel user
ManifestDPIAware true

!ifndef VERSION
  !error "Pass the version: makensis /DVERSION=0.5.0 /DEXE=path\to\tenths.exe tenths.nsi"
!endif
!ifndef EXE
  !error "Pass the program: makensis /DVERSION=0.5.0 /DEXE=path\to\tenths.exe tenths.nsi"
!endif
!ifndef VERSION_NUM
  !define VERSION_NUM "${VERSION}.0"
!endif
!ifndef ICON
  !define ICON "..\assets\icons\icon.ico"
!endif
!ifndef OUTFILE
  !define OUTFILE "2Tenths_${VERSION}_x64-setup.exe"
!endif

; PRODUCT names the install folder and the uninstall entry, as the Tauri build had them, so
; updates land on top of existing installs. DISPLAY is the name people see.
!define PRODUCT "Tenths"
!define DISPLAY "2Tenths"
!define MAINBINARY "tenths.exe"
!define PUBLISHER "Scott Carnegie"
!define URL "https://github.com/scarnegie96/2TenthsOverlay"
!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT}"

Name "${DISPLAY}"
OutFile "${OUTFILE}"
BrandingText "${DISPLAY} ${VERSION}"
InstallDir "$LOCALAPPDATA\${PRODUCT}"

VIProductVersion "${VERSION_NUM}"
VIAddVersionKey "ProductName" "${DISPLAY}"
VIAddVersionKey "FileDescription" "${DISPLAY} installer"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "Copyright (C) 2026 ${PUBLISHER}"

!include "MUI2.nsh"
!include "FileFunc.nsh"
!include "WordFunc.nsh"
!include "LogicLib.nsh"

Var Passive
Var RunAfter

; --- Pages -----------------------------------------------------------------------

!define MUI_ICON "${ICON}"
!define MUI_UNICON "${ICON}"
!define MUI_ABORTWARNING

; /P: nothing but the progress page.
!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES

!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
!define MUI_FINISHPAGE_RUN
!define MUI_FINISHPAGE_RUN_TEXT "Run ${DISPLAY}"
!define MUI_FINISHPAGE_RUN_FUNCTION LaunchApp
!define MUI_FINISHPAGE_SHOWREADME ""
!define MUI_FINISHPAGE_SHOWREADME_TEXT "Create a desktop shortcut"
!define MUI_FINISHPAGE_SHOWREADME_FUNCTION CreateDesktopShortcut
!define MUI_FINISHPAGE_SHOWREADME_NOTCHECKED
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

; --- Helpers -----------------------------------------------------------------------

Function SkipIfPassive
  ${If} $Passive == 1
    Abort
  ${EndIf}
FunctionEnd

Function LaunchApp
  SetOutPath "$INSTDIR"
  Exec '"$INSTDIR\${MAINBINARY}"'
FunctionEnd

Function CreateDesktopShortcut
  CreateShortCut "$DESKTOP\${DISPLAY}.lnk" "$INSTDIR\${MAINBINARY}"
FunctionEnd

; Reads the switches before /ARGS (everything after /ARGS belongs to the app being updated).
Function ParseSwitches
  StrCpy $Passive 0
  StrCpy $RunAfter 0
  ${GetParameters} $R0
  StrCmp $R0 "" done
  StrCpy $R2 1
  loop:
    IntCmp $R2 64 done 0 done
    ClearErrors
    ${WordFind} "$R0" " " "+$R2" $R1
    IfErrors done
    StrCmp $R1 "/ARGS" done
    StrCmp $R1 "/P" 0 +2
      StrCpy $Passive 1
    StrCmp $R1 "/R" 0 +2
      StrCpy $RunAfter 1
    IntOp $R2 $R2 + 1
    Goto loop
  done:
FunctionEnd

; Asks Tenths to close, then makes sure. Not /T: the app that started this
; installer is a parent of it and must not take it down with it. taskkill
; answers 0 when it found the program (128 when nothing is running).
!macro CloseAppBody
  nsExec::Exec 'taskkill /IM "${MAINBINARY}"'
  Pop $0
  ${If} $0 == 0
    Sleep 1500
    nsExec::Exec 'taskkill /F /IM "${MAINBINARY}"'
    Pop $0
    Sleep 300
  ${EndIf}
!macroend

Function CloseApp
  !insertmacro CloseAppBody
FunctionEnd

Function un.CloseApp
  !insertmacro CloseAppBody
FunctionEnd

Function .onInit
  Call ParseSwitches
  ; Where it was installed before (the Tauri build wrote this too), unless /D= says otherwise.
  ${If} $INSTDIR == "$LOCALAPPDATA\${PRODUCT}"
    ReadRegStr $0 HKCU "${UNINSTKEY}" "InstallLocation"
    ${If} $0 != ""
      StrCpy $INSTDIR $0
    ${EndIf}
  ${EndIf}
FunctionEnd

; /R with no finish page to ask on: silent and passive installs start the app themselves.
Function .onInstSuccess
  ${If} $RunAfter == 1
    ${If} $Passive == 1
    ${OrIf} ${Silent}
      Call LaunchApp
    ${EndIf}
  ${EndIf}
FunctionEnd

; --- Install -------------------------------------------------------------------------

Section "Install"
  SetShellVarContext current
  Call CloseApp
  SetOutPath "$INSTDIR"

  ; The old program may take a moment to let go of its file.
  StrCpy $R9 0
  retry:
    ClearErrors
    Delete "$INSTDIR\${MAINBINARY}"
    IfFileExists "$INSTDIR\${MAINBINARY}" 0 free
    IntOp $R9 $R9 + 1
    ${If} $R9 >= 10
      Goto free
    ${EndIf}
    Sleep 500
    Goto retry
  free:

  File "/oname=${MAINBINARY}" "${EXE}"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  ; Start menu (and the desktop, when it has one already: it is kept pointing here).
  ; The shortcuts were called Tenths before the rename: those are replaced.
  Delete "$SMPROGRAMS\${PRODUCT}.lnk"
  CreateShortCut "$SMPROGRAMS\${DISPLAY}.lnk" "$INSTDIR\${MAINBINARY}"
  ${If} ${FileExists} "$DESKTOP\${PRODUCT}.lnk"
    Delete "$DESKTOP\${PRODUCT}.lnk"
    CreateShortCut "$DESKTOP\${DISPLAY}.lnk" "$INSTDIR\${MAINBINARY}"
  ${ElseIf} ${FileExists} "$DESKTOP\${DISPLAY}.lnk"
    CreateShortCut "$DESKTOP\${DISPLAY}.lnk" "$INSTDIR\${MAINBINARY}"
  ${EndIf}

  ; Apps & features.
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayName" "${DISPLAY}"
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayIcon" "$\"$INSTDIR\${MAINBINARY}$\""
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTKEY}" "Publisher" "${PUBLISHER}"
  WriteRegStr HKCU "${UNINSTKEY}" "URLInfoAbout" "${URL}"
  WriteRegStr HKCU "${UNINSTKEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTKEY}" "UninstallString" "$\"$INSTDIR\uninstall.exe$\""
  WriteRegStr HKCU "${UNINSTKEY}" "QuietUninstallString" "$\"$INSTDIR\uninstall.exe$\" /S"
  WriteRegDWORD HKCU "${UNINSTKEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTKEY}" "NoRepair" 1
  WriteRegDWORD HKCU "${UNINSTKEY}" "EstimatedSize" $0
SectionEnd

; --- Uninstall ------------------------------------------------------------------------

Section "Uninstall"
  SetShellVarContext current
  Call un.CloseApp

  Delete "$INSTDIR\${MAINBINARY}"
  Delete "$INSTDIR\uninstall.exe"
  ; Only if nothing else is in it.
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\${DISPLAY}.lnk"
  Delete "$DESKTOP\${DISPLAY}.lnk"
  Delete "$SMPROGRAMS\${PRODUCT}.lnk"
  Delete "$DESKTOP\${PRODUCT}.lnk"
  DeleteRegKey HKCU "${UNINSTKEY}"
  ; Settings, layouts, laps and reports (%APPDATA%\dev.tenths.overlay) stay, so a
  ; reinstall picks them up again.
SectionEnd
