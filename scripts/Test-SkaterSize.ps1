# SK-038/039: screenshots of the Skater Size mod at several sizes on the test world
# (logs/test-shots/size-<s>.png). The camera should frame the skater the same way at every size.
param([double[]]$Sizes = @(0.5, 1, 2), [switch]$Stage)
Import-Module (Join-Path $PSScriptRoot 'GameTest.psm1') -Force
$first = $true
foreach ($size in $Sizes) {
    $game = Start-TestGame -TestWorld -Owner 'skater-size' -Stage:($Stage -and $first) -Mods @{ 'guipegoraro.skater-size' = @{ size = $size } }
    $first = $false
    try {
        Save-GameScreenshot $game "size-$size"
        $issues = @(Get-GameIssues $game)
        if ($issues) { $issues | Select-Object -First 3; exit 1 }
    } finally { Stop-TestGame $game }
}
