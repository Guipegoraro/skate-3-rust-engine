# SK-032: run Skate3Recomp with the XMA-tracing ReXGlue runtime, in a separate folder so the
# user's install is untouched. The trace (one line per XMA buffer the game decodes) is then
# matched to our sound ids with tools/audio_trace_match.py.
# -Automate: windowed, the recomp's own demo path boots straight to gameplay, and the pad is
# driven from logs/pad-script.txt (REX_PAD_SCRIPT, see tools/recomp_trace/README.md), so no
# controller or player is needed. Returns the process and trace path.
param(
    [string]$Install = 'C:\Users\guipegoraro\Desktop\virussafe\Skate3Recomp-Windows',
    [string]$Runtime = "$PSScriptRoot\..\..\rexglue-audiotrace\out\win-amd64\rexruntime.dll",
    [string]$RunDir = "$PSScriptRoot\..\..\s3r-trace-run",
    [switch]$Automate,
    [string[]]$ExtraArgs = @()
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
$gameArgs = @($ExtraArgs)
$env:REX_XMA_TRACE = $trace
if ($Automate) {
    $script = Join-Path $workspace 'logs/pad-script.txt'
    Set-Content $script '' -NoNewline
    $env:REX_PAD_SCRIPT = $script
    $gameArgs = @('--fullscreen=false', '--mnk_mode=true', '--skate3_demo_path=true') + $gameArgs
}
$splat = @{ FilePath = "$RunDir\skate3.exe"; WorkingDirectory = $RunDir; PassThru = $true }
if ($gameArgs) { $splat.ArgumentList = $gameArgs }
$process = Start-Process @splat
Remove-Item Env:REX_XMA_TRACE, Env:REX_PAD_SCRIPT -ErrorAction SilentlyContinue
Write-Host "Trace: $trace"
if ($Automate) { return [pscustomobject]@{ Process = $process; Trace = $trace; PadScript = $script } }
Write-Host "Play (ollie, land, grind, bail), close the game, then run:"
Write-Host "  python tools/audio_trace_match.py `"$trace`" `"$Install\game`""
