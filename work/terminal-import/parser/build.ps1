$ErrorActionPreference='Stop'
$local=Join-Path $PSScriptRoot 'research/toolchain/go/bin/go.exe'
$go=if(Test-Path -LiteralPath $local){$local}else{(Get-Command go).Source}
$env:GOCACHE=Join-Path $PSScriptRoot 'research/gocache'
$env:GOTOOLCHAIN='local'
$env:GOTELEMETRY='off'
$env:GOENV='off'
$env:CGO_ENABLED='0'
Push-Location -LiteralPath (Join-Path $PSScriptRoot 'src')
try{
    & $go build -trimpath -buildvcs=false -ldflags='-s -w' -o (Join-Path $PSScriptRoot 'pal-terminal-reader.exe') ./cmd/pal-terminal-reader
    if($LASTEXITCODE -ne 0){throw 'Terminal reader compilation failed'}
}finally{Pop-Location}
