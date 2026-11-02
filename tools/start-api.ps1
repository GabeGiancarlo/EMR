param([int]$Port = 8090)
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
$env:NEXUS_LOCAL_DEV = '1'
$env:HOST = '127.0.0.1'
$env:PORT = [string]$Port
if (!$env:DATABASE_URL) { $env:DATABASE_URL = 'postgres://nexus:nexus_local@127.0.0.1:55432/nexus' }
& (Join-Path $PSScriptRoot 'initialize-media-vault.ps1')
# The optional recognizer remains local and can be overridden with explicit paths.
$localMediaCli = Join-Path (Get-Location) '.local\media-tools\ffmpeg-9.0.2-essentials_build\bin\ffmpeg.exe'
if (!$env:NEXUS_FFMPEG_CLI -and (Test-Path -LiteralPath $localMediaCli -PathType Leaf)) { $env:NEXUS_FFMPEG_CLI = $localMediaCli }
$localSpeechCli = Join-Path (Get-Location) '.local\whisper\build\bin\Release\whisper-cli.exe'
$localSpeechModel = Join-Path (Get-Location) '.local\whisper\models\ggml-base.en.bin'
if (!$env:NEXUS_WHISPER_CLI -and !$env:NEXUS_WHISPER_MODEL -and
    (Test-Path -LiteralPath $localSpeechCli -PathType Leaf) -and
    (Test-Path -LiteralPath $localSpeechModel -PathType Leaf)) {
    $env:NEXUS_WHISPER_CLI = $localSpeechCli
    $env:NEXUS_WHISPER_MODEL = $localSpeechModel
    Write-Output 'Optional local speech recognizer found.'
}
$localDraftModel = Join-Path (Get-Location) '.local\models\Qwen3-4B-Q4_K_M.gguf'
if (!$env:NEXUS_LLM_MODEL -and (Test-Path -LiteralPath $localDraftModel -PathType Leaf)) {
    $env:NEXUS_LLM_MODEL = $localDraftModel
    if (!$env:NEXUS_LLM_PORT) { $env:NEXUS_LLM_PORT = '8093' }
    if (!$env:NEXUS_LLM_ALIAS) { $env:NEXUS_LLM_ALIAS = 'nexus-qwen3-4b' }
    Write-Output 'Optional local drafting model configured (service health checked on use).'
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
