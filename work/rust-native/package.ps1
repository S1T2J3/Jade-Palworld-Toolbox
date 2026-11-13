param([string]$Version='2.0.0-beta.9',[string]$InnoCompiler,[string]$PreviousPackage)
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
$hashes=Get-ChildItem -LiteralPath $dist -File | Where-Object {($_.Name -like "*$Version*" -and $_.Extension -in '.zip','.exe') -or $_.Name -eq 'rust-update.json'} | ForEach-Object { (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()+'  '+$_.Name }
[IO.File]::WriteAllLines((Join-Path $dist 'SHA256SUMS.txt'),$hashes,$encoding)
Write-Output $archive
