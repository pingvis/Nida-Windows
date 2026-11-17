$ErrorActionPreference = 'Stop'
$nativeOut = Join-Path ([System.IO.Path]::GetTempPath()) ('nida-decklink-platform-' + [guid]::NewGuid())
New-Item -ItemType Directory -Path $nativeOut | Out-Null
try {
    & midl.exe /nologo /env x64 /char signed /h DeckLinkAPI.h /iid DeckLinkAPI_i.c /tlb DeckLinkAPI.tlb /out $nativeOut /I vendor/decklink-sdk/win vendor/decklink-sdk/win/DeckLinkAPI.idl
    if ($LASTEXITCODE -ne 0) { throw 'MIDL generation failed' }
    $guidObject = Join-Path $nativeOut 'DeckLinkAPI_i.obj'
    & cl.exe /nologo /c "/Fo$guidObject" (Join-Path $nativeOut 'DeckLinkAPI_i.c')
    if ($LASTEXITCODE -ne 0) { throw 'SDK GUID compilation failed' }
    $testExe = Join-Path $nativeOut 'windows-platform-test.exe'
    $testObject = Join-Path $nativeOut 'windows-platform-test.obj'
    & cl.exe /nologo /std:c++17 /EHsc /W4 /I $nativeOut tests/decklink-native/windows_platform_test.cpp "/Fo$testObject" "/Fe$testExe" $guidObject /link ole32.lib oleaut32.lib uuid.lib
    if ($LASTEXITCODE -ne 0) { throw 'Windows COM adapter test compilation failed' }
    & $testExe
    if ($LASTEXITCODE -ne 0) { throw 'Windows COM adapter tests failed' }
    $shimExe = Join-Path $nativeOut 'shim-test.exe'
    $shimObject = Join-Path $nativeOut 'shim-test.obj'
    & cl.exe /nologo /std:c++17 /EHsc /I tests/decklink-native tests/decklink-native/shim_test.cpp "/Fo$shimObject" "/Fe$shimExe"
    if ($LASTEXITCODE -ne 0) { throw 'DeckLink capture/output behavioral test compilation failed' }
    & $shimExe
    if ($LASTEXITCODE -ne 0) { throw 'DeckLink capture/output behavioral tests failed' }
    & $shimExe --missing-driver
    if ($LASTEXITCODE -ne 0) { throw 'DeckLink missing-driver tests failed' }
} finally {
    $resolvedOut = [IO.Path]::GetFullPath($nativeOut)
    $tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
    if (!$resolvedOut.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to remove test output outside the temp directory: $resolvedOut"
    }
    Remove-Item -LiteralPath $resolvedOut -Recurse -Force
}
