# SK-032: run Skate3Recomp with the XMA-tracing ReXGlue runtime, in a separate folder so the
# user's install is untouched. The trace (one line per XMA buffer the game decodes) is then
# matched to our sound ids with tools/audio_trace_match.py.
param(
    [string]$Install = 'C:\Users\guipegoraro\Desktop\virussafe\Skate3Recomp-Windows',
    [string]$Runtime = "$PSScriptRoot\..\..\rexglue-audiotrace\out\win-amd64\rexruntime.dll",
    [string]$RunDir = "$PSScriptRoot\..\..\s3r-trace-run"
)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
if (-not (Test-Path $Runtime)) {
    $found = Get-ChildItem (Split-Path (Split-Path $Runtime)) -Recurse -Filter rexruntime.dll | Select-Object -First 1
    if (-not $found) { throw "Build the patched runtime first (rexruntime.dll not found)." }
    $Runtime = $found.FullName
}
New-Item -ItemType Directory -Force $RunDir | Out-Null
Copy-Item "$Install\skate3.exe" $RunDir -Force
Copy-Item $Runtime "$RunDir\rexruntime.dll" -Force
foreach ($folder in 'game', 'dlc') {
    if ((Test-Path "$Install\$folder") -and -not (Test-Path "$RunDir\$folder")) {
        New-Item -ItemType Junction -Path "$RunDir\$folder" -Target "$Install\$folder" | Out-Null
    }
}
$trace = Join-Path $workspace ('logs/xma-trace-' + (Get-Date -Format 'yyyyMMdd-HHmmss') + '.log')
$env:REX_XMA_TRACE = $trace
Start-Process -FilePath "$RunDir\skate3.exe" -WorkingDirectory $RunDir
Remove-Item Env:REX_XMA_TRACE
Write-Host "Trace: $trace"
Write-Host "Play (ollie, land, grind, bail), close the game, then run:"
Write-Host "  python tools/audio_trace_match.py `"$trace`" `"$Install\game`""
