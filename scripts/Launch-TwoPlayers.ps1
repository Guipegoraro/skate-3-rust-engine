# Two players side by side (SK-002): host + guest over local direct multiplayer, each window
# on one half of the primary monitor, borderless, and tied to its own XInput controller.
param([string]$MapPath = '', [switch]$Release, [switch]$Windowed)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
$binary = if ($Release) { Join-Path $workspace 'bin/multiplayer/skate3-multiplayer.exe' } else { Join-Path $workspace 'bin/skate3rust.exe' }
if (-not (Test-Path -LiteralPath $binary)) { throw "Missing $binary. Run BUILD.bat (or scripts/build-multiplayer-test.ps1 for -Release)." }
$assets = (Resolve-Path -LiteralPath (Join-Path $workspace 'assets')).Path

Add-Type -Namespace SkateLauncher -Name Native -MemberDefinition @'
[StructLayout(LayoutKind.Sequential)] public struct XInputState { public uint Packet; public ushort Buttons; public byte LT; public byte RT; public short LX; public short LY; public short RX; public short RY; }
[DllImport("xinput1_4.dll")] public static extern uint XInputGetState(uint slot, out XInputState state);
[DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
[StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left; public int Top; public int Right; public int Bottom; }
[DllImport("user32.dll")] public static extern bool SystemParametersInfo(uint action, uint param, ref Rect rect, uint winIni);
'@
# Physical pixels: the game's --window rectangle is physical too.
[SkateLauncher.Native]::SetProcessDPIAware() | Out-Null
$area = New-Object SkateLauncher.Native+Rect
[SkateLauncher.Native]::SystemParametersInfo(0x30, 0, [ref]$area, 0) | Out-Null # SPI_GETWORKAREA
$half = [int](($area.Right - $area.Left) / 2)
$height = $area.Bottom - $area.Top

$pads = @(0..3 | Where-Object { $s = New-Object SkateLauncher.Native+XInputState; [SkateLauncher.Native]::XInputGetState($_, [ref]$s) -eq 0 })
if ($pads.Count -lt 2) { Write-Warning "Found $($pads.Count) controller(s). Without two, the controller follows the focused window." }

$logs = Join-Path $workspace ('logs/2p/' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
New-Item -ItemType Directory -Path $logs -Force | Out-Null
# Reserve two free UDP ports, then release them just before the games start.
$reservations = @(1..2 | ForEach-Object { [Net.Sockets.UdpClient]::new([Net.IPEndPoint]::new([Net.IPAddress]::Loopback, 0)) })
$ports = @($reservations | ForEach-Object { $_.Client.LocalEndPoint.Port })
$session = Get-Random -Minimum 1 -Maximum ([long]::MaxValue)
$reservations | ForEach-Object { $_.Dispose() }

$env:SKATE3_MODS = Join-Path $workspace 'mods'
$common = '--assets "' + $assets + '" --net-session ' + $session
if ($MapPath) { $common += ' --map "' + (Resolve-Path -LiteralPath $MapPath).Path + '"' }
for ($index = 0; $index -lt 2; $index++) {
    $label = @('1', '2')[$index]
    $playerArgs = $common + ' --player-title "Jogador ' + $label + '"'
    if ($index -eq 0) { $playerArgs += ' --net-host 127.0.0.1:' + $ports[0] }
    else { $playerArgs += ' --net-local 127.0.0.1:' + $ports[1] + ' 127.0.0.1:' + $ports[0] + ' --spawn-offset 2' }
    if ($pads.Count -ge 2) { $playerArgs += ' --controller ' + $pads[$index] }
    if (-not $Windowed) { $playerArgs += " --window $($area.Left + $index * $half) $($area.Top) $half $height --borderless" }
    # Each instance serves BRP on its own port (15703 host, 15704 guest).
    $env:BRP_EXTRAS_PORT = [string](15703 + $index)
    $process = Start-Process -FilePath $binary -ArgumentList $playerArgs -WorkingDirectory $workspace -PassThru `
        -RedirectStandardOutput (Join-Path $logs "player-$label.out.log") -RedirectStandardError (Join-Path $logs "player-$label.err.log")
    Write-Host "Started player $label (pid $($process.Id))"
}
Write-Host "Logs: $logs"
