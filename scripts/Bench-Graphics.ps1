# Graphics A/B benchmark (SK-018): runs bin/skate3rust.exe once per scenario with
# SKATE_PERF_REPORT and prints one summary line each. The game must be closed.
# Example: scripts/Bench-Graphics.ps1 -Scales 100,200 -Msaa 1,4,8 -Repeats 1 [-Sweep]
param([string[]]$Scales = @('100', '200'), [string[]]$Msaa = @('1', '4', '8'), [int]$Repeats = 1, [switch]$Sweep, [string]$Map = '')
$ErrorActionPreference = 'Stop'
# "-Scales 100,200" arrives as one string through powershell -File.
$Scales = @($Scales -split ',' | ForEach-Object { [int]$_ })
$Msaa = @($Msaa -split ',' | ForEach-Object { [int]$_ })
$workspace = Split-Path -Parent $PSScriptRoot
$binary = Join-Path $workspace 'bin/skate3rust.exe'
if (Get-Process skate3rust -ErrorAction SilentlyContinue) { throw 'Close the game first.' }
$out = Join-Path $workspace ('logs/bench/' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
New-Item -ItemType Directory -Path $out -Force | Out-Null
$arguments = '--assets "' + (Join-Path $workspace 'assets') + '"'
if ($Map) { $arguments += ' --map "' + (Resolve-Path -LiteralPath $Map).Path + '"' }
$env:SKATE_FPS_LIMIT = '0'
if ($Sweep) { $env:SKATE_PERF_CAMERA_SWEEP = '1' } else { Remove-Item Env:SKATE_PERF_CAMERA_SWEEP -ErrorAction SilentlyContinue }
$rows = foreach ($scale in $Scales) { foreach ($samples in $Msaa) { foreach ($run in 1..$Repeats) {
    $name = "scale$scale-msaa$samples-run$run"
    $env:SKATE_RENDER_SCALE = [string]$scale
    $env:SKATE_MSAA = [string]$samples
    $env:SKATE_PERF_REPORT = Join-Path $out "$name.json"
    $process = Start-Process -FilePath $binary -ArgumentList $arguments -WorkingDirectory $workspace -PassThru `
        -RedirectStandardError (Join-Path $out "$name.err.log") -RedirectStandardOutput (Join-Path $out "$name.out.log")
    if (-not $process.WaitForExit(240000)) { $process.Kill(); Write-Warning "$name timed out"; continue }
    $r = Get-Content -Raw $env:SKATE_PERF_REPORT | ConvertFrom-Json
    [pscustomobject]@{
        Scenario = $name; FPS = [math]::Round($r.fps, 1); Mean = [math]::Round($r.frame_ms_mean, 2)
        P95 = [math]::Round($r.frame_ms_p95, 2); P99 = [math]::Round($r.frame_ms_p99, 2)
        Main = [math]::Round($r.main_schedule_ms_mean, 2); Submit = [math]::Round($r.render_submit_ms_mean, 2)
        Prepare = [math]::Round($r.render_prepare_ms_mean, 2)
    }
} } }
foreach ($key in 'SKATE_FPS_LIMIT', 'SKATE_RENDER_SCALE', 'SKATE_MSAA', 'SKATE_PERF_REPORT', 'SKATE_PERF_CAMERA_SWEEP') {
    Remove-Item "Env:$key" -ErrorAction SilentlyContinue
}
$rows | Format-Table -AutoSize | Out-String -Width 200
if ($Repeats -gt 1) {
    # Mean and standard deviation of FPS per scale/MSAA pair.
    $rows | Group-Object { $_.Scenario -replace '-run\d+$', '' } | ForEach-Object {
        $fps = @($_.Group | ForEach-Object FPS)
        $mean = ($fps | Measure-Object -Average).Average
        $sd = [math]::Sqrt((($fps | ForEach-Object { ($_ - $mean) * ($_ - $mean) }) | Measure-Object -Sum).Sum / [math]::Max(1, $fps.Count - 1))
        [pscustomobject]@{ Scenario = $_.Name; MeanFPS = [math]::Round($mean, 1); SdFPS = [math]::Round($sd, 1)
            MainMs = [math]::Round(($_.Group | Measure-Object Main -Average).Average, 2) }
    } | Format-Table -AutoSize | Out-String -Width 200
}
$rows | Export-Csv -NoTypeInformation (Join-Path $out 'summary.csv')
Write-Host "Reports: $out"
