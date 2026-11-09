param(
    [ValidatePattern('^[A-Za-z0-9][A-Za-z0-9-]{0,38}/[A-Za-z0-9][A-Za-z0-9_.-]{0,99}$')]
    [string]$Repository = 'S1T2J3/Jade-Palworld-Toolbox',
    [string[]]$DeltaBase = @()
)
$ErrorActionPreference = 'Stop'
$project = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$source = Join-Path $project 'work'
& node (Join-Path $PSScriptRoot 'prepare-announcement.cjs')
if ($LASTEXITCODE -ne 0) { throw 'Announcement preparation failed' }
$dist = Join-Path $project 'dist'
$content = Join-Path $project ('build/content-' + [Guid]::NewGuid().ToString('n'))
$null = [IO.Directory]::CreateDirectory($dist)
$null = [IO.Directory]::CreateDirectory($content)

$html = Join-Path $dist 'Jade-Palworld-Toolbox.html'
$desktop = Join-Path $dist 'Jade-Palworld-Toolbox.exe'
$fast = Join-Path $dist 'Jade-Palworld-Toolbox-Fast.exe'
# Capture an existing dist EXE before the compiler replaces it in place.
$baselines = foreach ($path in $DeltaBase) {
    $resolved = (Get-Item -LiteralPath $path -ErrorAction Stop).FullName
    if ([IO.Directory]::Exists($resolved)) { $resolved = Join-Path $resolved 'Jade-Palworld-Toolbox.exe' }
    $hash = (Get-FileHash -LiteralPath $resolved -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($resolved -eq $desktop) {
        $saved = Join-Path $project ('build/base-' + $hash + '.exe')
        [IO.File]::Copy($resolved,$saved,$true)
        $resolved = $saved
    }
    [pscustomobject]@{Path=$resolved; Hash=$hash}
}
$previousRepository = $env:JADE_RELEASE_REPOSITORY
try {
    $env:JADE_RELEASE_REPOSITORY = $Repository
    & node (Join-Path $source 'build.cjs') $html
    if ($LASTEXITCODE -ne 0) { throw 'HTML build failed' }
    & node (Join-Path $source 'build.cjs') --desktop $content
    if ($LASTEXITCODE -ne 0) { throw 'Desktop content build failed' }
} finally {
    $env:JADE_RELEASE_REPOSITORY = $previousRepository
}

$reader = Join-Path $source 'terminal-import/parser'
& (Join-Path $reader 'build.ps1')
Add-Type -AssemblyName System.IO.Compression.FileSystem
$readerArchive = Join-Path $reader 'terminal-reader-source.zip'
$newArchive = Join-Path $project ('build/reader-' + [Guid]::NewGuid().ToString('n') + '.zip')
[IO.Compression.ZipFile]::CreateFromDirectory((Join-Path $reader 'src'), $newArchive)
[IO.File]::Copy($newArchive, $readerArchive, $true)

& (Join-Path $source 'standalone/build.ps1') -ContentDirectory $content -Output $desktop -FastOutput $fast -Repository $Repository
$htmlHead = [IO.File]::ReadLines($html) | Select-Object -First 15
$desktopHead = [IO.File]::ReadLines((Join-Path $content 'index.html')) | Select-Object -First 15
$version = [Reflection.AssemblyName]::GetAssemblyName($desktop).Version
if ([Reflection.AssemblyName]::GetAssemblyName($fast).Version -ne $version) { throw 'Executable version mismatch' }
$versionTag = '<meta name="pal-release-version" content="' + $version.ToString(3) + '">'
$repositoryTag = '<meta name="pal-release-repository" content="' + $Repository + '">'
foreach ($tag in @($versionTag,$repositoryTag)) {
    if (-not ($htmlHead -match [regex]::Escape($tag)) -or -not ($desktopHead -match [regex]::Escape($tag))) {
        throw "Release metadata mismatch: $tag"
    }
}
$patches = @()
foreach ($base in $baselines) {
    $python = if ($env:JADE_PYTHON) { $env:JADE_PYTHON } else { (Get-Command python -ErrorAction Stop).Source }
    & $python (Join-Path $source 'standalone/make_delta.py') $base.Path $dist
    if ($LASTEXITCODE -ne 0) { throw 'Delta generation failed' }
    $patch = Join-Path $dist ('Jade-Delta-from-' + $base.Hash + '.jdp')
    if ([IO.File]::Exists($patch) -and $patch -notin $patches) { $patches += $patch }
}
foreach ($previous in (Get-ChildItem -LiteralPath $dist -File -Filter 'Jade-Delta-from-*.jdp')) {
    if ($previous.Name -cmatch '^Jade-Delta-from-[a-f0-9]{64}\.jdp$' -and $previous.FullName -notin $patches) {
        Remove-Item -LiteralPath $previous.FullName
    }
}
& (Join-Path $PSScriptRoot 'build-installer.ps1') -Distribution $dist
$setup=Join-Path $dist 'Jade-Palworld-Toolbox-Setup.exe'
$checksums = foreach ($asset in (@($desktop, $fast, $html, $setup) + $patches)) {
    '{0}  {1}' -f (Get-FileHash -LiteralPath $asset -Algorithm SHA256).Hash.ToLowerInvariant(), [IO.Path]::GetFileName($asset)
}
[IO.File]::WriteAllLines((Join-Path $dist 'SHA256SUMS.txt'), [string[]]$checksums, [Text.Encoding]::ASCII)
Get-ChildItem -LiteralPath $dist | Select-Object Name,Length
