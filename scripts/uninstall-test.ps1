# End-to-end check that the Windows NSIS uninstaller leaves nothing behind.
# Installs the built setup silently, seeds every kind of state the app creates, uninstalls,
# then lists any remnant. Backs up and restores the developer's own PickMyBrowser state.
param([string]$Setup = (Get-ChildItem "$PSScriptRoot\..\src-tauri\target\release\bundle\nsis\*-setup.exe" | Select-Object -First 1).FullName)
$ErrorActionPreference = 'Continue'
$id = 'com.tengfone.pickmybrowser'
$bak = Join-Path $env:TEMP "pmb-uninstall-test-backup"
Remove-Item $bak -Recurse -Force -EA 0; New-Item $bak -ItemType Directory | Out-Null

$keys = @('HKCU\Software\Classes\PickMyBrowserURL', 'HKCU\Software\Clients\StartMenuInternet\PickMyBrowser')
$dirs = @("$env:APPDATA\$id", "$env:LOCALAPPDATA\$id")
$instDir = "$env:LOCALAPPDATA\PickMyBrowser"

# ---- backup
$i = 0; foreach ($k in $keys) { reg export $k "$bak\k$i.reg" /y 2>$null | Out-Null; $i++ }
$regApp = (Get-ItemProperty 'HKCU:\Software\RegisteredApplications' -EA 0).PickMyBrowser
for ($j = 0; $j -lt $dirs.Count; $j++) { if (Test-Path $dirs[$j]) { Copy-Item $dirs[$j] "$bak\$(Split-Path $dirs[$j] -Leaf)-$j" -Recurse } }
Get-Process pickmybrowser -EA 0 | Stop-Process -Force; Start-Sleep 1

# ---- install
Write-Output "Installing $Setup"
Start-Process $Setup -ArgumentList '/S' -Wait
$exe = "$instDir\pickmybrowser.exe"
Write-Output ("installed exe: " + (Test-Path $exe))
function Get-UninstallEntry {
  Get-ChildItem 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall' | Where-Object { (Get-ItemProperty $_.PSPath).DisplayName -eq 'PickMyBrowser' }
}
$uninstKey = Get-UninstallEntry
Write-Output ("Add/Remove entry: " + [bool]$uninstKey)

# ---- seed state: run app (creates config + webview data), then register + autostart like the app does
$p = Start-Process $exe -ArgumentList '--background' -PassThru; Start-Sleep 5
reg add 'HKCU\Software\Classes\PickMyBrowserURL\shell\open\command' /ve /d "`"$exe`" `"%1`"" /f | Out-Null
reg add 'HKCU\Software\Clients\StartMenuInternet\PickMyBrowser\Capabilities\URLAssociations' /v https /d PickMyBrowserURL /f | Out-Null
reg add 'HKCU\Software\RegisteredApplications' /v PickMyBrowser /d 'Software\Clients\StartMenuInternet\PickMyBrowser\Capabilities' /f | Out-Null
reg add 'HKCU\Software\Microsoft\Windows\CurrentVersion\Run' /v PickMyBrowser /d "`"$exe`" --background" /f | Out-Null
reg add 'HKCU\Software\Microsoft\Windows\CurrentVersion\ApplicationAssociationToasts' /v PickMyBrowserURL_https /t REG_DWORD /d 0 /f | Out-Null
reg add 'HKCU\Software\Classes\.html\OpenWithProgids' /v PickMyBrowserURL /t REG_NONE /f | Out-Null
foreach ($d in $dirs) { Write-Output ("seeded {0}: {1}" -f $d, (Test-Path $d)) }

# ---- uninstall (app still running in tray, as a real user would have it)
$un = "$instDir\uninstall.exe"
Write-Output "Uninstalling"
Start-Process $un -ArgumentList '/S' -Wait
Start-Sleep 4  # NSIS uninstaller re-spawns itself from %TEMP%

# ---- verify
$left = @()
if (Test-Path $instDir) { $left += "dir $instDir" }
foreach ($d in $dirs) { if (Test-Path $d) { $left += "dir $d" } }
foreach ($k in $keys) { reg query $k 2>$null | Out-Null; if ($LASTEXITCODE -eq 0) { $left += "key $k" } }
$vals = @(
  @('HKCU:\Software\RegisteredApplications', 'PickMyBrowser'),
  @('HKCU:\Software\Microsoft\Windows\CurrentVersion\Run', 'PickMyBrowser'),
  @('HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run', 'PickMyBrowser'),
  @('HKCU:\Software\Microsoft\Windows\CurrentVersion\ApplicationAssociationToasts', 'PickMyBrowserURL_https'),
  @('HKCU:\Software\Classes\.html\OpenWithProgids', 'PickMyBrowserURL'))
foreach ($v in $vals) { if ($null -ne (Get-ItemProperty $v[0] -Name $v[1] -EA 0)) { $left += "value $($v[0])\$($v[1])" } }
if (Get-UninstallEntry) { $left += 'Add/Remove entry' }
if (Get-ChildItem "$env:APPDATA\Microsoft\Windows\Start Menu\Programs" -Recurse -Filter 'PickMyBrowser*.lnk' -EA 0) { $left += 'Start menu shortcut' }
if (Get-ChildItem ([Environment]::GetFolderPath('Desktop')) -Filter 'PickMyBrowser*.lnk' -EA 0) { $left += 'Desktop shortcut' }
if (Get-Process pickmybrowser -EA 0 | Where-Object Path -like "$instDir*") { $left += 'process still running' }
Get-ChildItem 'HKCU:\Control Panel\NotifyIconSettings' -EA 0 | Where-Object { (Get-ItemProperty $_.PSPath).ExecutablePath -eq $exe } | ForEach-Object { $left += "tray icon memory $($_.PSChildName)" }
$hkcuHits = reg query HKCU /f PickMyBrowser /s 2>$null | Select-String '^HKEY|PickMyBrowser' | Where-Object { $_ -notmatch 'MuiCache|UserAssist|Compatibility Assistant|bam\\' }

# ---- restore developer state
$i = 0; foreach ($k in $keys) { if (Test-Path "$bak\k$i.reg") { reg import "$bak\k$i.reg" 2>$null }; $i++ }
if ($regApp) { reg add 'HKCU\Software\RegisteredApplications' /v PickMyBrowser /d $regApp /f | Out-Null }
for ($j = 0; $j -lt $dirs.Count; $j++) { $b = "$bak\$(Split-Path $dirs[$j] -Leaf)-$j"; if (Test-Path $b) { Copy-Item $b $dirs[$j] -Recurse -Force } }

Write-Output '---- remnants'
if ($left.Count) { $left | ForEach-Object { "FAIL  $_" } } else { 'PASS  nothing left behind' }
Write-Output '---- full HKCU search for "PickMyBrowser" (after restore; excludes Windows shell history)'
$hkcuHits | Select-Object -First 20
exit $left.Count
