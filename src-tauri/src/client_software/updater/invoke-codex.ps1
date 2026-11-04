$ErrorActionPreference = 'Stop'
# Configure this PowerShell process; the official installer remains unmodified.
if ($env:ASB_UPDATE_PROXY_MODE -eq 'none') {
    [System.Net.WebRequest]::DefaultWebProxy = [System.Net.WebProxy]::new()
} elseif ($env:ASB_UPDATE_PROXY_MODE -eq 'manual') {
    $proxyUri = [System.Uri]::new($env:ASB_UPDATE_PROXY_URL)
    $address = [System.UriBuilder]::new($proxyUri)
    $address.UserName = ''
    $address.Password = ''
    $proxy = [System.Net.WebProxy]::new($address.Uri)
    if ($proxyUri.UserInfo) {
        $parts = $proxyUri.UserInfo.Split(':', 2)
        $user = [System.Uri]::UnescapeDataString($parts[0])
        $password = if ($parts.Length -gt 1) { [System.Uri]::UnescapeDataString($parts[1]) } else { '' }
        $proxy.Credentials = [System.Net.NetworkCredential]::new($user, $password)
    }
    [System.Net.WebRequest]::DefaultWebProxy = $proxy
}
& $env:ASB_CODEX_INSTALLER -Release $env:ASB_CODEX_RELEASE
if (-not $?) { exit 1 }
