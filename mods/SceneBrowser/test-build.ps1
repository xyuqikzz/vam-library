param([Parameter(Mandatory=$true)][string]$VamRoot)
$ErrorActionPreference = 'Stop'
$output = Join-Path $PSScriptRoot '../../src-tauri/resources/mods/VamLibrary.SceneBrowser.dll'
& (Join-Path $PSScriptRoot 'build.ps1') -VamRoot $VamRoot
$first = [IO.File]::ReadAllBytes($output)
& (Join-Path $PSScriptRoot 'build.ps1') -VamRoot $VamRoot
$second = [IO.File]::ReadAllBytes($output)
if (-not [System.Linq.Enumerable]::SequenceEqual[byte]($first, $second)) { throw 'Deterministic builds differ byte for byte' }
Write-Output 'PASS: two fresh Roslyn compilations produce identical bytes'
