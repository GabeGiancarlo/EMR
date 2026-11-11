$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$mediaRoot = Join-Path $repo '.local\media-tools'
$archive = Join-Path $repo '.local\ffmpeg-9.0.2-essentials.zip'
$cli = Join-Path $mediaRoot 'ffmpeg-9.0.2-essentials_build\bin\ffmpeg.exe'
$sha256 = '60F467265B1E312373DBCD92200C2618A74850F98D3D078E94296BB3FA2047BA'
New-Item -ItemType Directory -Force $mediaRoot | Out-Null
if (!(Test-Path -LiteralPath $archive)) {
    Invoke-WebRequest 'https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-9.0.2-essentials_build.zip' -OutFile $archive
}
if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne $sha256) {
    throw 'The decoder archive differs from the reviewed checksum. No archive was extracted.'
}
if (!(Test-Path -LiteralPath $cli -PathType Leaf)) { Expand-Archive -LiteralPath $archive -DestinationPath $mediaRoot }
if (!(Test-Path -LiteralPath $cli -PathType Leaf)) { throw 'The expected decoder executable was not found.' }
Write-Output 'Pinned local decoder archive verified. Restart the API to enable recorded-media processing.'
