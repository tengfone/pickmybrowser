param([string]$Exe, [string[]]$AppArgs = @())
# Lists the top-level windows of a freshly started process (title + visibility).
Add-Type @"
using System; using System.Text; using System.Runtime.InteropServices;
public class W {
  public delegate bool P(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(P p, IntPtr l);
  [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
}
"@
Get-Process pickmybrowser -EA 0 | Stop-Process -Force
$p = if ($AppArgs.Count) { Start-Process $Exe -ArgumentList $AppArgs -PassThru } else { Start-Process $Exe -PassThru }
Start-Sleep 4
$script:out = @()
[W]::EnumWindows({ param($h, $l)
  $id = 0; [void][W]::GetWindowThreadProcessId($h, [ref]$id)
  if ($id -eq $p.Id) {
    $sb = New-Object Text.StringBuilder 256; [void][W]::GetWindowText($h, $sb, 256)
    if ($sb.Length) { $script:out += "'$sb' visible=$([W]::IsWindowVisible($h))" }
  }
  $true }, [IntPtr]::Zero) | Out-Null
$script:out
Stop-Process $p -Force
