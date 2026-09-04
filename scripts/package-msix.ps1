[CmdletBinding()]
param(
  [Parameter(Mandatory)]
  [ValidateNotNullOrEmpty()]
  [string]$ExecutablePath,

  [Parameter(Mandatory)]
  [ValidateNotNullOrEmpty()]
  [string]$WebView2LoaderPath,

  [string]$CargoManifest = "Cargo.toml",

  [string]$OutputDirectory = "target/release/bundle/msix"
)

$ErrorActionPreference = 'Stop'

function Find-WindowsSdkTool {
  param(
    [Parameter(Mandatory)]
    [string]$Name
  )

  $programFilesX86 = [Environment]::GetEnvironmentVariable('ProgramFiles(x86)')
  $sdkRoot = Join-Path $programFilesX86 'Windows Kits\10\bin'
  if (-not (Test-Path -LiteralPath $sdkRoot -PathType Container)) {
    throw "Windows 10 SDK is required to create MSIX packages. Could not find $sdkRoot."
  }

  $tool = Get-ChildItem -LiteralPath $sdkRoot -Recurse -Filter $Name -File |
    Where-Object { $_.Directory.Name -eq 'x64' } |
    Sort-Object FullName -Descending |
    Select-Object -First 1
  if ($null -eq $tool) {
    throw "Windows 10 SDK is required to create MSIX packages. Could not find $Name under $sdkRoot."
  }
  $tool.FullName
}

function New-ScaledPng {
  param(
    [Parameter(Mandatory)]
    [string]$Source,
    [Parameter(Mandatory)]
    [string]$Destination,
    [Parameter(Mandatory)]
    [int]$Size
  )

  Add-Type -AssemblyName System.Drawing
  $sourceImage = [Drawing.Image]::FromFile($Source)
  $bitmap = [Drawing.Bitmap]::new($Size, $Size)
  $graphics = [Drawing.Graphics]::FromImage($bitmap)
  try {
    $graphics.Clear([Drawing.Color]::Transparent)
    $graphics.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $graphics.DrawImage($sourceImage, 0, 0, $Size, $Size)
    $bitmap.Save($Destination, [Drawing.Imaging.ImageFormat]::Png)
  }
  finally {
    $graphics.Dispose()
    $bitmap.Dispose()
    $sourceImage.Dispose()
  }
}

$resolvedExecutable = (Resolve-Path -LiteralPath $ExecutablePath -ErrorAction Stop).Path
$resolvedLoader = (Resolve-Path -LiteralPath $WebView2LoaderPath -ErrorAction Stop).Path
$resolvedCargoManifest = (Resolve-Path -LiteralPath $CargoManifest -ErrorAction Stop).Path
$repositoryRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
$identityPath = Join-Path $repositoryRoot 'src-tauri\windows\store-identity.json'
$iconPath = Join-Path $repositoryRoot 'src-tauri\icons\icon.png'
$msixScript = Join-Path $PSScriptRoot 'msix-release.mjs'
$version = (& node (Join-Path $PSScriptRoot 'updater-release.mjs') workspace-version --cargo-manifest $resolvedCargoManifest).Trim()
$assetName = (& node $msixScript package-name --version $version).Trim()

if ($LASTEXITCODE -ne 0) {
  throw 'Could not derive the Microsoft Store package name.'
}

$resolvedOutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
[IO.Directory]::CreateDirectory($resolvedOutputDirectory) | Out-Null
$outputPath = Join-Path $resolvedOutputDirectory $assetName
$stageRoot = Join-Path ([IO.Path]::GetTempPath()) ("agent-switchboard-msix-" + [guid]::NewGuid().ToString('N'))
$validationRoot = Join-Path ([IO.Path]::GetTempPath()) ("agent-switchboard-msix-validation-" + [guid]::NewGuid().ToString('N'))

try {
  New-Item -ItemType Directory -Path (Join-Path $stageRoot 'Assets') -Force | Out-Null
  Copy-Item -LiteralPath $resolvedExecutable -Destination (Join-Path $stageRoot 'agent-switchboard.exe')
  Copy-Item -LiteralPath $resolvedLoader -Destination (Join-Path $stageRoot 'WebView2Loader.dll')
  New-ScaledPng -Source $iconPath -Destination (Join-Path $stageRoot 'Assets\StoreLogo.png') -Size 50
  New-ScaledPng -Source $iconPath -Destination (Join-Path $stageRoot 'Assets\Square150x150Logo.png') -Size 150
  New-ScaledPng -Source $iconPath -Destination (Join-Path $stageRoot 'Assets\Square44x44Logo.png') -Size 44

  & node $msixScript manifest --identity $identityPath --output (Join-Path $stageRoot 'AppxManifest.xml') --version $version
  if ($LASTEXITCODE -ne 0) {
    throw 'Could not render the Microsoft Store package manifest.'
  }

  $makeAppx = Find-WindowsSdkTool -Name 'MakeAppx.exe'
  & $makeAppx pack /d $stageRoot /p $outputPath /o
  if ($LASTEXITCODE -ne 0) {
    throw "MakeAppx could not create $outputPath."
  }
  & $makeAppx unpack /p $outputPath /d $validationRoot /o
  if ($LASTEXITCODE -ne 0) {
    throw "MakeAppx could not unpack $outputPath for validation."
  }

  $requiredPackageFiles = @(
    'AppxManifest.xml',
    'agent-switchboard.exe',
    'WebView2Loader.dll',
    'Assets\StoreLogo.png',
    'Assets\Square150x150Logo.png',
    'Assets\Square44x44Logo.png'
  )
  $missingPackageFiles = @($requiredPackageFiles | Where-Object {
    -not (Test-Path -LiteralPath (Join-Path $validationRoot $_) -PathType Leaf)
  })
  if ($missingPackageFiles.Count -gt 0) {
    throw "MSIX validation failed; package is missing: $($missingPackageFiles -join ', ')."
  }
  "Created Microsoft Store package: $outputPath"
}
finally {
  if (Test-Path -LiteralPath $stageRoot) {
    [IO.Directory]::Delete($stageRoot, $true)
  }
  if (Test-Path -LiteralPath $validationRoot) {
    [IO.Directory]::Delete($validationRoot, $true)
  }
}
