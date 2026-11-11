$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$runtimeRoot = Join-Path $repo '.local\llama'
$modelRoot = Join-Path $repo '.local\models'
New-Item -ItemType Directory -Force $runtimeRoot,$modelRoot | Out-Null
$assets = @(
    @{ Path = (Join-Path $runtimeRoot 'llama-b12058-bin-win-vulkan-x64.zip'); Url = 'https://github.com/ggml-org/llama.cpp/releases/download/b12058/llama-b12058-bin-win-vulkan-x64.zip'; Sha = 'FBAB055F598DCD628F773DB531741C64D14C652E7EEFB2DFEBC6953E8BCAF197' },
    @{ Path = (Join-Path $modelRoot 'Qwen3-4B-Q4_K_M.gguf'); Url = 'https://huggingface.co/Qwen/Qwen3-4B-GGUF/resolve/bc640142c66e1fdd12af0bd68f40445458f3869b/Qwen3-4B-Q4_K_M.gguf'; Sha = '7485FE6F11AF29433BC51CAB58009521F205840F5B4AE3A32FA7F92E8534FDF5' }
)
foreach ($asset in $assets) {
    if (!(Test-Path -LiteralPath $asset.Path -PathType Leaf)) {
        & curl.exe --fail --location --silent --show-error --output $asset.Path $asset.Url
        if ($LASTEXITCODE -ne 0) { throw 'Local model/runtime download failed. A partial file may remain; verify it before retrying.' }
    }
    if ((Get-FileHash -LiteralPath $asset.Path -Algorithm SHA256).Hash -ne $asset.Sha) { throw 'A local drafting artifact differs from its pinned checksum. It was not executed or replaced.' }
}
$destination = Join-Path $runtimeRoot 'b12058'
if (!(Test-Path -LiteralPath (Join-Path $destination 'llama-server.exe') -PathType Leaf)) {
    Expand-Archive -LiteralPath $assets[0].Path -DestinationPath $destination
}
Write-Output 'Pinned llama.cpp runtime and official Qwen GGUF verified. Use start-local-model.ps1, then restart the API.'
