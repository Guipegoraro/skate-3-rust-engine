# SK-037 regression: minimising the window must not fail the physics or panic the renderer.
# Starts the test world, minimises for -Seconds, restores, and exits 1 on any issue.
param([int]$Seconds = 15, [switch]$Stage, [switch]$KeepOpen)
Import-Module (Join-Path $PSScriptRoot 'GameTest.psm1') -Force
$game = Start-TestGame -TestWorld -Stage:$Stage
try {
    $before = Get-GameWindowSize $game
    Set-GameWindow $game Minimize
    Start-Sleep $Seconds
    Set-GameWindow $game Restore
    Start-Sleep 5
    $shot = if (Test-GameAlive $game) { Save-GameScreenshot $game 'minimize-restored' }
    $issues = @(Get-GameIssues $game)
    $alive = Test-GameAlive $game
    # The window must come back at its size (it used to restore as 0x0 and render 1x1).
    $after = if ($alive) { Get-GameWindowSize $game }
    $sized = $after -and $after.Windows -eq $before.Windows -and $after.Bevy -eq $before.Bevy
    "alive=$alive issues=$($issues.Count) size=$($before.Bevy)->$($after.Bevy) (windows $($after.Windows)) screenshot=$shot"
    $issues | Select-Object -First 5
    if (-not $alive -or $issues -or -not $sized) { exit 1 }
} catch { "test error: $_"; Get-GameIssues $game | Select-Object -First 5; exit 1
} finally { if (-not $KeepOpen) { Stop-TestGame $game } }
