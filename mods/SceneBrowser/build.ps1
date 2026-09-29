param([Parameter(Mandatory=$true)][string]$VamRoot)
$ErrorActionPreference = 'Stop'
$managed = Join-Path $VamRoot 'VaM_Data/Managed'
$core = Join-Path $VamRoot 'BepInEx/core'
$compiler = Join-Path $env:WINDIR 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
$output = Join-Path $PSScriptRoot '../../src-tauri/resources/mods/VamLibrary.SceneBrowser.dll'
New-Item -ItemType Directory -Force -Path (Split-Path $output) | Out-Null
$references = @('mscorlib.dll','System.dll','System.Core.dll','Assembly-CSharp.dll','UnityEngine.dll','UnityEngine.CoreModule.dll','UnityEngine.UI.dll') | ForEach-Object { '/reference:' + (Join-Path $managed $_) }
$references += @('BepInEx.dll','0Harmony.dll') | ForEach-Object { '/reference:' + (Join-Path $core $_) }
& $compiler /nologo /noconfig /nostdlib+ /target:library /optimize+ "/out:$output" @references (Join-Path $PSScriptRoot 'BrowserRules.cs') (Join-Path $PSScriptRoot 'SceneBrowserPlugin.cs')
if ($LASTEXITCODE -ne 0) { throw 'Mod compilation failed' }
$manifest = [ordered]@{
    version = '1.0.2'
    gameVersion = '1.22.0.13'
    files = @(
        @{ path = 'VaM_Data/Managed/Assembly-CSharp.dll'; sha256 = (Get-FileHash (Join-Path $managed 'Assembly-CSharp.dll')).Hash.ToLowerInvariant() },
        @{ path = 'BepInEx/core/BepInEx.dll'; sha256 = (Get-FileHash (Join-Path $core 'BepInEx.dll')).Hash.ToLowerInvariant() },
        @{ path = 'BepInEx/core/0Harmony.dll'; sha256 = (Get-FileHash (Join-Path $core '0Harmony.dll')).Hash.ToLowerInvariant() }
    )
}
$manifest | ConvertTo-Json -Depth 4 | Set-Content -Encoding utf8NoBOM (Join-Path (Split-Path $output) 'scene-browser.json')
Get-FileHash $output -Algorithm SHA256
