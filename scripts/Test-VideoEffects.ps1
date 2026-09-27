# SK-051..055: screenshots and FPS of the optional video effects, one game per effect setting.
# Each -Effects entry is a SKATE_VIDEO_FX value (e.g. 'bloom=0', 'bloom=3', 'ssao=2'); it
# overrides settings/graphics.json without writing it. Screenshots: logs/test-shots/fx-<name>.png.
# -Env adds variables (e.g. @{SKATE_SSAO_DEBUG='1'}), -Suffix tells the shots apart.
# -Perf adds a second run per entry with SKATE_PERF_REPORT (10-25 s after start, uncapped FPS).
# Example: scripts/Test-VideoEffects.ps1 -Effects 'bloom=0','bloom=3' -Map maps/private/University.skate -Perf -Stage
param([string[]]$Effects = @('bloom=0', 'bloom=3'), [string]$Map, [switch]$Perf, [switch]$Stage, [int]$Repeats = 1, [hashtable]$Env = @{}, [string]$Suffix = '')
Import-Module (Join-Path $PSScriptRoot 'GameTest.psm1') -Force
$Root = Split-Path $PSScriptRoot -Parent
$first = $true
$rows = foreach ($fx in $Effects) {
    $name = 'fx-' + ($fx -replace '[=,]', '-') + $Suffix
    # Start from all effects off, so effects saved in graphics.json do not leak into the A/B.
    $spec = "bloom=0,ssao=0,ssr=0,volumetric=0,$fx"
    $common = @{ Owner = 'video-fx'; Map = $Map }
    if (-not $Map) { $common.Remove('Map') }
    $game = Start-TestGame @common -Stage:($Stage -and $first) -Env (@{ SKATE_VIDEO_FX = $spec; SKATE_FPS_LIMIT = '0' } + $Env)
    $first = $false
    try {
        Start-Sleep 3
        $shot = Save-GameScreenshot $game $name
        $issues = @(Get-GameIssues $game)
    } finally { Stop-TestGame $game }
    $fps = @()
    if ($Perf) {
        foreach ($run in 1..$Repeats) {
            $report = Join-Path $Root "logs\test-shots\$name-perf$run.json"
            Remove-Item $report -ErrorAction SilentlyContinue
            $game = Start-TestGame @common -Env (@{ SKATE_VIDEO_FX = $spec; SKATE_FPS_LIMIT = '0'; SKATE_PERF_REPORT = $report } + $Env)
            try { $game.Process.WaitForExit(90000) | Out-Null } finally { Stop-TestGame $game }
            if (Test-Path $report) { $fps += [math]::Round((Get-Content -Raw $report | ConvertFrom-Json).fps, 1) }
        }
    }
    [pscustomobject]@{ Effects = $fx; Shot = $shot; FPS = ($fps -join ' / '); Issues = ($issues | Select-Object -First 2) -join ' | ' }
}
$rows | Format-Table -AutoSize -Wrap | Out-String -Width 220
