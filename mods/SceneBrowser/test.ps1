param([Parameter(Mandatory=$true)][string]$VamRoot)
$ErrorActionPreference = 'Stop'
$compiler = Join-Path $env:WINDIR 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
$output = Join-Path ([IO.Path]::GetTempPath()) ('VamBrowserRules-' + [guid]::NewGuid() + '.exe')
try {
    & $compiler /nologo /target:exe "/out:$output" (Join-Path $PSScriptRoot 'BrowserRules.cs') (Join-Path $PSScriptRoot 'tests/BrowserRulesTests.cs')
    if ($LASTEXITCODE -ne 0) { throw 'Test compilation failed' }
    & $output
    if ($LASTEXITCODE -ne 0) { throw 'Browser rules tests failed' }
} finally {
    if (Test-Path -LiteralPath $output) { Remove-Item -LiteralPath $output }
}

# Validate the actual binary contracts used by Harmony, including parameter
# positions and visibility. This is not a substitute for an in-game test.
Add-Type -Path (Join-Path $VamRoot 'BepInEx/core/Mono.Cecil.dll')
$assembly = [Mono.Cecil.AssemblyDefinition]::ReadAssembly((Join-Path $VamRoot 'VaM_Data/Managed/Assembly-CSharp.dll'))
try {
    $browser = $assembly.MainModule.GetType('uFileBrowser.FileBrowser')
    foreach ($name in @('SetTitle','ShowInternal','SetSortBy','SortFilesAndDirs','GotoDirectory','OnFileClick','SyncDisplayed','SyncSort','CreateFileButton','HideButton','Hide')) {
        $methods = @($browser.Methods | Where-Object Name -EQ $name)
        if ($methods.Count -ne 1) { throw "Missing or ambiguous hook: $name" }
    }
    $sort = $browser.Methods | Where-Object Name -EQ 'SortFilesAndDirs'
    if ($sort.Parameters[0].ParameterType.FullName -ne 'System.Collections.Generic.List`1<uFileBrowser.FileBrowser/FileAndDirInfo>') { throw 'Sort contract changed' }
    foreach ($name in @('sortedFilesAndDirs','displayedFileButtons','fileContent')) {
        if (-not ($browser.Fields | Where-Object Name -EQ $name)) { throw "Missing field: $name" }
    }
    Write-Output 'PASS: 15 game binary contract checks'
} finally { $assembly.Dispose() }

$plugin = [Mono.Cecil.AssemblyDefinition]::ReadAssembly((Join-Path $PSScriptRoot '../../src-tauri/resources/mods/VamLibrary.SceneBrowser.dll'))
try {
    if ($plugin.MainModule.RuntimeVersion -notlike 'v2.0*') { throw 'Plugin must target the game CLR 2 profile' }
    $framework = @($plugin.MainModule.AssemblyReferences | Where-Object { $_.Name -in @('mscorlib','System','System.Core') })
    if ($framework | Where-Object { $_.Version.Major -ge 4 }) { throw 'CLR 4 framework reference found' }
    if ($plugin.MainModule.GetTypeReferences() | Where-Object FullName -Like '*ConditionalWeakTable*') { throw 'Unsupported ConditionalWeakTable reference found' }
    Write-Output 'PASS: game CLR 2 target and framework references'
} finally { $plugin.Dispose() }
