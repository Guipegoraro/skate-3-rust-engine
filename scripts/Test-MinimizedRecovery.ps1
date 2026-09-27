# SK-037 regression: a physics failure while minimised must recover once the window is back,
# without a renderer panic. Forces a failure (SKATE_FORCE_PHYSICS_FAILURE) during the minimise.
param([switch]$Stage, [switch]$KeepOpen)
Import-Module (Join-Path $PSScriptRoot 'GameTest.psm1') -Force
$game = Start-TestGame -TestWorld -Stage:$Stage -Env @{ SKATE_FORCE_PHYSICS_FAILURE = '25' }
try {
    Set-GameWindow $game Minimize
    Start-Sleep 20
    Set-GameWindow $game Restore
    Start-Sleep 20
    $issues = @(Get-GameIssues $game)
    $restored = $issues -match 'restored to'
    $panics = $issues -match 'REPORT_PANIC'
    $alive = Test-GameAlive $game
    $shot = if ($alive) { Save-GameScreenshot $game 'minimized-recovery' }
    "alive=$alive restored=$([bool]$restored) panics=$($panics.Count) screenshot=$shot log=$($game.ErrorLog)"
    $issues | Select-Object -First 5
    if (-not $alive -or -not $restored -or $panics) { exit 1 }
} catch { "test error: $_"; Get-GameIssues $game | Select-Object -First 5; exit 1
} finally { if (-not $KeepOpen) { Stop-TestGame $game } }
