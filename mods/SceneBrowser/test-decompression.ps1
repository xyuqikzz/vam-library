param([Parameter(Mandatory=$true)][string]$VamRoot)
$ErrorActionPreference = 'Stop'
$compiler = Join-Path $env:WINDIR 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
$temp = Join-Path ([IO.Path]::GetTempPath()) ('VamDecompression-' + [guid]::NewGuid())
New-Item -ItemType Directory -Path $temp | Out-Null
try {
    $hostExe = Join-Path $temp 'MonoHost.exe'
    $probeExe = Join-Path $temp 'DecompressionTests.exe'
    & $compiler /nologo /target:exe /platform:x64 "/out:$hostExe" (Join-Path $PSScriptRoot 'tests/MonoHost.cs')
    if ($LASTEXITCODE -ne 0) { throw 'Host compilation failed' }
    $refs = @('mscorlib.dll','System.dll','System.Core.dll','System.Drawing.dll','ICSharpCode.SharpZipLib.dll') | ForEach-Object { '/reference:' + (Join-Path $VamRoot "VaM_Data/Managed/$_") }
    $refs += '/reference:' + (Join-Path $VamRoot 'BepInEx/core/0Harmony.dll')
    $plugin = Join-Path $PSScriptRoot '../../src-tauri/resources/mods/VamLibrary.SceneBrowser.dll'
    $refs += '/reference:' + $plugin
    & $compiler /nologo /noconfig /nostdlib+ /target:exe "/out:$probeExe" @refs (Join-Path $PSScriptRoot 'tests/NativeDecompressionTests.cs')
    if ($LASTEXITCODE -ne 0) { throw 'Decompression tests compilation failed' }
    & $hostExe $VamRoot $probeExe $plugin
    if ($LASTEXITCODE -ne 0) { throw 'Game Mono decompression tests failed' }
} finally {
    foreach ($name in @('MonoHost.exe','DecompressionTests.exe')) {
        $file = Join-Path $temp $name
        if (Test-Path -LiteralPath $file) { Remove-Item -LiteralPath $file }
    }
    Remove-Item -LiteralPath $temp
}
