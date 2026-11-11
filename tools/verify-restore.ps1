param([Parameter(Mandatory=$true)][string]$Backup)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$bin=Join-Path $repo '.local\postgres\pgsql\bin'
$source=(Resolve-Path -LiteralPath $Backup).Path
$manifest=Get-Content -LiteralPath ($source+'.json') -Raw | ConvertFrom-Json
if ((Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash -ne $manifest.sha256) { throw 'Backup checksum does not match' }
$restoreName='nexus_restore_'+(Get-Date -Format 'yyyyMMddHHmmss')+'_'+[guid]::NewGuid().ToString('N').Substring(0,8)
$previousDatabasePassword=$env:PGPASSWORD
$env:PGPASSWORD='nexus_local'
try {
    & "$bin\createdb.exe" -h 127.0.0.1 -p 55432 -U nexus $restoreName
    if ($LASTEXITCODE -ne 0) { throw 'Could not create a new isolated restore database' }
    & "$bin\pg_restore.exe" -h 127.0.0.1 -p 55432 -U nexus -d $restoreName --exit-on-error --no-owner $source
    if ($LASTEXITCODE -ne 0) { throw 'Restore validation failed; isolated database retained for inspection' }
    & "$bin\psql.exe" -h 127.0.0.1 -p 55432 -U nexus -d $restoreName -c "SELECT count(*) AS public_tables FROM information_schema.tables WHERE table_schema='public'"
    if ($LASTEXITCODE -ne 0) { throw 'Restored data could not be inspected; isolated database retained for inspection' }
    Write-Output "Restored successfully into $restoreName. Original database unchanged."
} finally {
    if ($null -eq $previousDatabasePassword) { Remove-Item Env:\PGPASSWORD -ErrorAction SilentlyContinue }
    else { $env:PGPASSWORD=$previousDatabasePassword }
}
