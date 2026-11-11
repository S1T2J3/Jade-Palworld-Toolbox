param([string]$ContentDirectory)
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
if(-not $ContentDirectory){
 $ContentDirectory=Join-Path $root 'build/rust-data-source'
 & node (Join-Path $root 'work/build.cjs') --desktop $ContentDirectory
 if($LASTEXITCODE-ne 0){throw 'Data preparation failed'}
}
& node (Join-Path $PSScriptRoot 'export-data.cjs') $ContentDirectory
if($LASTEXITCODE-ne 0){throw 'Native data export failed'}
if(-not $env:RUSTFLAGS){$env:RUSTFLAGS='-C linker=rust-lld.exe -C linker-flavor=ld.lld'}
& cargo build --release --locked --manifest-path (Join-Path $PSScriptRoot 'Cargo.toml')
if($LASTEXITCODE-ne 0){throw 'Rust build failed'}
