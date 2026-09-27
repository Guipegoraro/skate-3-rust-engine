# Plays the game while holding the live-test lock (logs/game-test.lock), so agents and other
# sessions using scripts/GameTest.psm1 wait instead of closing the player's game.
# Waits for a running test to finish first; keeps the lock fresh until the game exits.
param([string]$Map, [int]$Port = 15703)
$Root = Split-Path $PSScriptRoot -Parent
$Lock = Join-Path $Root 'logs\game-test.lock'
New-Item -ItemType Directory -Force (Split-Path $Lock) | Out-Null
while ($true) {
    if ((Test-Path $Lock) -and ((Get-Date) - (Get-Item $Lock).LastWriteTime).TotalMinutes -gt 15) { Remove-Item $Lock -ErrorAction SilentlyContinue }
    try {
        $stream = [IO.File]::Open($Lock, 'CreateNew', 'Write')
        $bytes = [Text.Encoding]::UTF8.GetBytes("user pid=$PID $(Get-Date -Format s)")
        $stream.Write($bytes, 0, $bytes.Length); $stream.Close()
        break
    } catch [IO.IOException] { Write-Host "Waiting for a running test: $(Get-Content $Lock -ErrorAction SilentlyContinue)"; Start-Sleep 5 }
}
try {
    $env:BRP_EXTRAS_PORT = "$Port"
    $env:SKATE3_MODS = Join-Path $Root 'mods'
    $arguments = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', (Join-Path $Root 'scripts\Launch.ps1'))
    if ($Map) { $arguments += @('-Map', $Map) }
    $launcher = Start-Process powershell -ArgumentList $arguments -WorkingDirectory $Root -PassThru
    while (-not $launcher.HasExited) {
        (Get-Item $Lock).LastWriteTime = Get-Date   # stays fresh (stale after 15 min)
        Start-Sleep 30
    }
} finally { Remove-Item $Lock -ErrorAction SilentlyContinue }
