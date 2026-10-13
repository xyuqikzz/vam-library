param([Parameter(Mandatory=$true)][string]$VamRoot)
$ErrorActionPreference = 'Stop'
$managed = Join-Path $VamRoot 'VaM_Data/Managed'
$core = Join-Path $VamRoot 'BepInEx/core'
# Roslyn supports deterministic output while /nostdlib keeps the game CLR 2 ABI.
$dotnet = (Get-Command dotnet -ErrorAction Stop).Source
$sdk = & $dotnet --list-sdks | Select-Object -Last 1
if ($sdk -notmatch '^([^ ]+) \[(.+)\]$') { throw 'A .NET SDK with Roslyn is required' }
$compiler = Join-Path $Matches[2] "$($Matches[1])/Roslyn/bincore/csc.dll"
if (-not (Test-Path -LiteralPath $compiler)) { throw 'Roslyn csc.dll was not found' }
$sources = @('BrowserRules.cs', 'BrowserText.cs', 'ImportTimeIndex.cs', 'SceneLaunchRequest.cs', 'LaunchProgress.cs') | ForEach-Object { Join-Path $PSScriptRoot $_ }
$sources += Get-ChildItem -LiteralPath $PSScriptRoot -Filter 'SceneBrowserPlugin*.cs' | Sort-Object Name | Select-Object -ExpandProperty FullName
$output = Join-Path $PSScriptRoot '../../src-tauri/resources/mods/VamLibrary.SceneBrowser.dll'
New-Item -ItemType Directory -Force -Path (Split-Path $output) | Out-Null
# SharpZipLib stays a compile-time reference only so game type signatures resolve;
# the plugin no longer uses it, and test.ps1 verifies no runtime reference is emitted.
$references = @('mscorlib.dll','System.dll','System.Core.dll','ICSharpCode.SharpZipLib.dll','Assembly-CSharp.dll','UnityEngine.dll','UnityEngine.CoreModule.dll','UnityEngine.UI.dll') | ForEach-Object { '/reference:' + (Join-Path $managed $_) }
$references += @('BepInEx.dll','0Harmony.dll') | ForEach-Object { '/reference:' + (Join-Path $core $_) }
$buildDirectory = Join-Path ([IO.Path]::GetTempPath()) ('VamSceneBrowserBuild-' + [guid]::NewGuid())
New-Item -ItemType Directory -Path $buildDirectory | Out-Null
$candidate = Join-Path $buildDirectory 'VamLibrary.SceneBrowser.dll'
try {
    & $dotnet $compiler /nologo /noconfig /nostdlib+ /deterministic+ /langversion:5 /target:library /optimize+ "/pathmap:$PSScriptRoot=/SceneBrowser" "/out:$candidate" @references @sources
    if ($LASTEXITCODE -ne 0) { throw 'Mod compilation failed' }
    Copy-Item -LiteralPath $candidate -Destination $output -Force
} finally {
    if (Test-Path -LiteralPath $candidate) { Remove-Item -LiteralPath $candidate }
    Remove-Item -LiteralPath $buildDirectory
}
$manifest = [ordered]@{
    version = '1.2.0'
    gameVersion = '1.22.0.13'
    # SharpZipLib, System.dll and MonoPosixHelper were pinned only for native
    # decompression, which was removed in 1.0.10.
    files = @(
        [ordered]@{ sha256 = (Get-FileHash (Join-Path $managed 'Assembly-CSharp.dll')).Hash.ToLowerInvariant(); path = 'VaM_Data/Managed/Assembly-CSharp.dll' },
        [ordered]@{ sha256 = (Get-FileHash (Join-Path $core 'BepInEx.dll')).Hash.ToLowerInvariant(); path = 'BepInEx/core/BepInEx.dll' },
        [ordered]@{ sha256 = (Get-FileHash (Join-Path $core '0Harmony.dll')).Hash.ToLowerInvariant(); path = 'BepInEx/core/0Harmony.dll' }
    )
}
$manifest | ConvertTo-Json -Depth 4 | Set-Content -Encoding utf8NoBOM (Join-Path (Split-Path $output) 'scene-browser.json')
Get-FileHash $output -Algorithm SHA256
