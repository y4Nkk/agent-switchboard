[CmdletBinding()]
param(
  [Parameter(ValueFromRemainingArguments = $true)]
  [string[]]$TauriArguments
)

$ErrorActionPreference = 'Stop'
if (-not $IsWindows) { throw 'The Windows installer must be built on Windows.' }
$repository = Split-Path $PSScriptRoot -Parent
$framework = Join-Path $env:WINDIR 'Microsoft.NET/Framework64/v4.0.30319'
$compiler = Join-Path $framework 'csc.exe'
if (-not (Test-Path -LiteralPath $compiler -PathType Leaf)) {
  throw 'The Windows .NET Framework C# compiler is required.'
}

Push-Location $repository
try {
  $config = Get-Content -LiteralPath 'src-tauri/tauri.conf.json' -Raw | ConvertFrom-Json
  $package = Get-Content -LiteralPath 'package.json' -Raw | ConvertFrom-Json
  $version = (& node scripts/updater-release.mjs workspace-version --cargo-manifest Cargo.toml).Trim()
  if ($LASTEXITCODE -ne 0) { throw 'Could not read the workspace version.' }
  $targetRoot = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { 'target' }
  $bundleRoot = [IO.Path]::GetFullPath((Join-Path $targetRoot 'release/bundle'))
  & node node_modules/@tauri-apps/cli/tauri.js build --config src-tauri/tauri.windows.conf.json @TauriArguments
  if ($LASTEXITCODE -ne 0) { throw "Tauri engine build failed (exit code $LASTEXITCODE)." }
  $engine = (Resolve-Path -LiteralPath (Join-Path $bundleRoot "nsis/$($config.productName)_${version}_x64-setup.exe")).Path
  $engineVersion = [Diagnostics.FileVersionInfo]::GetVersionInfo($engine).ProductVersion
  if ($engineVersion -ne $version) {
    throw "Installation engine version '$engineVersion' does not match workspace '$version'."
  }
  $outputDirectory = Join-Path $bundleRoot 'installer'
  New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null
  $output = Join-Path $outputDirectory "$($config.productName)_${version}_x64-setup.exe"
  $metadata = Join-Path $outputDirectory 'Package.txt'
  [IO.File]::WriteAllText($metadata, "$($config.productName)`n$version`n$($package.name)`n", [Text.UTF8Encoding]::new($false))
  $fileVersion = ($version -split '[-+]')[0] + '.0'
  $assemblyInfo = Join-Path $outputDirectory 'AssemblyInfo.cs'
  $productLiteral = $config.productName.Replace('"', '""')
  $versionInfo = @"
using System.Reflection;
[assembly: AssemblyVersion("$fileVersion")]
[assembly: AssemblyFileVersion("$fileVersion")]
[assembly: AssemblyInformationalVersion("$version")]
[assembly: AssemblyProduct(@"$productLiteral")]
"@
  [IO.File]::WriteAllText($assemblyInfo, $versionInfo, [Text.UTF8Encoding]::new($false))
  $references = @('System.dll', 'System.Core.dll', 'System.Xaml.dll', 'System.Windows.Forms.dll') |
    ForEach-Object { '/reference:' + (Join-Path $framework $_) }
  $references += @('WindowsBase.dll', 'PresentationCore.dll', 'PresentationFramework.dll') |
    ForEach-Object { '/reference:' + (Join-Path $framework "WPF/$_") }
  $sources = @(Get-ChildItem -LiteralPath 'installer' -Filter '*.cs' -File | ForEach-Object { $_.FullName })
  if ($sources.Count -eq 0) { throw 'Installer sources are missing.' }
  $sources += $assemblyInfo
  & $compiler /nologo /target:winexe /platform:x64 /optimize+ /langversion:5 `
    "/out:$output" '/win32manifest:installer/app.manifest' '/win32icon:src-tauri/icons/icon.ico' `
    "/resource:$engine,AgentSwitchboard.Installer.Engine.exe" `
    '/resource:installer/Theme.xaml,AgentSwitchboard.Installer.Theme.xaml' `
    "/resource:$metadata,AgentSwitchboard.Installer.Package.txt" @references @sources
  if ($LASTEXITCODE -ne 0) { throw "Custom installer compilation failed (exit code $LASTEXITCODE)." }
  $compiledVersion = [Diagnostics.FileVersionInfo]::GetVersionInfo($output)
  if ($compiledVersion.ProductVersion -ne $version -or $compiledVersion.FileVersion -ne $fileVersion -or $compiledVersion.ProductName -ne $config.productName) {
    throw 'Compiled installer version metadata does not match the workspace contract.'
  }
  if (Test-Path -LiteralPath "$output.sig") { Remove-Item -LiteralPath "$output.sig" }
  if ($env:TAURI_SIGNING_PRIVATE_KEY) {
    & node node_modules/@tauri-apps/cli/tauri.js signer sign $output
    if ($LASTEXITCODE -ne 0) { throw "Custom installer signing failed (exit code $LASTEXITCODE)." }
  }
  Write-Output "Custom installer: $output"
}
finally { Pop-Location }
