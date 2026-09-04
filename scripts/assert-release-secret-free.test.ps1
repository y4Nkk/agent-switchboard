[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$scanner = Join-Path $PSScriptRoot 'assert-release-secret-free.ps1'
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ("agent-switchboard-release-scan-test-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $testRoot | Out-Null

function Invoke-Scanner {
  param(
    [Parameter(Mandatory)]
    [string]$ArtifactPath
  )

  & $scanner -ArtifactPath $ArtifactPath
}

function Assert-ScannerRejects {
  param(
    [Parameter(Mandatory)]
    [string]$ArtifactPath,
    [string]$SecretShape
  )

  try {
    Invoke-Scanner $ArtifactPath
    throw "Scanner accepted the fixture: $ArtifactPath"
  }
  catch {
    $message = $_ | Out-String
    if ($message -notmatch 'Release artifact contains high-confidence credential-shaped data') {
      throw
    }
    if ($SecretShape -and $message.Contains($SecretShape)) {
      throw 'Scanner output exposed a matched credential-shaped value.'
    }
  }
}

function New-TestDmg {
  param(
    [Parameter(Mandatory)]
    [string]$Source,
    [Parameter(Mandatory)]
    [string]$ArtifactPath
  )

  & hdiutil create -quiet -ov -srcfolder $Source -format UDZO $ArtifactPath
  if ($LASTEXITCODE -ne 0) {
    throw "hdiutil could not create the test disk image (exit code $LASTEXITCODE)."
  }
}

function Get-SystemLibrary {
  $line = @(& ldconfig -p 2>$null) |
    Where-Object { $_ -match '^\s*libgnutls\.so\.30\s+\([^)]+\)\s+=>\s+(?<path>.+)$' } |
    Select-Object -First 1
  if ($null -eq $line) {
    throw 'The Linux fixture requires libgnutls.so.30 in the system linker cache.'
  }
  $line -match '=>\s+(?<path>.+)$' | Out-Null
  $Matches.path.Trim()
}

function New-TestAppImage {
  param(
    [Parameter(Mandatory)]
    [string]$ArtifactPath,
    [Parameter(Mandatory)]
    [string]$SystemLibrary,
    [switch]$AppendByte,
    [switch]$AppendPrivateKey
  )

  $quotedLibrary = $SystemLibrary.Replace("'", "'\''")
  $mutation = if ($AppendPrivateKey) {
    "printf '\n-----BEGIN PRIVATE KEY-----\n$([string]::new('A', 64))\n-----END PRIVATE KEY-----\n' >> squashfs-root/usr/lib/libgnutls.so.30"
  } elseif ($AppendByte) {
    'printf x >> squashfs-root/usr/lib/libgnutls.so.30'
  } else {
    ':'
  }
  $fixture = @'
#!/bin/sh
if [ "$1" = "--appimage-extract" ]; then
  mkdir -p squashfs-root/usr/lib
  cp '__SYSTEM_LIBRARY__' squashfs-root/usr/lib/libgnutls.so.30
  __MUTATION__
fi
'@
  [IO.File]::WriteAllText(
    $ArtifactPath,
    $fixture.Replace('__SYSTEM_LIBRARY__', $quotedLibrary).Replace('__MUTATION__', $mutation),
    [Text.UTF8Encoding]::new($false)
  )
  & chmod +x $ArtifactPath
  if ($LASTEXITCODE -ne 0) {
    throw "chmod could not prepare the AppImage fixture (exit code $LASTEXITCODE)."
  }
}

try {
  if ($IsMacOS) {
    $source = Join-Path $testRoot 'source'
    $payload = Join-Path $source 'Agent Switchboard.app/Contents/Resources/payload.txt'
    New-Item -ItemType Directory -Path (Split-Path -Parent $payload) -Force | Out-Null
    [IO.File]::WriteAllText($payload, 'ordinary package content')
    New-Item -ItemType SymbolicLink -Path (Join-Path $source 'Applications') -Target '/Applications' | Out-Null
    $artifact = Join-Path $testRoot 'fixture.dmg'

    New-TestDmg $source $artifact
    Invoke-Scanner $artifact

    $syntheticKeyBlock = "-----BEGIN PRIVATE KEY-----`n$([string]::new('A', 64))`n-----END PRIVATE KEY-----"
    [IO.File]::WriteAllText($payload, $syntheticKeyBlock)
    New-TestDmg $source $artifact
    Assert-ScannerRejects $artifact $syntheticKeyBlock
  }
  elseif ($IsLinux) {
    $systemLibrary = Get-SystemLibrary
    $artifact = Join-Path $testRoot 'fixture.AppImage'

    New-TestAppImage $artifact $systemLibrary
    Invoke-Scanner $artifact

    New-TestAppImage $artifact $systemLibrary -AppendByte
    Invoke-Scanner $artifact

    New-TestAppImage $artifact $systemLibrary -AppendPrivateKey
    Assert-ScannerRejects $artifact
  }
  else {
    throw 'Release artifact scanner fixtures require macOS or Linux.'
  }

  'Release artifact scanner fixtures passed.'
}
finally {
  if (Test-Path -LiteralPath $testRoot) {
    [IO.Directory]::Delete($testRoot, $true)
  }
}
