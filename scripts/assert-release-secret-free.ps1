[CmdletBinding()]
param(
  [Parameter(Mandatory)]
  [ValidateNotNullOrEmpty()]
  [string]$ArtifactPath
)

# Extracts one release artifact (.exe custom installer, .msix, .dmg, .AppImage,
# .deb or .app.tar.gz) into a temp directory and scans every contained file for
# credential-shaped data. 7z is required for .exe; the other formats use
# tools preinstalled on their building runner (hdiutil on macOS,
# ar/tar/unzstd on Linux, --appimage-extract is built into AppImages). Raw
# updater signature files are scanned directly.

$resolvedArtifact = (Resolve-Path -LiteralPath $ArtifactPath -ErrorAction Stop).Path
$extension = [IO.Path]::GetExtension($resolvedArtifact)

$rules = [ordered]@{
  'OpenAI-style token' = '(?<![A-Za-z0-9_-])sk-(?:proj-)?[A-Za-z0-9_-]{20,}'
  'Anthropic token' = '\bsk-ant-[A-Za-z0-9_-]{20,}'
  'GitHub token' = '\b(?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{20,}\b|\bgithub_pat_[A-Za-z0-9_]{20,}\b'
  'AWS access key id' = '\b(?:AKIA|ASIA)[A-Z0-9]{16}\b'
  'Google API key' = '\bAIza[A-Za-z0-9_-]{30,}\b'
  'Slack token' = '\bxox(?:b|p|a|r|s)-[A-Za-z0-9-]{10,}\b'
  'Stripe secret key' = '\bsk_(?:live|test)_[A-Za-z0-9]{16,}\b'
  # A full PEM block (header, newline, base64 body, footer) — not a bare
  # header literal: dependency libraries embed the header/footer strings as
  # adjacent assertion constants (schannel), which are not keys.
  'Private key block' = '-----BEGIN (?:[A-Z ]+ )?PRIVATE KEY-----\r?\n[A-Za-z0-9+/=\r\n]{40,}-----END (?:[A-Z ]+ )?PRIVATE KEY-----'
  'JWT-like token' = '\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\b'
}

function Test-KnownSystemPrivateKeyBlock {
  param(
    [Parameter(Mandatory)]
    [IO.FileInfo]$File,
    [Parameter(Mandatory)]
    [string]$PrivateKeyBlock
  )

  if (-not $IsLinux -or $File.Name -notmatch '\.so(?:\.\d+)*$') {
    return $false
  }

  $ldconfig = Get-Command ldconfig -ErrorAction SilentlyContinue
  if ($null -eq $ldconfig) {
    return $false
  }

  $escapedName = [regex]::Escape($File.Name)
  foreach ($line in @(& $ldconfig.Source -p 2>$null)) {
    if ($line -notmatch "^\s*$escapedName\s+\([^)]+\)\s+=>\s+(?<path>.+)$") {
      continue
    }
    $systemPath = $Matches.path.Trim()
    if (-not (Test-Path -LiteralPath $systemPath -PathType Leaf)) {
      continue
    }
    # linuxdeploy can patch an ELF's metadata after copying it. Only exempt a
    # precise PEM block when the build host's registered library contains it.
    $systemContent = [Text.Encoding]::Latin1.GetString([IO.File]::ReadAllBytes($systemPath))
    if ($systemContent.IndexOf($PrivateKeyBlock, [StringComparison]::Ordinal) -ge 0) {
      return $true
    }
  }

  return $false
}

$scanRoot = Join-Path ([IO.Path]::GetTempPath()) ("agent-switchboard-release-scan-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $scanRoot -ErrorAction Stop | Out-Null
$scanSource = $scanRoot
$mountedDmg = $null
$isMacUpdaterArchive = $resolvedArtifact.EndsWith('.app.tar.gz', [StringComparison]::OrdinalIgnoreCase)

try {
  if ($isMacUpdaterArchive) {
    & tar -xzf $resolvedArtifact -C $scanRoot
    if ($LASTEXITCODE -ne 0) {
      throw "tar could not extract the macOS updater archive (exit code $LASTEXITCODE)."
    }
  }
  else {
    switch ($extension) {
    '.exe' {
      if (-not $IsWindows) { throw 'Windows installer inspection requires Windows.' }
      $sevenZip = Get-Command 7z.exe -ErrorAction SilentlyContinue
      if ($null -eq $sevenZip) {
        $sevenZip = Get-Command 7z -ErrorAction SilentlyContinue
      }
      if ($null -eq $sevenZip) {
        throw '7z is required to inspect the NSIS installer contents.'
      }
      # Reading manifest resources does not invoke the installer's entrypoint.
      $assembly = [Reflection.Assembly]::Load([IO.File]::ReadAllBytes($resolvedArtifact))
      $resource = $assembly.GetManifestResourceStream('AgentSwitchboard.Installer.Engine.exe')
      if ($null -eq $resource) { throw 'Custom installer contains no installation engine.' }
      $enginePath = Join-Path $scanRoot 'Engine.exe'
      $engineFile = [IO.File]::Create($enginePath)
      try { $resource.CopyTo($engineFile) }
      finally { $engineFile.Dispose(); $resource.Dispose() }
      Copy-Item -LiteralPath $resolvedArtifact -Destination (Join-Path $scanRoot 'Installer.exe')
      $engineContents = Join-Path $scanRoot 'engine-contents'
      & $sevenZip.Source x -y "-o$engineContents" $enginePath | Out-Null
      if ($LASTEXITCODE -ne 0) {
        throw "7z could not extract the installer (exit code $LASTEXITCODE)."
      }
    }
    '.msix' {
      $sevenZip = Get-Command 7z.exe -ErrorAction SilentlyContinue
      if ($null -eq $sevenZip) {
        $sevenZip = Get-Command 7z -ErrorAction SilentlyContinue
      }
      if ($null -eq $sevenZip) {
        throw '7z is required to inspect the MSIX package contents.'
      }
      & $sevenZip.Source x -y "-o$scanRoot" $resolvedArtifact | Out-Null
      if ($LASTEXITCODE -ne 0) {
        throw "7z could not extract the MSIX package (exit code $LASTEXITCODE)."
      }
    }
    '.dmg' {
      $mountPoint = Join-Path $scanRoot 'mount'
      New-Item -ItemType Directory -Path $mountPoint | Out-Null
      & hdiutil attach -readonly -nobrowse -mountpoint $mountPoint $resolvedArtifact | Out-Null
      if ($LASTEXITCODE -ne 0) {
        throw "hdiutil could not mount the disk image (exit code $LASTEXITCODE)."
      }
      $mountedDmg = $mountPoint
      $scanSource = $mountPoint
    }
    '.appimage' {
      # --appimage-extract is built in and needs no FUSE; it unpacks into
      # squashfs-root below the working directory.
      $copy = Join-Path $scanRoot ([IO.Path]::GetFileName($resolvedArtifact))
      Copy-Item -LiteralPath $resolvedArtifact -Destination $copy
      & chmod +x $copy
      Push-Location $scanRoot
      try {
        & $copy --appimage-extract | Out-Null
        if ($LASTEXITCODE -ne 0) {
          throw "AppImage extraction failed (exit code $LASTEXITCODE)."
        }
      }
      finally {
        Pop-Location
      }
    }
    '.deb' {
      Push-Location $scanRoot
      try {
        & ar x $resolvedArtifact
        if ($LASTEXITCODE -ne 0) {
          throw "ar could not extract the deb package (exit code $LASTEXITCODE)."
        }
        $dataTar = Get-ChildItem -Path $scanRoot -Filter 'data.tar.*' |
          Select-Object -First 1
        if ($null -eq $dataTar) {
          throw 'deb package contains no data.tar payload.'
        }
        & tar -xf $dataTar.FullName -C $scanRoot
        if ($LASTEXITCODE -ne 0) {
          & tar --use-compress-program=unzstd -xf $dataTar.FullName -C $scanRoot
        }
        if ($LASTEXITCODE -ne 0) {
          throw "tar could not extract the deb payload (exit code $LASTEXITCODE)."
        }
      }
      finally {
        Pop-Location
      }
    }
    '.sig' {
      Copy-Item -LiteralPath $resolvedArtifact -Destination $scanRoot
    }
    default {
      throw "Unsupported artifact type '$extension'; expected .exe, .msix, .dmg, .AppImage, .deb, .app.tar.gz or .sig: $resolvedArtifact"
    }
    }
  }

  $findings = [System.Collections.Generic.List[object]]::new()
  foreach ($file in @(Get-ChildItem -LiteralPath $scanSource -Recurse -File | Where-Object {
      [string]::IsNullOrEmpty($_.LinkType)
    })) {
    $content = [Text.Encoding]::Latin1.GetString([IO.File]::ReadAllBytes($file.FullName))
    foreach ($entry in $rules.GetEnumerator()) {
      foreach ($match in [regex]::Matches($content, $entry.Value)) {
        if (
          $extension -ieq '.appimage' -and
          $entry.Key -eq 'Private key block' -and
          (Test-KnownSystemPrivateKeyBlock $file $match.Value)
        ) {
          continue
        }
        $findings.Add([pscustomobject]@{
          Rule = $entry.Key
          Path = $file.FullName.Substring($scanSource.Length + 1)
          Offset = $match.Index
          Length = $match.Length
        })
      }
    }
  }

  if ($findings.Count -gt 0) {
    $summary = $findings |
      Sort-Object Path, Offset, Rule |
      ForEach-Object { "[$($_.Rule)] $($_.Path) offset=$($_.Offset) length=$($_.Length)" }
    throw "Release artifact contains high-confidence credential-shaped data. Values are intentionally redacted.`n$($summary -join "`n")"
  }

  "Release artifact credential scan passed: $([IO.Path]::GetFileName($resolvedArtifact))"
}
finally {
  if ($null -ne $mountedDmg) {
    & hdiutil detach $mountedDmg -Force | Out-Null
  }
  if (Test-Path -LiteralPath $scanRoot) {
    [IO.Directory]::Delete($scanRoot, $true)
  }
}
