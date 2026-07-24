; Tauri CLI 2.11.3 includes this file before defining its installer sections.
; The hook macros below are expanded only after MAINBINARYNAME, BUNDLEID,
; MANUPRODUCTKEY and the Tauri installer variables have been defined.

Var InstallDataOldLocation
Var InstallDataUserSid
Var InstallDataLegacyAppData
Var InstallDataDeleteRequested

!define INSTALL_DATA_PROBE_COMMAND "JABwAD0AWwBFAG4AdgBpAHIAbwBuAG0AZQBuAHQAXQA6ADoARwBlAHQARQBuAHYAaQByAG8AbgBtAGUAbgB0AFYAYQByAGkAYQBiAGwAZQAoACIAVwBBAFQAQwBIAEIAQQBDAEsAXwBEAEEAVABBAF8AUABSAE8AQgBFACIALAAiAFAAcgBvAGMAZQBzAHMAIgApADsAdAByAHkAewBbAEkATwAuAEYAaQBsAGUAXQA6ADoAVwByAGkAdABlAEEAbABsAFQAZQB4AHQAKAAkAHAALAAiAG8AawAiACwAWwBUAGUAeAB0AC4ARQBuAGMAbwBkAGkAbgBnAF0AOgA6AEEAUwBDAEkASQApADsAZQB4AGkAdAAgADAAfQBjAGEAdABjAGgAewBlAHgAaQB0ACAAMQB9AA=="

!macro InstallDataAbort MESSAGE
  MessageBox MB_OK|MB_ICONSTOP "${MESSAGE}" /SD IDOK
  Abort
!macroend

!macro NSIS_HOOK_PREINSTALL
  ; Capture the previous location before Tauri overwrites its own location keys.
  ReadRegStr $InstallDataOldLocation SHCTX "Software\${BUNDLEID}" "InstallLocation"
  ${If} $InstallDataOldLocation == ""
    ReadRegStr $InstallDataOldLocation SHCTX "${MANUPRODUCTKEY}" ""
  ${EndIf}

  ; Resolve the owner of the Explorer shell in this interactive session. This
  ; remains the original user when UAC credentials belong to another account.
  nsExec::ExecToStack '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "$$s=[Diagnostics.Process]::GetCurrentProcess().SessionId;$$p=Get-CimInstance Win32_Process|Where-Object{$$_.Name-eq$\"explorer.exe$\"-and$$_.SessionId-eq$$s}|Select-Object -First 1;if($$null-eq$$p){exit 2};$$r=Invoke-CimMethod -InputObject $$p -MethodName GetOwnerSid;if($$r.ReturnValue-ne 0){exit 3};[Console]::Out.Write($$r.Sid)"'
  Pop $0
  Pop $InstallDataUserSid
  ${If} $0 != 0
    !insertmacro InstallDataAbort "无法确认当前交互用户。安装将停止。"
  ${EndIf}

  StrCpy $1 $InstallDataUserSid 4
  ${If} $1 != "S-1-"
  ${OrIf} $InstallDataUserSid == "S-1-5-18"
  ${OrIf} $InstallDataUserSid == "S-1-5-19"
  ${OrIf} $InstallDataUserSid == "S-1-5-20"
    !insertmacro InstallDataAbort "无法确认当前交互用户。安装将停止。"
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL

  ; Resolve the same user's roaming AppData from that user's loaded registry
  ; hive. Do not use the elevated process's shell variables.
  ReadRegStr $2 HKLM "Software\Microsoft\Windows NT\CurrentVersion\ProfileList\$InstallDataUserSid" "ProfileImagePath"
  ExpandEnvStrings $2 "$2"
  ReadRegStr $InstallDataLegacyAppData HKU "$InstallDataUserSid\Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders" "AppData"
  ${If} $2 == ""
    StrCpy $InstallDataLegacyAppData ""
  ${ElseIf} $InstallDataLegacyAppData != ""
    StrCpy $3 $InstallDataLegacyAppData
    ${StrCase} $3 $3 "U"
    ${StrLoc} $4 $3 "%USERPROFILE%" ">"
    ${If} $4 != ""
      StrCpy $5 $InstallDataLegacyAppData $4
      IntOp $4 $4 + 13
      StrCpy $3 $InstallDataLegacyAppData "" $4
      StrCpy $InstallDataLegacyAppData "$5$2$3"
    ${EndIf}
    ExpandEnvStrings $InstallDataLegacyAppData "$InstallDataLegacyAppData"
  ${EndIf}

  ; Prefer a different old install directory, then the original user's legacy
  ; AppData, then no source ("-"). Published destination data is protected by
  ; the migration helper; the explicit guard also covers same-directory reuse.
  StrCpy $0 "-"
  ${If} $InstallDataOldLocation != ""
  ${AndIf} $InstallDataOldLocation != "$INSTDIR"
  ${AndIf} ${FileExists} "$InstallDataOldLocation\data\*.*"
    StrCpy $0 "$InstallDataOldLocation\data"
    Goto install_data_source_selected
  ${EndIf}
  ; A same-directory upgrade (or a reinstall into a preserved directory) uses
  ; no source so the already-published destination remains authoritative.
  ${If} ${FileExists} "$INSTDIR\data\*.*"
    Goto install_data_source_selected
  ${EndIf}
  ${If} $InstallDataLegacyAppData != ""
  ${AndIf} ${FileExists} "$InstallDataLegacyAppData\local.owstats.desktop\*.*"
    StrCpy $0 "$InstallDataLegacyAppData\local.owstats.desktop"
  ${EndIf}

  install_data_source_selected:
  nsExec::ExecToStack '"$PLUGINSDIR\watchback-trusted-helper.exe" --secure-install-data "$0" "$INSTDIR\data" "$InstallDataUserSid"'
  Pop $1
  Pop $2
  StrCpy $2 ""
  ${If} $1 != 0
    !insertmacro InstallDataAbort "无法迁移或创建安装目录数据。安装将停止。"
  ${EndIf}

  ; Verify the ACL with a process launched from Tauri's original shell token,
  ; not with the elevated installer token. The encoded command writes only the
  ; path supplied through this process-local environment variable.
  System::Call 'kernel32::GetTickCount() i.r5'
  StrCpy $6 "$INSTDIR\data\.write-test-$5"
  ${If} ${FileExists} "$6"
    !insertmacro InstallDataAbort "无法验证数据目录写权限。安装将停止。"
  ${EndIf}
  System::Call 'kernel32::SetEnvironmentVariableW(w "WATCHBACK_DATA_PROBE", w "$6") i.r7'
  ${If} $7 == 0
    !insertmacro InstallDataAbort "无法验证数据目录写权限。安装将停止。"
  ${EndIf}
  nsis_tauri_utils::RunAsUser "$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" "-NoLogo -NoProfile -NonInteractive -WindowStyle Hidden -ExecutionPolicy Bypass -EncodedCommand ${INSTALL_DATA_PROBE_COMMAND}"
  Pop $7
  System::Call 'kernel32::SetEnvironmentVariableW(w "WATCHBACK_DATA_PROBE", p 0) i.r4'
  ${If} $7 != 0
    !insertmacro InstallDataAbort "无法验证数据目录写权限。安装将停止。"
  ${EndIf}

  StrCpy $7 0
  install_data_probe_wait:
    ${If} ${FileExists} "$6"
      Goto install_data_probe_ready
    ${EndIf}
    IntOp $7 $7 + 1
    ${If} $7 >= 100
      Goto install_data_probe_failed
    ${EndIf}
    Sleep 100
    Goto install_data_probe_wait

  install_data_probe_ready:
    ClearErrors
    Delete "$INSTDIR\data\.write-test-$5"
    ${If} ${Errors}
      Goto install_data_probe_failed
    ${EndIf}
    Goto install_data_probe_complete

  install_data_probe_failed:
    Delete "$INSTDIR\data\.write-test-$5"
    !insertmacro InstallDataAbort "安装目录中的数据目录不可写。安装将停止。"

  install_data_probe_complete:
  WriteRegStr SHCTX "Software\${BUNDLEID}" "InstallLocation" "$INSTDIR"
  StrCpy $InstallDataOldLocation ""
  StrCpy $InstallDataLegacyAppData ""
  StrCpy $InstallDataUserSid ""
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Tauri's built-in checkbox targets AppData. It must never remove the legacy
  ; migration source, so only the explicit exact-data choice below is honored.
  StrCpy $DeleteAppDataCheckboxState 0
  StrCpy $InstallDataDeleteRequested 0

  ${If} $WatchbackNestedUpgrade == 1
  ${OrIf} $UpdateMode == 1
  ${OrIf} $PassiveMode == 1
    Goto install_data_delete_choice_done
  ${EndIf}

  GetFullPathName $0 "$INSTDIR"
  GetFullPathName $1 "$INSTDIR\data"
  ReadRegStr $2 SHCTX "Software\${BUNDLEID}" "InstallLocation"
  GetFullPathName $2 "$2"
  ${If} $0 == ""
  ${OrIf} $1 != "$0\data"
  ${OrIf} $2 != "$0"
  ${OrIfNot} ${FileExists} "$INSTDIR\${MAINBINARYNAME}.exe"
    !insertmacro InstallDataAbort "无法确认本地数据目录。卸载将停止。"
  ${EndIf}

  MessageBox MB_YESNO|MB_DEFBUTTON2|MB_ICONEXCLAMATION \
    "是否同时删除本地数据？这将删除账号凭据、数据库、赛季数据、备份和浏览器登录状态。$\r$\n$\r$\n默认选择“否”以保留数据。" \
    /SD IDNO IDYES install_data_delete_yes IDNO install_data_delete_keep
  install_data_delete_yes:
    StrCpy $InstallDataDeleteRequested 1
    Goto install_data_delete_choice_done
  install_data_delete_keep:
    MessageBox MB_OK|MB_ICONINFORMATION \
      "本地数据将保留在原安装目录；卸载后该目录可能不会被完全移除。" \
      /SD IDOK
  install_data_delete_choice_done:
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; The resolved Tauri template performs its application-running check after
  ; PREUNINSTALL and before this hook, so destructive work belongs here.
  ${If} $InstallDataDeleteRequested == 1
    nsExec::ExecToStack '"$PLUGINSDIR\watchback-trusted-helper.exe" --trusted-delete-install-data "$INSTDIR" "$INSTDIR\data"'
    Pop $1
    Pop $2
    StrCpy $2 ""
    ${If} $1 != 0
      !insertmacro InstallDataAbort "无法删除本地数据。卸载未完成。"
    ${EndIf}
    DeleteRegKey SHCTX "Software\${BUNDLEID}"
  ${EndIf}
!macroend
