$ErrorActionPreference = 'Stop'
if ([Console]::In.ReadLine() -ne 'start') { exit 1 }
$program = $env:ASB_UPDATER_PROGRAM
# Windows PowerShell 5 returns the JSON array intact; @() would nest it.
$arguments = ConvertFrom-Json -InputObject $env:ASB_UPDATER_ARGUMENTS
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
    $start.RedirectStandardInput = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $process = [Diagnostics.Process]::Start($start)
    $process.StandardInput.Close()
    $streams = @(
        [pscustomobject]@{ Source = $process.StandardOutput.BaseStream; Target = [Console]::OpenStandardOutput(); Buffer = [byte[]]::new(8192); Read = $null }
        [pscustomobject]@{ Source = $process.StandardError.BaseStream; Target = [Console]::OpenStandardError(); Buffer = [byte[]]::new(8192); Read = $null }
    )
    foreach ($stream in $streams) {
        $stream.Read = $stream.Source.ReadAsync($stream.Buffer, 0, $stream.Buffer.Length)
    }
    $remaining = $streams.Count
    while ($remaining -gt 0) {
        $progress = $false
        foreach ($stream in $streams) {
            if ($null -eq $stream.Read -or -not $stream.Read.IsCompleted) { continue }
            $count = $stream.Read.GetAwaiter().GetResult()
            $progress = $true
            if ($count -eq 0) {
                $stream.Read = $null
                $remaining--
                continue
            }
            try {
                $stream.Target.Write($stream.Buffer, 0, $count)
            } catch [IO.IOException] {
                # ASB may exit after handoff; keep draining so the installer can finish.
                $stream.Target = [IO.Stream]::Null
            }
            $stream.Read = $stream.Source.ReadAsync($stream.Buffer, 0, $stream.Buffer.Length)
        }
        if (-not $progress) { [Threading.Thread]::Sleep(10) }
    }
    $process.WaitForExit()
    exit $process.ExitCode
} catch {
    [Console]::Error.WriteLine($_.Exception.Message)
    exit 1
}
