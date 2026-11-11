param([switch]$Stop)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$dbRoot = Join-Path $repo '.local\postgres'
$bin = Join-Path $dbRoot 'pgsql\bin'
$data = Join-Path $dbRoot 'data'
if (!(Test-Path (Join-Path $bin 'pg_ctl.exe'))) {
    throw 'Local PostgreSQL binaries not installed. Use docker compose -f compose.local.yml up -d, or extract the EDB Windows PostgreSQL 17 binaries ZIP under .local/postgres.'
}
if ($Stop) { & "$bin\pg_ctl.exe" -D $data stop -m fast; exit $LASTEXITCODE }
if (!(Test-Path (Join-Path $data 'PG_VERSION'))) {
    $passwordFile = Join-Path $dbRoot 'init-password.local'
    try {
        [IO.File]::WriteAllText($passwordFile, 'nexus_local')
        & "$bin\initdb.exe" -D $data -U nexus --pwfile=$passwordFile --auth=scram-sha-256 --encoding=UTF8 --locale=C
        if ($LASTEXITCODE -ne 0) { throw 'PostgreSQL initialization failed' }
    } finally { if (Test-Path $passwordFile) { Remove-Item -LiteralPath $passwordFile } }
}
& "$bin\pg_ctl.exe" -D $data status *> $null
if ($LASTEXITCODE -ne 0) {
    & "$bin\pg_ctl.exe" -D $data -l (Join-Path $dbRoot 'server.log') -o '-h 127.0.0.1 -p 55432' start
    if ($LASTEXITCODE -ne 0) { throw 'PostgreSQL start failed' }
}
$env:PGPASSWORD = 'nexus_local'
try {
    $exists = & "$bin\psql.exe" -h 127.0.0.1 -p 55432 -U nexus -d postgres -tAc "SELECT 1 FROM pg_database WHERE datname='nexus'"
    if ($exists -ne '1') { & "$bin\createdb.exe" -h 127.0.0.1 -p 55432 -U nexus nexus }
} finally { Remove-Item Env:\PGPASSWORD }
Write-Output 'Nexus PostgreSQL available on 127.0.0.1:55432. Database: nexus. Local development only.'
