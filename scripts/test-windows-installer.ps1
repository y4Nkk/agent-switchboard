[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$repository = Split-Path $PSScriptRoot -Parent
$framework = Join-Path $env:WINDIR 'Microsoft.NET/Framework64/v4.0.30319'
$temporary = Join-Path ([IO.Path]::GetTempPath()) ('asb-installer-contracts-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $temporary | Out-Null
try {
  $test = Join-Path $temporary 'contracts.exe'
  $metadata = Join-Path $repository 'installer/tests/Package.txt'
  $theme = Join-Path $repository 'installer/Theme.xaml'
  $sources = @('installer/InstallOptions.cs', 'installer/InstallerEngine.cs', 'installer/InstallerWindow.cs', 'installer/tests/Contracts.cs') | ForEach-Object { [IO.Path]::GetFullPath((Join-Path $repository $_)) }
  $references = @('System.Xaml.dll', 'System.Windows.Forms.dll', 'WPF/WindowsBase.dll', 'WPF/PresentationCore.dll', 'WPF/PresentationFramework.dll') | ForEach-Object { '/reference:' + [IO.Path]::GetFullPath((Join-Path $framework $_)) }
  & (Join-Path $framework 'csc.exe') /nologo /target:exe /platform:x64 /langversion:5 "/out:$test" `
    "/resource:$metadata,AgentSwitchboard.Installer.Package.txt" "/resource:$theme,AgentSwitchboard.Installer.Theme.xaml" @references @sources
  if ($LASTEXITCODE -ne 0) { throw 'Installer contract tests did not compile.' }
  & $test
  if ($LASTEXITCODE -ne 0) { throw "Installer contract tests failed ($LASTEXITCODE)." }
}
finally { Remove-Item -LiteralPath $temporary -Recurse -Force }
