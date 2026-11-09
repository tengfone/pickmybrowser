param([string]$d)
$names = if($args[1]){$args[1] -split ','}else{'router','signal','chevron','dial-dark','dial-light','dial-ring'}
$h = "<html><body style='margin:0;background:#888;font:14px sans-serif'>"
foreach ($n in $names) {
  $h += "<div style='display:flex;align-items:center;gap:16px;padding:8px'><b style='width:90px'>$n</b>"
  foreach ($s in 256, 64, 32, 16) { $h += "<img src='$n.svg' width=$s height=$s>" }
  $h += "<span style='background:#1f1f1f;padding:6px'><img src='$n.svg' width=16></span><span style='background:#f3f3f3;padding:6px'><img src='$n.svg' width=16></span></div>"
}
$h += "</body></html>"
Set-Content "$d\sheet.html" $h
$edge = "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"
$url = "file:///" + ($d -replace '\\', '/') + "/sheet.html"
$p = Start-Process $edge -ArgumentList "--headless", "--disable-gpu", "--hide-scrollbars", "--window-size=820,600", "--screenshot=$d\sheet.png", $url -PassThru -Wait
Write-Output (Test-Path "$d\sheet.png")

