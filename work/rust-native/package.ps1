param([string]$Version='2.0.0-beta.1',[string]$InnoCompiler)
$ErrorActionPreference='Stop'
$project=[IO.Path]::GetFullPath($PSScriptRoot)
$dist=Join-Path $project 'dist'
$package=Join-Path $dist ('package-'+$Version)
if(Test-Path -LiteralPath $package){throw 'Use a new empty packaging directory; existing packages are not overwritten.'}
$null=New-Item -ItemType Directory -Path $package -Force
Copy-Item -LiteralPath (Join-Path $project 'target/release/Jade-Palworld-Rust.exe') -Destination $package
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
[IO.File]::WriteAllText((Join-Path $dist 'rust-update.json'),($update|ConvertTo-Json -Depth 20),$encoding)
if($InnoCompiler){& $InnoCompiler ('/DPackageDir='+$package) ('/DDistDir='+$dist) ('/DVersion='+$Version) (Join-Path $project 'installer.iss');if($LASTEXITCODE-ne 0){throw 'Installer build failed'}}
$hashes=Get-ChildItem -LiteralPath $dist -File | Where-Object {$_.Extension -in '.zip','.exe','.json'} | ForEach-Object { (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()+'  '+$_.Name }
[IO.File]::WriteAllLines((Join-Path $dist 'SHA256SUMS.txt'),$hashes,$encoding)
Write-Output $archive
