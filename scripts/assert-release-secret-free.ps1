[CmdletBinding()]
param(
  [Parameter(Mandatory)]
  [ValidateNotNullOrEmpty()]
  [string]$InstallerPath
)

$resolvedInstaller = (Resolve-Path -LiteralPath $InstallerPath -ErrorAction Stop).Path
if ([IO.Path]::GetExtension($resolvedInstaller) -ne '.exe') {
  throw "Only an NSIS .exe installer can be scanned: $resolvedInstaller"
}

$sevenZip = Get-Command 7z.exe -ErrorAction SilentlyContinue
if ($null -eq $sevenZip) {
  throw '7z.exe is required to inspect the NSIS installer contents.'
}

$rules = [ordered]@{
  'OpenAI-style token' = '(?<![A-Za-z0-9_-])sk-(?:proj-)?[A-Za-z0-9_-]{20,}'
  'Anthropic token' = '\bsk-ant-[A-Za-z0-9_-]{20,}'
  'GitHub token' = '\b(?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{20,}\b|\bgithub_pat_[A-Za-z0-9_]{20,}\b'
  'AWS access key id' = '\b(?:AKIA|ASIA)[A-Z0-9]{16}\b'
  'Google API key' = '\bAIza[A-Za-z0-9_-]{30,}\b'
  'Slack token' = '\bxox(?:b|p|a|r|s)-[A-Za-z0-9-]{10,}\b'
  'Stripe secret key' = '\bsk_(?:live|test)_[A-Za-z0-9]{16,}\b'
  'Private key block' = '-----BEGIN (?:[A-Z ]+ )?PRIVATE KEY-----'
  'JWT-like token' = '\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\b'
}

$scanRoot = Join-Path ([IO.Path]::GetTempPath()) ("agent-switchboard-release-scan-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $scanRoot -ErrorAction Stop | Out-Null

try {
  & $sevenZip.Source x -y "-o$scanRoot" $resolvedInstaller | Out-Null
  if ($LASTEXITCODE -ne 0) {
    throw "7z.exe could not extract the installer (exit code $LASTEXITCODE)."
  }

  $findings = [System.Collections.Generic.List[object]]::new()
  foreach ($file in @(Get-ChildItem -LiteralPath $scanRoot -Recurse -File)) {
    $content = [Text.Encoding]::Latin1.GetString([IO.File]::ReadAllBytes($file.FullName))
    foreach ($entry in $rules.GetEnumerator()) {
      foreach ($match in [regex]::Matches($content, $entry.Value)) {
        $findings.Add([pscustomobject]@{
          Rule = $entry.Key
          Path = $file.FullName.Substring($scanRoot.Length + 1)
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

  "Release artifact credential scan passed: $([IO.Path]::GetFileName($resolvedInstaller))"
}
finally {
  if (Test-Path -LiteralPath $scanRoot) {
    [IO.Directory]::Delete($scanRoot, $true)
  }
}
