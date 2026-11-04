$ErrorActionPreference = 'Stop'
if ([Console]::In.ReadLine() -ne 'start') { exit 1 }
$program = $env:ASB_UPDATER_PROGRAM
$arguments = @(ConvertFrom-Json -InputObject $env:ASB_UPDATER_ARGUMENTS)
Remove-Item Env:ASB_UPDATER_PROGRAM, Env:ASB_UPDATER_ARGUMENTS
try {
    $quoted = foreach ($argument in $arguments) {
        $escaped = [regex]::Replace([string]$argument, '(\\*)"', '$1$1\"')
        $escaped = [regex]::Replace($escaped, '(\\+)$', '$1$1')
        '"' + $escaped + '"'
    }
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $program
    $start.Arguments = $quoted -join ' '
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $process = [Diagnostics.Process]::Start($start)
    $process.WaitForExit()
    exit $process.ExitCode
} catch {
    [Console]::Error.WriteLine($_.Exception.Message)
    exit 1
}
