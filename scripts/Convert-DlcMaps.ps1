# Convert the user's own official DLC districts (SK-009) into .skate maps.
# Sources: the DLC .big packages Skate3Recomp already extracted to
# %APPDATA%\skate3\0000000000000000\454108E6\00000002\. Output goes to the installation's
# maps/ folder (outside git); the in-game map menu lists them automatically.
# Needs Python 3 with tools/requirements-setup.txt (numpy, Pillow), e.g. a venv:
#   python -m venv .local/venv; .local/venv/Scripts/python -m pip install -r tools/requirements-setup.txt
param([string]$DlcRoot = "$env:APPDATA\skate3\0000000000000000\454108E6\00000002",
      [string]$Python = '', [string]$Installation = '', [string[]]$Only = @())
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
if (-not $Python) { $Python = if (Test-Path "$workspace/.local/venv/Scripts/python.exe") { "$workspace/.local/venv/Scripts/python.exe" } else { 'python' } }
if (-not $Installation) { $Installation = (Get-ChildItem "$workspace/data/installations" -Directory | Select-Object -First 1).FullName }
$game = Join-Path $workspace 'bin/skate3rust.exe'
if (Get-Process skate3rust -ErrorAction SilentlyContinue) { throw 'Close the game first (conversion validates each map with it).' }
# Package file, district stream inside it, map name.
$districts = @(
    @('maloof_money_cup_00000000.big', 'DIST_MaloofMoneyCupDLC', 'MaloofMoneyCupDLC'),
    @('play_00000000.big', 'Dist_Zen', 'Zen'),
    @('play_00000000.big', 'DIST_AGPark', 'AGPark'),
    @('player1p_00000000.big', 'DIST_SanitariumDLC', 'SanitariumDLC'),
    @('creator_00000000.big', 'DIST_BackLotParkDLC', 'BackLotParkDLC'),
    @('creator_00000000.big', 'DIST_DownTownSkateParkNightDLC', 'DownTownSkateParkNightDLC'),
    @('dway_park_00000000.big', 'DLC_DW_MegaCompund', 'DWMegaCompound')
)
$work = Join-Path $Installation 'conversion'
New-Item -ItemType Directory -Force $work | Out-Null
try {
    foreach ($d in $districts) {
        if ($Only.Count -and $d[2] -notin $Only) { continue }
        $package = Get-ChildItem $DlcRoot -Recurse -Filter $d[0] -ErrorAction SilentlyContinue | Select-Object -First 1
        if (-not $package) { Write-Warning "$($d[2]): $($d[0]) not found"; continue }
        $result = Join-Path $work "$($d[2]).json"
        & $Python (Join-Path $workspace 'tools/asset_pipeline/map_job.py') --archive $package.FullName --stage $Installation `
            --game-exe $game --result $result --district $d[1] --label $d[2] 2>&1 | Out-Null
        if (Test-Path $result) { Write-Host "OK   $($d[2])" } else { Write-Warning "FAIL $($d[2]): see $Installation\$($package.BaseName)-load.log" }
    }
} finally {
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}
