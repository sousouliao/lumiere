ManifestDPIAware true

!ifndef BUILD_UNINSTALLER
  !include WinMessages.nsh
  !include nsDialogs.nsh

  !define MUI_PAGE_CUSTOMFUNCTION_SHOW lumiereDirectoryPageShow
  !define MUI_PAGE_CUSTOMFUNCTION_LEAVE lumiereDirectoryPageLeave

  Function lumiereDirectoryPageShow
    Call normalizeLumiereDriveRoot
    nsDialogs::CreateTimer normalizeLumiereDriveRoot 100
  FunctionEnd

  Function lumiereDirectoryPageLeave
    nsDialogs::KillTimer normalizeLumiereDriveRoot
    Call normalizeLumiereDriveRoot
  FunctionEnd

  Function normalizeLumiereDriveRoot
    GetDlgItem $0 $HWNDPARENT 1019
    ${If} $0 != 0
      ${NSD_GetText} $0 $1
      StrLen $2 $1
      ${If} $2 == 3
        StrCpy $2 $1 1 1
        StrCpy $3 $1 1 2
        ${If} $2 == ":"
        ${AndIf} $3 == "\"
          StrCpy $INSTDIR "$1${APP_FILENAME}"
          ${NSD_SetText} $0 $INSTDIR
        ${EndIf}
      ${EndIf}
    ${EndIf}
  FunctionEnd
!endif

!macro customInstall
  IfFileExists "$INSTDIR\resources\windows-identity\Lumiere.Identity.msix" 0 identity_done
  DetailPrint "Registering Lumiere Windows identity"
  nsExec::ExecToLog 'powershell.exe -NoLogo -NoProfile -NonInteractive -WindowStyle Hidden -ExecutionPolicy Bypass -File "$INSTDIR\resources\windows-installer\register-identity.ps1" -InstallDirectory "$INSTDIR"'
  Pop $0
  ${If} $0 != 0
    DetailPrint "Windows identity registration failed; Lumiere will retain the system capture border."
  ${EndIf}
  identity_done:
!macroend

!macro customUnInstall
  ${IfNot} ${isUpdated}
    DetailPrint "Removing Lumiere Windows identity"
    nsExec::ExecToLog 'powershell.exe -NoLogo -NoProfile -NonInteractive -WindowStyle Hidden -ExecutionPolicy Bypass -File "$INSTDIR\resources\windows-installer\unregister-identity.ps1"'
    Pop $0
  ${EndIf}
!macroend
