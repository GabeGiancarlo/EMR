$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$keyDirectory = Join-Path $repo '.local\keys'
$keyFile = Join-Path $keyDirectory 'media-v1.key'
if (!$env:NEXUS_MEDIA_KEY_HEX) {
    New-Item -ItemType Directory -Force -Path $keyDirectory | Out-Null
    # Protect the key before writing it. Database backups intentionally omit it.
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent().User
    $acl = New-Object Security.AccessControl.DirectorySecurity
    $acl.SetAccessRuleProtection($true, $false)
    $inheritance = [Security.AccessControl.InheritanceFlags]'ContainerInherit,ObjectInherit'
    $propagation = [Security.AccessControl.PropagationFlags]::None
    $allow = [Security.AccessControl.AccessControlType]::Allow
    $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($identity, 'FullControl', $inheritance, $propagation, $allow))
    $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new([Security.Principal.SecurityIdentifier]::new('S-1-5-18'), 'FullControl', $inheritance, $propagation, $allow))
    $directoryInfo = [IO.DirectoryInfo]::new($keyDirectory)
    if ('System.IO.FileSystemAclExtensions' -as [type]) { [IO.FileSystemAclExtensions]::SetAccessControl($directoryInfo, $acl) }
    else { $directoryInfo.SetAccessControl($acl) }
    if (!(Test-Path -LiteralPath $keyFile -PathType Leaf)) {
        $keyBytes = New-Object byte[] 32
        $generator = [Security.Cryptography.RandomNumberGenerator]::Create()
        try { $generator.GetBytes($keyBytes) } finally { $generator.Dispose() }
        $keyStream = [IO.File]::Open($keyFile, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
        try { $keyStream.Write($keyBytes, 0, $keyBytes.Length); $keyStream.Flush($true) } finally { $keyStream.Dispose() }
        [Array]::Clear($keyBytes, 0, $keyBytes.Length)
        Write-Output 'Created the protected local media key. Keep a separate protected recovery copy; database backups do not contain this key.'
    }
    $fileAcl = New-Object Security.AccessControl.FileSecurity
    $fileAcl.SetAccessRuleProtection($true, $false)
    $fileAcl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($identity, 'FullControl', $allow))
    $fileAcl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new([Security.Principal.SecurityIdentifier]::new('S-1-5-18'), 'FullControl', $allow))
    $fileInfo = [IO.FileInfo]::new($keyFile)
    if ('System.IO.FileSystemAclExtensions' -as [type]) { [IO.FileSystemAclExtensions]::SetAccessControl($fileInfo, $fileAcl) }
    else { $fileInfo.SetAccessControl($fileAcl) }
    $savedKey = [IO.File]::ReadAllBytes($keyFile)
    if ($savedKey.Length -ne 32) { throw 'Local media key has an invalid length. It was not replaced.' }
    $env:NEXUS_MEDIA_KEY_HEX = [BitConverter]::ToString($savedKey).Replace('-', '').ToLowerInvariant()
    [Array]::Clear($savedKey, 0, $savedKey.Length)
}
if ($env:NEXUS_MEDIA_KEY_HEX -notmatch '^[a-fA-F0-9]{64}$') { throw 'NEXUS_MEDIA_KEY_HEX must encode exactly 32 bytes. The value was not printed.' }
Write-Output 'Local media vault encryption configured.'
