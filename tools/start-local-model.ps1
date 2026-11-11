param([ValidateRange(1024,65535)][int]$Port = 8093)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$cli = Join-Path $repo '.local\llama\b12058\llama-server.exe'
$model = Join-Path $repo '.local\models\Qwen3-4B-Q4_K_M.gguf'
if (!(Test-Path -LiteralPath $cli -PathType Leaf) -or !(Test-Path -LiteralPath $model -PathType Leaf)) { throw 'Run tools/setup-local-drafting.ps1 first.' }
if ((Get-FileHash -LiteralPath $model -Algorithm SHA256).Hash -ne '7485FE6F11AF29433BC51CAB58009521F205840F5B4AE3A32FA7F92E8534FDF5') { throw 'The configured model checksum changed.' }
Write-Output "Local drafting model: http://127.0.0.1:$Port (loopback only; clinical review required)."
# No UI, prompt logs, remote model downloads, tool servers or persistent prompt cache.
& $cli -m $model --alias nexus-qwen3-4b --host 127.0.0.1 --port $Port --ctx-size 8192 --parallel 1 --n-gpu-layers 99 --jinja --reasoning off --no-webui --log-disable
exit $LASTEXITCODE
