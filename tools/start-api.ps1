param([int]$Port = 8090)
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
$env:NEXUS_LOCAL_DEV = '1'
$env:HOST = '127.0.0.1'
$env:PORT = [string]$Port
if (!$env:DATABASE_URL) { $env:DATABASE_URL = 'postgres://nexus:nexus_local@127.0.0.1:55432/nexus' }
# The optional recognizer remains local and can be overridden with explicit paths.
$localSpeechCli = Join-Path (Get-Location) '.local\whisper\build\bin\Release\whisper-cli.exe'
$localSpeechModel = Join-Path (Get-Location) '.local\whisper\models\ggml-base.en.bin'
if (!$env:NEXUS_WHISPER_CLI -and !$env:NEXUS_WHISPER_MODEL -and
    (Test-Path -LiteralPath $localSpeechCli -PathType Leaf) -and
    (Test-Path -LiteralPath $localSpeechModel -PathType Leaf)) {
    $env:NEXUS_WHISPER_CLI = $localSpeechCli
    $env:NEXUS_WHISPER_MODEL = $localSpeechModel
    Write-Output 'Optional local speech recognizer found.'
}
cargo build -p emr-api
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
# A separate running copy keeps Windows from locking Cargo's build output.
$runtimeDir = Join-Path (Get-Location) '.local\runtime'
New-Item -ItemType Directory -Force $runtimeDir | Out-Null
$runtimePath = Join-Path $runtimeDir ('nexus-api-' + [guid]::NewGuid().ToString('N') + '.exe')
Copy-Item -LiteralPath '.\target\debug\emr-api.exe' -Destination $runtimePath
& $runtimePath
exit $LASTEXITCODE
