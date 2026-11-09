param([Parameter(Mandatory=$true)][string]$ContentDirectory,[Parameter(Mandatory=$true)][string]$Output,[string]$FastOutput,
    [ValidatePattern('^[A-Za-z0-9][A-Za-z0-9-]{0,38}/[A-Za-z0-9][A-Za-z0-9_.-]{0,99}$')][string]$Repository='S1T2J3/Jade-Palworld-Toolbox')
$ErrorActionPreference='Stop'
$contentPath=(Resolve-Path -LiteralPath $ContentDirectory).Path
$outputPath=[IO.Path]::GetFullPath($Output)
$null=[IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($outputPath))
$python=if($env:JADE_PYTHON){$env:JADE_PYTHON}else{(Get-Command python -ErrorAction Stop).Source}
& $python (Join-Path $PSScriptRoot 'pack.py') $contentPath
if($LASTEXITCODE -ne 0){throw 'Packing failed'}
$dir=Join-Path $PSScriptRoot 'build'
$updateConfig=Join-Path $dir 'update-repository.txt'
[IO.File]::WriteAllText($updateConfig,$Repository,[Text.UTF8Encoding]::new($false))
$compiler=Join-Path $env:WINDIR 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
$common=@('/nologo','/target:winexe','/platform:x64','/optimize+','/codepage:65001',
    '/reference:System.Windows.Forms.dll','/reference:System.Drawing.dll','/reference:System.IO.Compression.dll','/reference:System.Web.Extensions.dll',
    '/reference:System.Security.dll',"/resource:$updateConfig,pal.update.repository",
    "/win32icon:$(Join-Path $PSScriptRoot '../desktop/atlas.ico')","/resource:$(Join-Path $dir 'hashes'),pal.package.hashes")
$sources=@((Join-Path $PSScriptRoot 'DesktopHost.cs'),
    (Join-Path $PSScriptRoot 'UpdateService.cs'),(Join-Path $PSScriptRoot 'ReleaseAccess.cs'),(Join-Path $PSScriptRoot 'RuntimeBootstrap.cs'),
    (Join-Path $PSScriptRoot '../migration/PalLegacyMigration.cs'),
    (Join-Path $PSScriptRoot '../game-saves/Discovery.cs'),(Join-Path $PSScriptRoot '../game-saves/XboxSaves.cs'),
    (Join-Path $PSScriptRoot '../game-saves/TerminalSources.cs'))
& $compiler @common "/out:$outputPath" "/resource:$(Join-Path $dir 'engine.zip'),pal.engine.zip" "/resource:$(Join-Path $dir 'content.zip'),pal.content.zip" @sources
if($LASTEXITCODE -ne 0){throw 'Windows executable compilation failed'}
Get-Item -LiteralPath $outputPath | Select-Object FullName,Length
if($FastOutput){
    $fastPath=[IO.Path]::GetFullPath($FastOutput)
    if($fastPath -eq $outputPath){throw 'Fast launcher and full executable need different paths'}
    $null=[IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($fastPath))
    $metadata=Join-Path $dir 'companion'
    $fullHash=(Get-FileHash -LiteralPath $outputPath -Algorithm SHA256).Hash
    [IO.File]::WriteAllLines($metadata,@($fullHash,[IO.Path]::GetFileName($outputPath)),[Text.UTF8Encoding]::new($false))
    & $compiler @common '/define:FAST_LAUNCHER' "/out:$fastPath" "/resource:$metadata,pal.companion" @sources
    if($LASTEXITCODE -ne 0){throw 'Fast launcher compilation failed'}
    Get-Item -LiteralPath $fastPath | Select-Object FullName,Length
}
