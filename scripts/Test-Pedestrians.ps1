# SK-034: NPC pedestrians spawn on the test world, walk (roots move) and show up in screenshots.
# Exits 1 on failure. Screenshots: logs/test-shots/pedestrians-<n>.png, 0.3 s apart.
param([int]$Count = 8, [switch]$Stage)
Import-Module (Join-Path $PSScriptRoot 'GameTest.psm1') -Force
$g = Start-TestGame -Owner 'npc' -Stage:$Stage -Env @{ SKATE3_PEDESTRIANS = "$Count" }
$failed = $false
try {
    $query = @{ data = @{ components = @('bevy_ecs::name::Name', 'bevy_transform::components::transform::Transform') } }
    function Get-Pedestrians {
        (Invoke-Brp $g 'world.query' $query) | Where-Object { $_.components.'bevy_ecs::name::Name' -eq 'Pedestrian' } |
            ForEach-Object { [pscustomobject]@{ Id = $_.entity; At = $_.components.'bevy_transform::components::transform::Transform'.translation } }
    }
    $before = @(Get-Pedestrians)
    Start-Sleep 3
    $after = @(Get-Pedestrians)
    "pedestrians: $($before.Count) (wanted $Count)"
    $moved = 0
    foreach ($p in $after) {
        $old = $before | Where-Object Id -eq $p.Id
        if (-not $old) { continue }
        $d = [Math]::Sqrt([Math]::Pow($p.At[0] - $old.At[0], 2) + [Math]::Pow($p.At[2] - $old.At[2], 2))
        '{0,-12} {1,8:N2} {2,8:N2} {3,8:N2}  moved {4:N2} m' -f $p.Id, $p.At[0], $p.At[1], $p.At[2], $d
        if ($d -gt 0.5) { $moved++ }
    }
    foreach ($i in 1..4) { Save-GameScreenshot $g "pedestrians-$i" | Out-Null; Start-Sleep -Milliseconds 300 }
    $issues = @(Get-GameIssues $g)
    $issues
    if ($before.Count -ne $Count) { 'FAIL: pedestrian count'; $failed = $true }
    if ($moved -eq 0) { 'FAIL: no pedestrian walked in 3 s'; $failed = $true }
    if ($issues.Count) { 'FAIL: issues in the log'; $failed = $true }
} finally { Stop-TestGame $g }
if ($failed) { exit 1 }
'PASS'
