# Plays the game while holding the live-test lock (logs/game-test.lock), so agents and other
# sessions using scripts/GameTest.psm1 wait instead of closing the player's game.
# Waits for a running test to finish first; keeps the lock fresh until the game exits.
# The window and the lock are named "Skate 3 users session" so the player's game is easy to
# tell apart from agents' test windows.
param([string]$Map, [int]$Port = 15703, [string]$Title = 'Skate 3 users session')
$Root = Split-Path $PSScriptRoot -Parent
$Lock = Join-Path $Root 'logs\game-test.lock'
New-Item -ItemType Directory -Force (Split-Path $Lock) | Out-Null
while ($true) {
    if ((Test-Path $Lock) -and ((Get-Date) - (Get-Item $Lock).LastWriteTime).TotalMinutes -gt 15) { Remove-Item $Lock -ErrorAction SilentlyContinue }
    try {
        $stream = [IO.File]::Open($Lock, 'CreateNew', 'Write')
        $bytes = [Text.Encoding]::UTF8.GetBytes("$Title pid=$PID $(Get-Date -Format s)")
        $stream.Write($bytes, 0, $bytes.Length); $stream.Close()
        break
    } catch [IO.IOException] { Write-Host "Waiting for a running test: $(Get-Content $Lock -ErrorAction SilentlyContinue)"; Start-Sleep 5 }
}
try {
    # iw4L often owns 15702/15703: take the first free BRP port.
    while (Get-NetTCPConnection -State Listen -LocalPort $Port -ErrorAction SilentlyContinue) { $Port++ }
    $env:BRP_EXTRAS_PORT = "$Port"
    $env:SKATE3_MODS = Join-Path $Root 'mods'
    $arguments = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', (Join-Path $Root 'scripts\Launch.ps1'))
    if ($Map) { $arguments += @('-Map', $Map) }
    $launcher = Start-Process powershell -ArgumentList $arguments -WorkingDirectory $Root -PassThru
    $titleTries = 60
    $lastTouch = [datetime]::MinValue
    while (-not $launcher.HasExited) {
        if (((Get-Date) - $lastTouch).TotalSeconds -ge 30) {
            (Get-Item $Lock).LastWriteTime = Get-Date   # stays fresh (stale after 15 min)
            $lastTouch = Get-Date
        }
        if ($titleTries -gt 0) {
            # Rename the window through BRP once the game answers (about 2 minutes of tries).
            $titleTries--
            $body = @{ jsonrpc = '2.0'; id = 1; method = 'brp_extras/set_window_title'; params = @{ title = $Title } } | ConvertTo-Json
            try {
                Invoke-RestMethod -Uri "http://127.0.0.1:$Port/jsonrpc" -Method Post -Body $body -ContentType 'application/json' -TimeoutSec 3 | Out-Null
                $titleTries = 0
            } catch {}
        }
        Start-Sleep 2
    }
} finally { Remove-Item $Lock -ErrorAction SilentlyContinue }
