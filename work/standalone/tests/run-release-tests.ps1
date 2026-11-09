param([string]$ScratchRoot=[IO.Path]::GetTempPath())
$ErrorActionPreference='Stop'
$source=Split-Path $PSScriptRoot
$work=Join-Path ([IO.Path]::GetFullPath($ScratchRoot)) ("JadeReleaseTests-"+[guid]::NewGuid().ToString('N'))
$null=[IO.Directory]::CreateDirectory($work)
try {
    $testExe=Join-Path $work 'ReleaseTests.exe'
    $csc=Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
    & $csc /nologo /target:exe /platform:x64 /codepage:65001 /reference:System.Security.dll /reference:System.Web.Extensions.dll /reference:System.Windows.Forms.dll /reference:System.Drawing.dll "/out:$testExe" (Join-Path $source 'UpdateService.cs') (Join-Path $source 'ReleaseDelta.cs') (Join-Path $source 'ReleaseAccess.cs') (Join-Path $PSScriptRoot 'ReleaseTests.cs') (Join-Path $PSScriptRoot 'DeltaTests.cs')
    if($LASTEXITCODE){throw "Test compilation failed ($LASTEXITCODE)"}
    & $testExe $work
    if($LASTEXITCODE){throw "Release tests failed ($LASTEXITCODE)"}
} finally {
    $expected=[IO.Path]::GetFullPath($ScratchRoot).TrimEnd('\')+'\'
    $testDir=[IO.Path]::GetFullPath($work)
    if($testDir.StartsWith($expected,[StringComparison]::OrdinalIgnoreCase) -and
       [IO.Path]::GetFileName($testDir) -match '^JadeReleaseTests-[a-f0-9]{32}$'){
        Remove-Item -LiteralPath $testDir -Recurse -Force
    }
}
