param([Parameter(Mandatory=$true)][string]$VamRoot)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'build-fingerprint.ps1')
$output = Join-Path $PSScriptRoot '../../src-tauri/resources/mods/VamLibrary.SceneBrowser.dll'
$rebuild = Join-Path $PSScriptRoot 'tests/fixtures/SceneBrowser-1.0.8-rebuild.dll'
if ((Get-SceneBrowserBuildFingerprint $rebuild) -ne 'b150843894ab1419c8e2473369a67c7fddc649e4083cd1b7029c79230da93d9e') {
    throw 'Verified 1.0.8 rebuild fingerprint changed'
}
$temporary = Join-Path ([IO.Path]::GetTempPath()) ('VamBuildCheck-' + [guid]::NewGuid() + '.dll')
try {
    $changed = [IO.File]::ReadAllBytes($rebuild)
    # Executable section, outside all build metadata ignored by the fingerprint.
    $changed[512] = $changed[512] -bxor 1
    [IO.File]::WriteAllBytes($temporary, $changed)
    if ((Get-SceneBrowserBuildFingerprint $temporary) -eq (Get-SceneBrowserBuildFingerprint $rebuild)) {
        throw 'Changed executable contents were ignored'
    }
    Write-Output 'PASS: changed executable contents change the build fingerprint'
    [IO.File]::WriteAllBytes($temporary, [byte[]]@(0, 1, 2))
    $invalidRejected = $false
    try { Get-SceneBrowserBuildFingerprint $temporary | Out-Null }
    catch { $invalidRejected = $true }
    if (-not $invalidRejected) { throw 'Invalid assembly was accepted' }
    Write-Output 'PASS: invalid assemblies fail closed'
} finally {
    if (Test-Path -LiteralPath $temporary) { Remove-Item -LiteralPath $temporary }
}

& (Join-Path $PSScriptRoot 'build.ps1') -VamRoot $VamRoot
$firstHash = (Get-FileHash -LiteralPath $output -Algorithm SHA256).Hash
$firstWriteTime = (Get-Item -LiteralPath $output).LastWriteTimeUtc
& (Join-Path $PSScriptRoot 'build.ps1') -VamRoot $VamRoot
if ((Get-FileHash -LiteralPath $output -Algorithm SHA256).Hash -ne $firstHash -or
    (Get-Item -LiteralPath $output).LastWriteTimeUtc -ne $firstWriteTime) {
    throw 'An unchanged build replaced the DLL'
}
Write-Output 'PASS: repeated compilation preserves the DLL bytes and write time'
