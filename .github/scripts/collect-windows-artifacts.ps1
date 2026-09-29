$ErrorActionPreference = 'Stop'
if ($env:APP_VERSION -notmatch '^\d+\.\d+\.\d+$') { throw 'Missing or invalid APP_VERSION' }

$release = Join-Path $PSScriptRoot '../../src-tauri/target/release'
$output = Join-Path $PSScriptRoot '../../artifacts'
$exe = Join-Path $release 'vam-library.exe'
$nsis = @(Get-ChildItem -LiteralPath (Join-Path $release 'bundle/nsis') -Filter '*-setup.exe' -File)
$msi = @(Get-ChildItem -LiteralPath (Join-Path $release 'bundle/msi') -Filter '*.msi' -File)
if (!(Test-Path -LiteralPath $exe) -or $nsis.Count -ne 1 -or $msi.Count -ne 1) {
    throw 'Expected one application executable, one NSIS installer and one MSI installer'
}
New-Item -ItemType Directory -Path $output -Force | Out-Null
$prefix = "VAM-Library-$env:APP_VERSION-windows-x64"
Copy-Item -LiteralPath $nsis[0].FullName -Destination (Join-Path $output "$prefix-setup.exe")
Copy-Item -LiteralPath $msi[0].FullName -Destination (Join-Path $output "$prefix.msi")
Compress-Archive -LiteralPath $exe -DestinationPath (Join-Path $output "$prefix-portable.zip") -Force
$checksums = Get-ChildItem -LiteralPath $output -File | Where-Object Name -ne 'SHA256SUMS.txt' | Sort-Object Name | ForEach-Object {
    $hash = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $($_.Name)"
}
$checksums | Set-Content -LiteralPath (Join-Path $output 'SHA256SUMS.txt') -Encoding utf8NoBOM
