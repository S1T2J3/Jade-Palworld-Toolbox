param(
    [switch]$Offline,
    [string]$TargetDirectory = (Join-Path $PSScriptRoot 'target')
)
$ErrorActionPreference = 'Stop'
$target = [IO.Path]::GetFullPath($TargetDirectory)
$flags = @('build', '--release', '--locked', '--no-default-features',
    '--manifest-path', (Join-Path $PSScriptRoot 'Cargo.toml'),
    '--target-dir', $target, '--package', 'jade-app')
if ($Offline) { $flags += '--offline' }

$kernelPath = Join-Path $target 'release/Jade-Pal-Assistant-Kernel.exe'
$basePath = Join-Path $target 'release/Jade-Palworld-Rust.exe'
$previousPin = $env:JADE_ASSISTANT_KERNEL_SHA256
$previousFlags = $env:RUSTFLAGS
try {
    if (!$env:RUSTFLAGS) { $env:RUSTFLAGS = '-C linker=rust-lld.exe -C linker-flavor=ld.lld' }
    $env:JADE_ASSISTANT_KERNEL_SHA256 = $null
    & cargo @flags --features assistant-kernel --bin Jade-Pal-Assistant-Kernel
    if ($LASTEXITCODE -ne 0) { throw 'Assistant kernel build failed.' }
    $pin = (Get-FileHash -Algorithm SHA256 -LiteralPath $kernelPath).Hash.ToLowerInvariant()
    $env:JADE_ASSISTANT_KERNEL_SHA256 = $pin
    & cargo @flags --bin Jade-Palworld-Rust
    if ($LASTEXITCODE -ne 0) { throw 'Base application build failed.' }
} finally {
    $env:JADE_ASSISTANT_KERNEL_SHA256 = $previousPin
    $env:RUSTFLAGS = $previousFlags
}
[PSCustomObject]@{BaseExecutable=$basePath; KernelExecutable=$kernelPath; KernelSHA256=$pin}
