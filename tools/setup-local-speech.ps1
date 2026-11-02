$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$speechRoot = Join-Path $repo '.local\whisper'
$source = Join-Path $speechRoot 'source'
$build = Join-Path $speechRoot 'build'
$model = Join-Path $speechRoot 'models\ggml-base.en.bin'
$expectedCommit = 'd1be6fde11ac6e0407606b4e42fe72d34add8037'
$expectedModelSha1 = '137C40403D78FD54D454DA0F9BD998F78703390C'
New-Item -ItemType Directory -Force (Split-Path $model -Parent) | Out-Null
if (!(Test-Path -LiteralPath $source)) {
    git clone --depth 1 --branch v1.9.5 https://github.com/ggml-org/whisper.cpp.git $source
    if ($LASTEXITCODE -ne 0) { throw 'Could not download the official recognizer source.' }
}
$actualCommit = git -C $source rev-parse HEAD
if ($actualCommit -ne $expectedCommit) { throw 'Existing recognizer source differs from the reviewed version. No files were replaced.' }
$cmakeCommand = Get-Command cmake -ErrorAction SilentlyContinue
$cmakePath = if ($cmakeCommand) { $cmakeCommand.Source } else { $null }
if (!$cmakePath) {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (Test-Path -LiteralPath $vswhere) {
        $installation = & $vswhere -latest -products '*' -property installationPath
        if ($installation) { $cmakePath = Join-Path $installation 'Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe' }
    }
}
if (!$cmakePath -or !(Test-Path -LiteralPath $cmakePath)) { throw 'Install Visual Studio C++ build tools with CMake before running this helper.' }
& $cmakePath -S $source -B $build -A x64 -DWHISPER_BUILD_TESTS=OFF -DGGML_OPENMP=OFF
if ($LASTEXITCODE -ne 0) { throw 'Recognizer configuration failed.' }
& $cmakePath --build $build --config Release --target whisper-cli --parallel 6
if ($LASTEXITCODE -ne 0) { throw 'Recognizer build failed.' }
if (!(Test-Path -LiteralPath $model)) {
    Invoke-WebRequest 'https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin' -OutFile $model
}
if ((Get-FileHash -LiteralPath $model -Algorithm SHA1).Hash -ne $expectedModelSha1) { throw 'Model differs from the checksum in the official whisper.cpp manifest.' }
Write-Output 'Local recognizer built and model checksum verified. No audio was sent to an external service.'
Write-Output ('CLI: ' + (Join-Path $build 'bin\Release\whisper-cli.exe'))
Write-Output ('Model: ' + $model)
