# Live game test helpers (see docs/testing-notes.md, "Live game tests").
#   Import-Module .\scripts\GameTest.psm1 -Force
#   $g = Start-TestGame -TestWorld -Mods @{'guipegoraro.skater-size' = @{size = 2}}
#   Save-GameScreenshot $g 'size-2'; Get-GameIssues $g; Stop-TestGame $g
# Start-TestGame runs bin/skate3rust.exe directly (stage first with -Stage), logs to
# logs/test-<time>.log/.stderr.log, and backs up every settings file it changes;
# Stop-TestGame puts them back.
$Root = Split-Path $PSScriptRoot -Parent
$Shots = Join-Path $Root 'logs\test-shots'
$Library = Join-Path $env:LOCALAPPDATA 'Skate3RustEngine\custom-characters'

Add-Type -ErrorAction SilentlyContinue @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class GameTestWin {
    delegate bool EnumProc(IntPtr hWnd, IntPtr lParam);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc callback, IntPtr lParam);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr hWnd, System.Text.StringBuilder text, int count);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr hWnd);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr hWnd, out RECT rect);
    // The game window among the processes' top-level windows, by title: each process also owns
    // "visible" helpers (the supervisor's PseudoConsoleWindow, winit's event target window).
    public static IntPtr Find(uint[] pids, string title) {
        var wanted = new HashSet<uint>(pids);
        IntPtr found = IntPtr.Zero;
        EnumWindows((h, l) => {
            uint pid; GetWindowThreadProcessId(h, out pid);
            var text = new System.Text.StringBuilder(256);
            GetWindowText(h, text, 256);
            if (wanted.Contains(pid) && IsWindowVisible(h) && text.ToString() == title) { found = h; return false; }
            return true;
        }, IntPtr.Zero);
        return found;
    }
}
'@

function Get-Installation { (Get-ChildItem (Join-Path $Root 'data\installations') -Directory | Select-Object -First 1).FullName }

# UTF-8 without BOM on both Windows PowerShell 5 and PowerShell 7 (5 has no utf8NoBOM).
function Write-Utf8([string]$path) { [IO.File]::WriteAllText($path, ($input | Out-String)) }

function Backup-File($game, [string]$path) {
    if ($game.Backups.ContainsKey($path)) { return }
    $game.Backups[$path] = if (Test-Path $path) { [IO.File]::ReadAllText($path) } else { $null }
}

# One live test at a time across sessions and agents: logs/game-test.lock holds the owner.
# A lock older than 15 minutes is stale (its test died without Stop-TestGame).
$Lock = Join-Path $Root 'logs\game-test.lock'
function Enter-TestLock([string]$owner, [int]$timeoutSeconds) {
    New-Item -ItemType Directory -Force (Split-Path $Lock) | Out-Null
    $deadline = (Get-Date).AddSeconds($timeoutSeconds)
    while ($true) {
        if ((Test-Path $Lock) -and ((Get-Date) - (Get-Item $Lock).LastWriteTime).TotalMinutes -gt 15) {
            Remove-Item $Lock -ErrorAction SilentlyContinue
        }
        try {
            $stream = [IO.File]::Open($Lock, 'CreateNew', 'Write')
            $bytes = [Text.Encoding]::UTF8.GetBytes("$owner pid=$PID $(Get-Date -Format s)")
            $stream.Write($bytes, 0, $bytes.Length); $stream.Close()
            return
        } catch [IO.IOException] {
            if ((Get-Date) -gt $deadline) { throw "Another live test holds $Lock ($(Get-Content $Lock -ErrorAction SilentlyContinue))" }
            Start-Sleep 5
        }
    }
}

function Test-PortFree([int]$port) {
    -not (Get-NetTCPConnection -State Listen -LocalPort $port -ErrorAction SilentlyContinue)
}

<#
.SYNOPSIS Starts the game for a test and waits until BRP answers.
-TestWorld   small procedural map, loads fastest (default when no -Map).
-Map         a .skate path.
-Mods        @{ '<mod id>' = @{ setting = value } } enables those mods with those values.
-Model       custom model library id to select, or 'stock'.
-Env         extra environment variables, e.g. @{ SKATE_FORCE_PHYSICS_FAILURE = '20' }.
-Stage       stage target/ into bin/ first (scripts/Build.ps1 -StageOnly).
#>
function Start-TestGame {
    param([switch]$TestWorld, [string]$Map, [hashtable]$Mods = @{}, [string]$Model,
        [hashtable]$Env = @{}, [int]$Port = 15705, [switch]$Stage, [int]$TimeoutSeconds = 120,
        [string]$Owner = 'test', [int]$LockTimeoutSeconds = 900)
    # Waits for any other session's test to finish instead of killing its game.
    Enter-TestLock $Owner $LockTimeoutSeconds
    try {
        # The game may leave its Steam relay running; it holds bin/steam-relay files.
        Get-Process skate3rust, skate-steam-relay -ErrorAction SilentlyContinue | Stop-Process -Force
        Start-Sleep 2
        if ($Stage) {
            # Staging fails when another session's game still holds bin/ DLLs; never test a stale exe.
            for ($try = 1; $try -le 5; $try++) {
                $output = & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $Root 'scripts\Build.ps1') -StageOnly 2>&1
                if ($LASTEXITCODE -eq 0) { break }
                if ($try -eq 5) { throw "Staging into bin/ failed: $(($output | Select-Object -Last 3) -join ' | ')" }
                Start-Sleep 10
            }
        }
        while (-not (Test-PortFree $Port)) { $Port++ }   # another Bevy app (iw4L) may own it
        $game = [pscustomobject]@{ Process = $null; Port = $Port; Log = $null; ErrorLog = $null; Backups = @{} }
        # Tests can change settings through the menus; always put the player's back afterwards.
        foreach ($name in 'graphics.json', 'game-options.json') {
            Backup-File $game (Join-Path (Get-Installation) "settings\$name")
        }
        $settings = Join-Path (Get-Installation) 'settings\mods'
        foreach ($id in $Mods.Keys) {
            $file = Join-Path $settings "$id.json"
            Backup-File $game $file
            @{ enabled = $true; values = $Mods[$id] } | ConvertTo-Json -Depth 5 | Write-Utf8 $file
        }
        if ($Model) {
            $file = Join-Path $Library 'selection.json'
            Backup-File $game $file
            $selected = if ($Model -eq 'stock') { $null } else { $Model }
            @{ version = 1; selected = $selected } | ConvertTo-Json | Write-Utf8 $file
        }
        $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
        $game.Log = Join-Path $Root "logs\test-$stamp.log"
        $game.ErrorLog = Join-Path $Root "logs\test-$stamp.stderr.log"
        $arguments = @('--assets', ('"' + (Join-Path $Root 'assets') + '"'))
        if ($Map) { $arguments += @('--map', ('"' + (Resolve-Path $Map).Path + '"')) } else { $arguments += '--test-world' }
        $vars = @{ BRP_EXTRAS_PORT = "$Port"; SKATE3_MODS = (Join-Path $Root 'mods') } + $Env
        foreach ($k in $vars.Keys) { Set-Item "Env:$k" $vars[$k] }
        try {
            $game.Process = Start-Process -FilePath (Join-Path $Root 'bin\skate3rust.exe') -WorkingDirectory $Root `
                -ArgumentList $arguments -PassThru -RedirectStandardOutput $game.Log -RedirectStandardError $game.ErrorLog
        } finally { foreach ($k in $vars.Keys) { Remove-Item "Env:$k" -ErrorAction SilentlyContinue } }
        $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
        while ((Get-Date) -lt $deadline) {
            if ($game.Process.HasExited) { throw "Game exited with code $($game.Process.ExitCode); see $($game.ErrorLog)" }
            try { Invoke-Brp $game 'rpc.discover' | Out-Null; break } catch { Start-Sleep 1 }
        }
        if ((Get-Date) -ge $deadline) { throw "BRP did not answer on port $Port" }
        Start-Sleep 8   # let the world finish loading and the skater settle
        $game
    } catch { Remove-Item $Lock -ErrorAction SilentlyContinue; throw }
}

function Invoke-Brp($game, [string]$Method, $Params = $null) {
    $body = @{ jsonrpc = '2.0'; id = 1; method = $Method }
    if ($null -ne $Params) { $body.params = $Params }
    $reply = Invoke-RestMethod -Uri "http://127.0.0.1:$($game.Port)/jsonrpc" -Method Post `
        -Body ($body | ConvertTo-Json -Depth 10) -ContentType 'application/json' -TimeoutSec 10
    if ($reply.error) { throw "BRP $Method failed: $($reply.error.message)" }
    $reply.result
}

<# Saves logs/test-shots/<name>.png and returns its path. #>
function Save-GameScreenshot($game, [string]$Name) {
    New-Item -ItemType Directory -Force $Shots | Out-Null
    $path = Join-Path $Shots "$Name.png"
    Remove-Item $path -ErrorAction SilentlyContinue
    Invoke-Brp $game 'brp_extras/screenshot' @{ path = ($path -replace '\\', '/') } | Out-Null
    for ($i = 0; $i -lt 20 -and -not (Test-Path $path); $i++) { Start-Sleep -Milliseconds 250 }
    $path
}

<# Presses keys for a moment, e.g. Send-GameKeys $g @('KeyW') 1000. #>
function Send-GameKeys($game, [string[]]$Keys, [int]$DurationMs = 100) {
    Invoke-Brp $game 'brp_extras/send_keys' @{ keys = $Keys; duration_ms = $DurationMs } | Out-Null
}

# The exe is a crash-report supervisor; the window belongs to its child process.
function Get-GameWindow {
    $ids = [uint32[]]@(Get-Process skate3rust -ErrorAction SilentlyContinue | ForEach-Object { [uint32]$_.Id })
    $handle = [GameTestWin]::Find($ids, 'Skate 3 Rust Engine')
    if ($handle -eq [IntPtr]::Zero) { throw 'No game window found' }
    $handle
}

<# Minimise (or restore) the game window; throws if Windows did not apply it. #>
function Set-GameWindow($game, [ValidateSet('Minimize', 'Restore')][string]$State) {
    $handle = Get-GameWindow
    $minimize = $State -eq 'Minimize'
    [GameTestWin]::ShowWindow($handle, $(if ($minimize) { 6 } else { 9 })) | Out-Null
    Start-Sleep -Milliseconds 500
    if ([GameTestWin]::IsIconic($handle) -ne $minimize) { throw "Window did not $State" }
}

<# Window size as Windows sees it (client area) and as Bevy sees it (PrimaryWindow resolution). #>
function Get-GameWindowSize($game) {
    $rect = New-Object GameTestWin+RECT
    [GameTestWin]::GetClientRect((Get-GameWindow), [ref]$rect) | Out-Null
    $query = @{ data = @{ components = @('bevy_window::window::Window') }; filter = @{} }
    $window = (Invoke-Brp $game 'world.query' $query)[0].components.'bevy_window::window::Window'
    [pscustomobject]@{
        Windows = "$($rect.Right - $rect.Left)x$($rect.Bottom - $rect.Top)"
        Bevy = "$($window.resolution.physical_width)x$($window.resolution.physical_height)"
    }
}

<# Physics failures, recoveries, panics and errors logged so far. Empty means clean. #>
function Get-GameIssues($game) {
    Select-String -Path $game.Log, $game.ErrorLog -Pattern 'non-finite|Non-finite|CRASH_RECOVERY|REPORT_PANIC [A-Za-z]| ERROR ' -ErrorAction SilentlyContinue |
        ForEach-Object { $_.Line.Substring(0, [Math]::Min(220, $_.Line.Length)) }
}

function Test-GameAlive($game) { -not $game.Process.HasExited }

<# Closes the game, restores every settings file Start-TestGame changed and releases the lock. #>
function Stop-TestGame($game) {
    try { Stop-TestGameCore $game } finally { Remove-Item $Lock -ErrorAction SilentlyContinue }
}

function Stop-TestGameCore($game) {
    if (-not $game) { return }
    if (-not $game.Process.HasExited) { Stop-Process -Id $game.Process.Id -Force }
    Get-Process skate3rust, skate-steam-relay -ErrorAction SilentlyContinue | Stop-Process -Force
    foreach ($path in $game.Backups.Keys) {
        $text = $game.Backups[$path]
        if ($null -eq $text) { Remove-Item $path -ErrorAction SilentlyContinue } else { [IO.File]::WriteAllText($path, $text) }
    }
}

Export-ModuleMember -Function Start-TestGame, Get-GameWindowSize, Invoke-Brp, Save-GameScreenshot, Send-GameKeys, Set-GameWindow, Get-GameIssues, Test-GameAlive, Stop-TestGame
