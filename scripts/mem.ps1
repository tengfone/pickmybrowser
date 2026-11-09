param([int]$root)
$procs = Get-CimInstance Win32_Process
$all = [System.Collections.Generic.List[int]]::new(); $all.Add($root)
$q = @($root)
while ($q.Count) {
  $n = @($procs | Where-Object { $q -contains $_.ParentProcessId } | ForEach-Object { [int]$_.ProcessId } | Where-Object { -not $all.Contains($_) })
  $n | ForEach-Object { $all.Add($_) }
  $q = $n
}
$ps = Get-Process -Id $all -EA 0
$ps | Select-Object Name, Id, @{n='WS_MB';e={[math]::Round($_.WorkingSet64/1MB,1)}}, @{n='Priv_MB';e={[math]::Round($_.PrivateMemorySize64/1MB,1)}} | Format-Table -AutoSize | Out-String | Write-Output
Write-Output ("TOTAL WS {0:N0} MB | Private {1:N0} MB | procs {2}" -f (($ps|Measure-Object WorkingSet64 -Sum).Sum/1MB), (($ps|Measure-Object PrivateMemorySize64 -Sum).Sum/1MB), $ps.Count)
