# Installs the reylo command on Windows, from the binaries a GitHub release of colinhacks/reylo
# carries, each checked against the release's SHA256SUMS before it is installed.
#
#   irm https://reylo.dev/install.ps1 | iex
#
# $env:REYLO_VERSION         The version to install, as 0.8.0; the latest release by default.
# $env:REYLO_INSTALL_DIR     Where reylo.exe goes; ~\.local\bin by default, which is added to the
#                            user's Path unless $env:REYLO_NO_MODIFY_PATH is set.
# $env:REYLO_DOWNLOAD_URL    A directory holding a release's assets, in place of the release's own.
#
# On macOS and Linux, install.sh beside this file does the same.
& {
  $ErrorActionPreference = 'Stop'
  $ProgressPreference = 'SilentlyContinue'
  # Windows PowerShell 5.1 starts without TLS 1.2, which GitHub requires.
  [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

  $repo = 'colinhacks/reylo'
  $dir = if ($env:REYLO_INSTALL_DIR) { $env:REYLO_INSTALL_DIR } else { Join-Path $HOME '.local\bin' }
  $version = if ($env:REYLO_VERSION) { $env:REYLO_VERSION -replace '^v', '' } else { '' }

  # The machine's own architecture, from the system's environment: a process emulated as x64 on arm64,
  # as Windows PowerShell 5.1 can be, sees AMD64 in its own environment and in .NET's OSArchitecture.
  $arch = (Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Environment' -ErrorAction SilentlyContinue).PROCESSOR_ARCHITECTURE
  if (-not $arch) { $arch = $env:PROCESSOR_ARCHITECTURE }
  $cpu = switch ($arch) {
    'AMD64' { 'x64' }
    'ARM64' { 'arm64' }
    default { throw "reylo install: no build for $arch; reylo is built for x64 and arm64" }
  }
  $asset = "reylo-win32-$cpu.tar.gz"

  # The latest release's assets come from GitHub's latest/download, SHA256SUMS first. Should a release
  # land between the two downloads, the archive no longer matches the sums and nothing is installed.
  $base = if ($env:REYLO_DOWNLOAD_URL) { $env:REYLO_DOWNLOAD_URL.TrimEnd('/') }
    elseif ($version) { "https://github.com/$repo/releases/download/v$version" }
    else { "https://github.com/$repo/releases/latest/download" }

  $tmp = Join-Path ([IO.Path]::GetTempPath()) ("reylo-install-" + [Guid]::NewGuid())
  New-Item -ItemType Directory -Path $tmp | Out-Null
  try {
    try { Invoke-WebRequest -UseBasicParsing -Uri "$base/SHA256SUMS" -OutFile "$tmp\SHA256SUMS" }
    catch { throw "reylo install: cannot download $base/SHA256SUMS$(if ($version) { "; is $version a released version?" }): $($_.Exception.Message)" }
    $expected = foreach ($line in Get-Content "$tmp\SHA256SUMS") {
      $hash, $name = $line -split '\s+\*?', 2
      if ($name -eq $asset) { $hash.ToLowerInvariant() }
    }
    if (-not $expected) { throw "reylo install: $base/SHA256SUMS lists no $asset" }
    try { Invoke-WebRequest -UseBasicParsing -Uri "$base/$asset" -OutFile "$tmp\$asset" }
    catch { throw "reylo install: cannot download $base/$asset`: $($_.Exception.Message)" }
    $actual = (Get-FileHash -Algorithm SHA256 "$tmp\$asset").Hash.ToLowerInvariant()
    if ($actual -ne $expected) { throw "reylo install: $asset does not match SHA256SUMS (expected $expected, got $actual); nothing was installed" }

    # tar.exe has come with Windows since Windows 10 1803.
    & "$env:SystemRoot\System32\tar.exe" -xzf "$tmp\$asset" -C $tmp
    if ($LASTEXITCODE -ne 0) { throw "reylo install: cannot unpack $asset" }
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    Move-Item -Force "$tmp\reylo.exe" (Join-Path $dir 'reylo.exe')
  } finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
  }

  $exe = Join-Path $dir 'reylo.exe'
  Write-Host "Installed $(& $exe --version) (win32-$cpu) at $exe"
  $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
  $onPath = ($userPath -split ';' | Where-Object { $_ -and ($_.TrimEnd('\') -eq $dir.TrimEnd('\')) })
  if (-not $onPath) {
    if ($env:REYLO_NO_MODIFY_PATH) {
      Write-Host "$dir is not on your Path: add it to use reylo by name"
    } else {
      [Environment]::SetEnvironmentVariable('Path', ((@($dir) + ($userPath -split ';' | Where-Object { $_ })) -join ';'), 'User')
      $env:Path = "$dir;$env:Path"
      Write-Host "Added $dir to your user Path; a terminal opened from now on finds reylo"
    }
  }
}
