$ErrorActionPreference = 'Stop'

# url and checksum are replaced automatically by CI on each tagged release.
$toolsDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$url64    = 'https://github.com/egohygiene/renderflow/releases/download/v0.2.1/renderflow-x86_64-pc-windows-msvc.exe'

Get-ChocolateyWebFile -PackageName  'renderflow' `
                      -FileFullPath "$toolsDir\renderflow.exe" `
                      -Url64bit     $url64 `
                      -Checksum64   'PLACEHOLDER_SHA256' `
                      -ChecksumType64 'sha256'
