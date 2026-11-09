param([Parameter(Mandatory=$true)][string]$Distribution)
$ErrorActionPreference='Stop'
$project=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$dist=(Resolve-Path -LiteralPath $Distribution).Path
$main=Join-Path $dist 'Jade-Palworld-Toolbox.exe'
$fast=Join-Path $dist 'Jade-Palworld-Toolbox-Fast.exe'
$version=[Reflection.AssemblyName]::GetAssemblyName($main).Version
if ([Reflection.AssemblyName]::GetAssemblyName($fast).Version -ne $version) {throw 'The two launcher versions differ'}
$compiler=$env:JADE_ISCC
if (!$compiler) {
    $command=Get-Command ISCC.exe -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($command) {$compiler=$command.Source}
}
if (!$compiler) {
    foreach ($root in @((Join-Path $env:LOCALAPPDATA 'Programs'),$env:ProgramFiles,${env:ProgramFiles(x86)})) {
        $path=Join-Path $root 'Inno Setup 7/ISCC.exe'
        if (Test-Path -LiteralPath $path -PathType Leaf) {$compiler=$path;break}
    }
}
if (!$compiler -or !(Test-Path -LiteralPath $compiler -PathType Leaf)) {throw 'Install Inno Setup 7 or set JADE_ISCC to the absolute path of ISCC.exe.'}
$icon=Join-Path $project 'work/desktop/atlas.ico'
$script=Join-Path $project 'work/standalone/installer/JadeToolbox.iss'
& $compiler '/Qp' ("/DAppVersion="+$version.ToString(3)) ("/DDistPath="+$dist) ("/DProductIcon="+$icon) $script
if ($LASTEXITCODE -ne 0) {throw 'Windows installer build failed'}
$setup=Get-Item -LiteralPath (Join-Path $dist 'Jade-Palworld-Toolbox-Setup.exe')
if ([Version]$setup.VersionInfo.FileVersion -ne $version) {throw 'Unexpected Windows installer version'}
$setup.FullName
