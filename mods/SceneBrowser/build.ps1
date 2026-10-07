param([Parameter(Mandatory=$true)][string]$VamRoot)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'build-fingerprint.ps1')
$managed = Join-Path $VamRoot 'VaM_Data/Managed'
$core = Join-Path $VamRoot 'BepInEx/core'
$compiler = Join-Path $env:WINDIR 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
$output = Join-Path $PSScriptRoot '../../src-tauri/resources/mods/VamLibrary.SceneBrowser.dll'
New-Item -ItemType Directory -Force -Path (Split-Path $output) | Out-Null
$references = @('mscorlib.dll','System.dll','System.Core.dll','ICSharpCode.SharpZipLib.dll','Assembly-CSharp.dll','UnityEngine.dll','UnityEngine.CoreModule.dll','UnityEngine.UI.dll') | ForEach-Object { '/reference:' + (Join-Path $managed $_) }
$references += @('BepInEx.dll','0Harmony.dll') | ForEach-Object { '/reference:' + (Join-Path $core $_) }
$buildDirectory = Join-Path ([IO.Path]::GetTempPath()) ('VamSceneBrowserBuild-' + [guid]::NewGuid())
New-Item -ItemType Directory -Path $buildDirectory | Out-Null
$candidate = Join-Path $buildDirectory 'VamLibrary.SceneBrowser.dll'
try {
    & $compiler /nologo /noconfig /nostdlib+ /target:library /optimize+ "/out:$candidate" @references (Join-Path $PSScriptRoot 'BrowserRules.cs') (Join-Path $PSScriptRoot 'ImportTimeIndex.cs') (Join-Path $PSScriptRoot 'SceneLaunchRequest.cs') (Join-Path $PSScriptRoot 'NativeDecompression.cs') (Join-Path $PSScriptRoot 'SceneBrowserPlugin.cs')
    if ($LASTEXITCODE -ne 0) { throw 'Mod compilation failed' }
    if ((Test-Path -LiteralPath $output) -and
        (Get-SceneBrowserBuildFingerprint $output) -eq (Get-SceneBrowserBuildFingerprint $candidate)) {
        Write-Output 'Unchanged plugin contents; keeping the existing DLL and its release hash.'
    } else {
        Copy-Item -LiteralPath $candidate -Destination $output -Force
    }
} finally {
    if (Test-Path -LiteralPath $candidate) { Remove-Item -LiteralPath $candidate }
    Remove-Item -LiteralPath $buildDirectory
}
$manifest = [ordered]@{
    version = '1.0.9'
    gameVersion = '1.22.0.13'
    files = @(
        [ordered]@{ sha256 = (Get-FileHash (Join-Path $managed 'Assembly-CSharp.dll')).Hash.ToLowerInvariant(); path = 'VaM_Data/Managed/Assembly-CSharp.dll' },
        [ordered]@{ sha256 = (Get-FileHash (Join-Path $managed 'ICSharpCode.SharpZipLib.dll')).Hash.ToLowerInvariant(); path = 'VaM_Data/Managed/ICSharpCode.SharpZipLib.dll' },
        [ordered]@{ sha256 = (Get-FileHash (Join-Path $managed 'System.dll')).Hash.ToLowerInvariant(); path = 'VaM_Data/Managed/System.dll' },
        [ordered]@{ sha256 = (Get-FileHash (Join-Path $VamRoot 'Mono/EmbedRuntime/MonoPosixHelper.dll')).Hash.ToLowerInvariant(); path = 'Mono/EmbedRuntime/MonoPosixHelper.dll' },
        [ordered]@{ sha256 = (Get-FileHash (Join-Path $core 'BepInEx.dll')).Hash.ToLowerInvariant(); path = 'BepInEx/core/BepInEx.dll' },
        [ordered]@{ sha256 = (Get-FileHash (Join-Path $core '0Harmony.dll')).Hash.ToLowerInvariant(); path = 'BepInEx/core/0Harmony.dll' }
    )
}
$manifest | ConvertTo-Json -Depth 4 | Set-Content -Encoding utf8NoBOM (Join-Path (Split-Path $output) 'scene-browser.json')
Get-FileHash $output -Algorithm SHA256
