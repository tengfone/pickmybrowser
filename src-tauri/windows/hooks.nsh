; NSIS installer hooks (bundle.windows.nsis.installerHooks in tauri.conf.json).
; A real uninstall (not the /UPDATE pass the updater runs) wipes everything outside $INSTDIR:
; default-browser registration, autostart entry, settings, icon cache and webview data.
; Tauri's own template then removes $INSTDIR, shortcuts and the Add/Remove Programs entry.

!macro NSIS_HOOK_PREUNINSTALL
  ${If} $UpdateMode <> 1
    ; Quit a running copy (it lives in the tray) so no files are locked.
    nsExec::Exec 'taskkill /F /T /IM "${MAINBINARYNAME}.exe"'
    Sleep 500
    nsExec::Exec '"$INSTDIR\${MAINBINARYNAME}.exe" --uninstall'
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    ; Belt and braces in case the binary could not run (e.g. already deleted).
    DeleteRegKey HKCU "Software\Classes\PickMyBrowserURL"
    DeleteRegKey HKCU "Software\Clients\StartMenuInternet\PickMyBrowser"
    DeleteRegValue HKCU "Software\RegisteredApplications" "PickMyBrowser"
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "PickMyBrowser"
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "PickMyBrowser"
    DeleteRegValue HKCU "Software\Classes\.htm\OpenWithProgids" "PickMyBrowserURL"
    DeleteRegValue HKCU "Software\Classes\.html\OpenWithProgids" "PickMyBrowserURL"
    RMDir /r "$APPDATA\com.tengfone.pickmybrowser"
    RMDir /r "$LOCALAPPDATA\com.tengfone.pickmybrowser"
  ${EndIf}
!macroend
