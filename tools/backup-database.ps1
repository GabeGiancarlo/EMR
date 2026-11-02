param([string]$Database='nexus')
$ErrorActionPreference='Stop'
if ($Database -notmatch '^nexus(_[a-z0-9_]+)?$') { throw 'Expected a local Nexus database name' }
$repo=Split-Path $PSScriptRoot -Parent
$bin=Join-Path $repo '.local\postgres\pgsql\bin'
$folder=Join-Path $repo '.local\backups'
New-Item -ItemType Directory -Force $folder | Out-Null
$file=Join-Path $folder ($Database+'-'+(Get-Date -Format 'yyyyMMdd-HHmmss')+'-'+[guid]::NewGuid().ToString('N').Substring(0,8)+'.dump')
$previousDatabasePassword=$env:PGPASSWORD
$env:PGPASSWORD='nexus_local'
try {
    & "$bin\pg_dump.exe" -h 127.0.0.1 -p 55432 -U nexus -d $Database -Fc -f $file
    if ($LASTEXITCODE -ne 0) { throw 'Database backup failed' }
    @{database=$Database;created_at=[DateTime]::UtcNow.ToString('o');sha256=(Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash} | ConvertTo-Json | Set-Content ($file+'.json')
    Write-Output $file
} finally {
    if ($null -eq $previousDatabasePassword) { Remove-Item Env:\PGPASSWORD -ErrorAction SilentlyContinue }
    else { $env:PGPASSWORD=$previousDatabasePassword }
}
