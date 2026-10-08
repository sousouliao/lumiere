; Private transaction helper is extracted outside $INSTDIR.
Var LumiereTransaction
!macro LumiereExtractHelper
  InitPluginsDir
  File /oname=$PLUGINSDIR\lumiere-installer.exe "${__FILEDIR__}\..\..\..\..\target\release\lumiere-installer.exe"
!macroend
!macro LumiereExec command
  ClearErrors
  ExecWait '${command}' $0
  ${If} ${Errors}
    StrCpy $0 1
  ${EndIf}
!macroend
!macro NSIS_HOOK_PREINSTALL
  !insertmacro LumiereExtractHelper
  File /oname=$PLUGINSDIR\payload.json "${__FILEDIR__}\..\..\..\..\artifacts\windows\tauri\payload.json"
  System::Call 'kernel32::GetCurrentProcessId() i .r0'
  !insertmacro LumiereExec '"$PLUGINSDIR\lumiere-installer.exe" begin "$INSTDIR" $0 "$DESKTOP\Lumiere.lnk" "$SMPROGRAMS\Lumiere.lnk"'
  ${If} $0 != 0
    DetailPrint "Private helper failed. See $TEMP\Lumiere-installer-error.txt."
    Abort "Could not prepare a safe upgrade. Program files were not replaced."
  ${EndIf}
  StrCpy $LumiereTransaction 1
  ClearErrors
!macroend
Function LumiereRollback
  ${If} $LumiereTransaction = 1
    !insertmacro LumiereExec '"$PLUGINSDIR\lumiere-installer.exe" rollback "$INSTDIR"'
    ${If} $0 != 0
      DetailPrint "Rollback could not finish. Rerun the installer to recover the retained backup."
    ${Else}
      StrCpy $LumiereTransaction 0
    ${EndIf}
  ${EndIf}
FunctionEnd
!macro NSIS_HOOK_POSTINSTALL
  !insertmacro LumiereExec '"$PLUGINSDIR\lumiere-installer.exe" commit "$INSTDIR" "$PLUGINSDIR\payload.json"'
  ${If} $0 != 0
    Call LumiereRollback
    Abort "Could not finish the upgrade. Rerun the installer to recover the previous version."
  ${EndIf}
  StrCpy $LumiereTransaction 0
!macroend
