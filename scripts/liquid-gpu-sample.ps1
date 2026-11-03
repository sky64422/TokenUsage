param([Parameter(Mandatory=$true)][string]$ProcessIds)
$ErrorActionPreference = 'Stop'
$taskIds = @($ProcessIds.Split(',') | ForEach-Object { [int]$_ })
$taskDwmIds = @(Get-Process dwm | Select-Object -ExpandProperty Id)
$taskPattern = '^pid_(' + (($taskIds + $taskDwmIds) -join '|') + ')_'
$taskCounterClass = Get-CimClass Win32_PerfRawData_GPUPerformanceCounters_GPUEngine
$taskType = ($taskCounterClass.CimClassProperties['UtilizationPercentage'].Qualifiers | Where-Object Name -eq 'CounterType').Value
if ($taskType -ne 542180608) { throw 'Unexpected GPU counter type; refusing timer interpretation.' }
$taskBefore = @{}
Get-CimInstance Win32_PerfRawData_GPUPerformanceCounters_GPUEngine | Where-Object Name -match $taskPattern | ForEach-Object { $taskBefore[$_.Name] = $_ }
Start-Sleep -Seconds 5
$taskAfter = @(Get-CimInstance Win32_PerfRawData_GPUPerformanceCounters_GPUEngine | Where-Object Name -match $taskPattern)
$taskRows = @()
foreach ($taskRow in $taskAfter) {
    $taskPrevious = $taskBefore[$taskRow.Name]
    if ($null -eq $taskPrevious) { continue }
    $taskElapsed = [decimal]$taskRow.Timestamp_Sys100NS - [decimal]$taskPrevious.Timestamp_Sys100NS
    $taskDelta = [decimal]$taskRow.UtilizationPercentage - [decimal]$taskPrevious.UtilizationPercentage
    if ($taskElapsed -le 0 -or $taskDelta -lt 0) { continue }
    $taskPid = [int]([regex]::Match($taskRow.Name, '^pid_(\d+)_').Groups[1].Value)
    $taskRows += [pscustomobject]@{name=$taskRow.Name; scope=$(if($taskPid -in $taskDwmIds){'dwm'}else{'app'});
        percent=[double](100 * $taskDelta / $taskElapsed); elapsed_s=[double]($taskElapsed / 10000000)}
}
# Engine percentages can overlap; their sum is NOT whole-GPU utilization.
[pscustomobject]@{counter_type=$taskType; before_rows=$taskBefore.Count; after_rows=$taskAfter.Count;
    matched_rows=$taskRows.Count; engines=$taskRows} | ConvertTo-Json -Depth 4 -Compress
