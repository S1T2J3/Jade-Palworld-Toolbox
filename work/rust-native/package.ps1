param(
    [string]$Version,
    [string]$InnoCompiler,
    [string]$PreviousPackage,
    [string]$BaseExecutable,
    [string]$KernelExecutable
)
$ErrorActionPreference='Stop'
$project=[IO.Path]::GetFullPath($PSScriptRoot)
$manifestPath = Join-Path $project 'crates/jade-app/Cargo.toml'
$declaration = Select-String -LiteralPath $manifestPath -Pattern '^version\s*=\s*"([^"]+)"\s*$' -Encoding UTF8 | Select-Object -First 1
if (!$declaration) { throw "Missing version in $manifestPath" }
$cargoVersion = $declaration.Matches[0].Groups[1].Value
if (!$Version) { $Version = $cargoVersion }
if (!$BaseExecutable) {
    $BaseExecutable = Join-Path $project 'target/release/Jade-Palworld-Rust.exe'
}
$baseFile = Get-Item -LiteralPath $BaseExecutable
if ($baseFile -isnot [IO.FileInfo]) { throw "Expected a base executable: $BaseExecutable" }
if ($KernelExecutable) {
    if ($Version -cne $cargoVersion) { throw "Kernel package version must match Cargo version $cargoVersion." }
    $kernelFile = Get-Item -LiteralPath $KernelExecutable
    if ($kernelFile -isnot [IO.FileInfo]) { throw "Expected a kernel executable: $KernelExecutable" }
    $kernelHash = (Get-FileHash -LiteralPath $kernelFile.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    if ((Get-FileHash -LiteralPath $baseFile.FullName -Algorithm SHA256).Hash -eq $kernelHash) {
        throw 'The base and kernel binaries must be built separately.'
    }
    # The host's runtime check uses this exact ASCII digest embedded by the paired build.
    $hostImage = [Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes($baseFile.FullName))
    if ($hostImage.IndexOf($kernelHash, [StringComparison]::Ordinal) -lt 0) {
        throw 'The base executable does not contain the selected kernel SHA-256. Rebuild the pair with build-assistant.ps1.'
    }
    $hostImage = $null
}
$dist=Join-Path $project 'dist'
$package=Join-Path $dist ('package-'+$Version)
if(Test-Path -LiteralPath $package){throw 'Use a new empty packaging directory; existing packages are not overwritten.'}
$null=New-Item -ItemType Directory -Path $package -Force
Copy-Item -LiteralPath $baseFile.FullName -Destination (Join-Path $package 'Jade-Palworld-Rust.exe')
Copy-Item -LiteralPath (Join-Path $project 'data') -Destination $package -Recurse
Copy-Item -LiteralPath (Join-Path $project 'LICENSE') -Destination $package
Copy-Item -LiteralPath (Join-Path $project 'THIRD_PARTY.md') -Destination $package
Copy-Item -LiteralPath (Join-Path $project 'README.md') -Destination $package
if(Test-Path (Join-Path $project 'licenses')){Copy-Item -LiteralPath (Join-Path $project 'licenses') -Destination $package -Recurse}
$files=@(Get-ChildItem -LiteralPath $package -Recurse -File | Sort-Object FullName | ForEach-Object {
 [ordered]@{path=[IO.Path]::GetRelativePath($package,$_.FullName).Replace('\','/');size=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()}
})
$manifest=[ordered]@{schema=1;version=$Version;files=$files}
$encoding=[Text.UTF8Encoding]::new($false)
[IO.File]::WriteAllText((Join-Path $package 'rust-package.json'),(($manifest|ConvertTo-Json -Depth 20).Replace("`r`n","`n")),$encoding)
Add-Type -AssemblyName System.IO.Compression
$archive=Join-Path $dist ('Jade-Palworld-Rust-'+$Version+'-windows-x64.zip')
$stream=[IO.File]::Create($archive)
$zip=[IO.Compression.ZipArchive]::new($stream,[IO.Compression.ZipArchiveMode]::Create)
try{foreach($file in (Get-ChildItem -LiteralPath $package -Recurse -File)){
 $entry=$zip.CreateEntry([IO.Path]::GetRelativePath($package,$file.FullName).Replace('\','/'),[IO.Compression.CompressionLevel]::Optimal)
 $output=$entry.Open();$input=[IO.File]::OpenRead($file.FullName)
 try{$input.CopyTo($output)}finally{$input.Dispose();$output.Dispose()}
}}finally{$zip.Dispose();$stream.Dispose()}
$tag='rust-v'+$Version
$update=[ordered]@{schema=1;version=$Version;releaseTag=$tag;package=$manifest;full=[ordered]@{name=[IO.Path]::GetFileName($archive);url=('https://github.com/S1T2J3/Jade-Palworld-Toolbox/releases/download/'+$tag+'/'+[IO.Path]::GetFileName($archive));size=(Get-Item -LiteralPath $archive).Length;sha256=(Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()};deltas=@()}
if($PreviousPackage){
 $previousRoot=(Resolve-Path -LiteralPath $PreviousPackage).Path
 $previousFile=Join-Path $previousRoot 'rust-package.json'
 $previous=Get-Content -Raw -LiteralPath $previousFile | ConvertFrom-Json
 $old=@{};foreach($file in $previous.files){$old[$file.path]=$file}
 $changed=@($files | Where-Object {-not $old.ContainsKey($_.path) -or $old[$_.path].sha256 -ne $_.sha256 -or $old[$_.path].size -ne $_.size})
 $deltaName='Jade-Palworld-Rust-'+$Version+'-from-'+$previous.version+'-delta.zip'
 $deltaPath=Join-Path $dist $deltaName
 $stream=[IO.File]::Create($deltaPath);$zip=[IO.Compression.ZipArchive]::new($stream,[IO.Compression.ZipArchiveMode]::Create)
 try{foreach($relative in @('rust-package.json')+@($changed|ForEach-Object {$_.path})){
  $entry=$zip.CreateEntry($relative,[IO.Compression.CompressionLevel]::Optimal)
  $output=$entry.Open();$input=[IO.File]::OpenRead((Join-Path $package $relative))
  try{$input.CopyTo($output)}finally{$input.Dispose();$output.Dispose()}
 }}finally{$zip.Dispose();$stream.Dispose()}
 $update.deltas=@([ordered]@{fromManifestSha256=(Get-FileHash -LiteralPath $previousFile -Algorithm SHA256).Hash.ToLowerInvariant();name=$deltaName;url=('https://github.com/S1T2J3/Jade-Palworld-Toolbox/releases/download/'+$tag+'/'+$deltaName);size=(Get-Item -LiteralPath $deltaPath).Length;sha256=(Get-FileHash -LiteralPath $deltaPath -Algorithm SHA256).Hash.ToLowerInvariant()})
}
[IO.File]::WriteAllText((Join-Path $dist 'rust-update.json'),($update|ConvertTo-Json -Depth 20),$encoding)
if($InnoCompiler){& $InnoCompiler ('/DPackageDir='+$package) ('/DDistDir='+$dist) ('/DVersion='+$Version) (Join-Path $project 'installer.iss');if($LASTEXITCODE-ne 0){throw 'Installer build failed'}}
$assets=@($archive, (Join-Path $dist 'rust-update.json'))
if($PreviousPackage){$assets+=$deltaPath}
if($InnoCompiler){$assets+=Join-Path $dist ('Jade-Palworld-Rust-'+$Version+'-Setup.exe')}
if ($KernelExecutable) {
    $kernelName = 'Jade-Pal-Assistant-Kernel.exe'
    $kernelManifest = [ordered]@{
        schema = 1
        id = 'pal-assistant'
        hostVersion = $Version
        executable = $kernelName
        sha256 = $kernelHash
    }
    $kernelArchive = Join-Path $dist ('Jade-Pal-Assistant-Kernel-'+$Version+'-windows-x64.zip')
    $stream = [IO.File]::Open($kernelArchive, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
    try {
        $zip = [IO.Compression.ZipArchive]::new($stream, [IO.Compression.ZipArchiveMode]::Create, $true)
        try {
            $entry = $zip.CreateEntry('kernel.json')
            $writer = [IO.StreamWriter]::new($entry.Open(), $encoding)
            try { $writer.Write(($kernelManifest | ConvertTo-Json -Compress)) } finally { $writer.Dispose() }
            $noticeName = 'Palworld-Knowledge-MIT.txt'
            $notice = Get-Item -LiteralPath (Join-Path $project "licenses/$noticeName")
            $entry = $zip.CreateEntry($noticeName, [IO.Compression.CompressionLevel]::Optimal)
            $output = $entry.Open()
            try {
                $input = $notice.OpenRead()
                try { $input.CopyTo($output) } finally { $input.Dispose() }
            } finally { $output.Dispose() }
            $entry = $zip.CreateEntry($kernelName, [IO.Compression.CompressionLevel]::Optimal)
            $output = $entry.Open()
            try {
                $input = $kernelFile.OpenRead()
                try { $input.CopyTo($output) } finally { $input.Dispose() }
            } finally { $output.Dispose() }
        } finally { $zip.Dispose() }
    } finally { $stream.Dispose() }
    $assets += $kernelArchive
}
$hashes=$assets | Sort-Object | ForEach-Object { (Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant()+'  '+[IO.Path]::GetFileName($_) }
$checksumFile=Join-Path $dist 'SHA256SUMS.txt'
[IO.File]::WriteAllLines($checksumFile,$hashes,$encoding)
& (Join-Path $project '../../scripts/archive-release.ps1') -Version $Version -Files ($assets+$checksumFile)
Write-Output $archive
