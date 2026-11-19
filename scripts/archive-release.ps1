param(
    [Parameter(Mandatory)]
    [ValidatePattern('\A(0|[1-9][0-9]*)(\.(0|[1-9][0-9]*)){2}(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?\z')]
    [string]$Version,
    [Parameter(Mandatory)]
    [ValidateNotNullOrEmpty()]
    [string[]]$Files
)
$ErrorActionPreference = 'Stop'
$prerelease = (($Version -split '\+', 2)[0] -split '-', 2)[1]
if ($prerelease -and ($prerelease -split '\.' -match '^0[0-9]+$')) {
    throw "Numeric prerelease identifiers cannot have leading zeros: $Version"
}
$project = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$outputRoot = Join-Path $project 'outputs'
$destination = Join-Path $outputRoot $Version
$staging = Join-Path $outputRoot ('.archive-' + [Guid]::NewGuid().ToString('n'))
$null = [IO.Directory]::CreateDirectory($outputRoot)
$lock = [IO.FileStream]::new(
    (Join-Path $outputRoot ('.' + $Version + '.lock')),
    [IO.FileMode]::OpenOrCreate, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None,
    4096, [IO.FileOptions]::DeleteOnClose
)
try {
    $null = [IO.Directory]::CreateDirectory($staging)
    $newFiles = @()
    foreach ($path in $Files) {
        $source = Get-Item -LiteralPath $path
        if ($source -isnot [IO.FileInfo]) { throw "Expected a release file: $path" }
        $staged = Join-Path $staging $source.Name
        $target = Join-Path $destination $source.Name
        if (Test-Path -LiteralPath $target) { $existing = $target }
        elseif ([IO.File]::Exists($staged)) { $existing = $staged }
        else {
            [IO.File]::Copy($source.FullName, $staged)
            $newFiles += $source.Name
            continue
        }
        if (-not [IO.File]::Exists($existing) -or
            (Get-FileHash -LiteralPath $existing -Algorithm SHA256).Hash -ne
            (Get-FileHash -LiteralPath $source.FullName -Algorithm SHA256).Hash) {
            throw "Different content already uses $($source.Name) in $Version. Archive the new build under a new version."
        }
    }
    if (-not [IO.Directory]::Exists($destination)) {
        [IO.Directory]::Move($staging, $destination)
    } else {
        # All collisions and inputs were checked before adding anything to the archive.
        $added = @()
        try {
            foreach ($name in $newFiles) {
                $target = Join-Path $destination $name
                [IO.File]::Move((Join-Path $staging $name), $target)
                $added += $target
            }
        } catch {
            foreach ($target in $added) { Remove-Item -LiteralPath $target -Force }
            throw
        }
    }
} finally {
    try {
        if ([IO.Directory]::Exists($staging)) {
            Get-ChildItem -LiteralPath $staging -File -Force | Remove-Item -Force
            [IO.Directory]::Delete($staging)
        }
    } finally {
        $lock.Dispose()
    }
}
