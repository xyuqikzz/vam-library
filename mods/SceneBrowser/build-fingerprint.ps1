# Used only for local build output reuse. Installation still checks exact hashes.
function Get-SceneBrowserBuildFingerprint {
    param([Parameter(Mandatory=$true)][string]$Path)
    $bytes = [IO.File]::ReadAllBytes($Path)
    $stream = [IO.MemoryStream]::new($bytes, $false)
    $pe = $null
    try {
        $pe = [System.Reflection.PortableExecutable.PEReader]::new($stream)
        $metadata = [System.Reflection.Metadata.PEReaderExtensions]::GetMetadataReader($pe)
        $module = $metadata.GetModuleDefinition()
        $mvid = $metadata.GetGuid($module.Mvid)
        if ($module.Mvid.IsNil) { throw 'Missing module build identifier' }

        # The Framework compiler changes these fields on every identical build.
        # Leave IL, resources, references and every other metadata byte intact.
        $metadataStart = $pe.PEHeaders.MetadataStartOffset
        $guidHeap = [System.Reflection.Metadata.Ecma335.MetadataReaderExtensions]::GetHeapMetadataOffset($metadata, [System.Reflection.Metadata.Ecma335.HeapIndex]::Guid)
        $guidIndex = [System.Reflection.Metadata.Ecma335.MetadataTokens]::GetHeapOffset($module.Mvid)
        $guidStart = $metadataStart + $guidHeap + (($guidIndex - 1) * 16)
        [Array]::Clear($bytes, $pe.PEHeaders.CoffHeaderStartOffset + 4, 4)
        [Array]::Clear($bytes, $guidStart, 16)

        $stringHeap = [System.Reflection.Metadata.Ecma335.MetadataReaderExtensions]::GetHeapMetadataOffset($metadata, [System.Reflection.Metadata.Ecma335.HeapIndex]::String)
        $prefix = '<PrivateImplementationDetails>{'
        $privateName = $prefix + $mvid.ToString().ToUpperInvariant() + '}'
        foreach ($handle in $metadata.TypeDefinitions) {
            $type = $metadata.GetTypeDefinition($handle)
            if ($metadata.GetString($type.Name) -ceq $privateName) {
                $nameStart = $metadataStart + $stringHeap + [System.Reflection.Metadata.Ecma335.MetadataTokens]::GetHeapOffset($type.Name)
                [Array]::Clear($bytes, $nameStart + $prefix.Length, 36)
            }
        }
        $sha = [Security.Cryptography.SHA256]::Create()
        try { return [BitConverter]::ToString($sha.ComputeHash($bytes)).Replace('-', '').ToLowerInvariant() }
        finally { $sha.Dispose() }
    } finally {
        if ($pe) { $pe.Dispose() }
        $stream.Dispose()
    }
}
