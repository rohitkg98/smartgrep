# smartgrep installer for Windows (Windows PowerShell 5.1+ and PowerShell 7+).
#
#   irm https://raw.githubusercontent.com/rohitkg98/smartgrep/main/install.ps1 | iex
#
# Environment variables:
#   SMARTGREP_VERSION      release to install, e.g. 0.4.0 or v0.4.0 (default: latest)
#   SMARTGREP_INSTALL_DIR  install directory (default: %LOCALAPPDATA%\Programs\smartgrep)
#   SMARTGREP_ARCHIVE      install from a local release .zip instead of downloading
#                          (checksum-verified against a SHA256SUMS file next to it, if any)
#   SMARTGREP_NO_PATH=1    don't add the install directory to the user PATH
#
# macOS / Linux / FreeBSD / Termux: use install.sh instead.

function Install-Smartgrep {
    $ErrorActionPreference = 'Stop'
    $ProgressPreference = 'SilentlyContinue'   # Invoke-WebRequest is very slow with the progress bar
    $repo = 'rohitkg98/smartgrep'
    $fallback = "cargo install --git https://github.com/$repo"

    # Windows PowerShell 5.1 defaults to TLS 1.0; GitHub needs 1.2+.
    try {
        [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    } catch { }

    # ── detect arch ──────────────────────────────────────────────────────────
    # The registry holds the machine's native arch even when this PowerShell runs
    # under x64/x86 emulation (where $env:PROCESSOR_ARCHITECTURE would say AMD64/x86).
    $arch = $null
    try {
        $arch = (Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Environment' -Name PROCESSOR_ARCHITECTURE).PROCESSOR_ARCHITECTURE
    } catch { }
    if (-not $arch) { $arch = $env:PROCESSOR_ARCHITEW6432 }
    if (-not $arch) { $arch = $env:PROCESSOR_ARCHITECTURE }
    switch ($arch) {
        'AMD64' { $target = 'x86_64-pc-windows-msvc' }
        'ARM64' { $target = 'aarch64-pc-windows-msvc' }
        default {
            throw "No prebuilt smartgrep binary for Windows on '$arch'. Build from source instead (needs Rust 1.70+): $fallback"
        }
    }
    $asset = "smartgrep-$target.zip"

    $tmp = Join-Path ([IO.Path]::GetTempPath()) ("smartgrep-" + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $tmp -Force | Out-Null
    try {
        $zip = Join-Path $tmp $asset
        $sums = $null

        # ── get the archive ──────────────────────────────────────────────────
        if ($env:SMARTGREP_ARCHIVE) {
            if (-not (Test-Path -LiteralPath $env:SMARTGREP_ARCHIVE -PathType Leaf)) {
                throw "SMARTGREP_ARCHIVE not found: $env:SMARTGREP_ARCHIVE"
            }
            $label = "from $env:SMARTGREP_ARCHIVE"
            $asset = Split-Path -Leaf $env:SMARTGREP_ARCHIVE
            $zip = Join-Path $tmp $asset
            Write-Host "Installing smartgrep $label ($target)..."
            Copy-Item -LiteralPath $env:SMARTGREP_ARCHIVE -Destination $zip
            $localSums = Join-Path (Split-Path -Parent $env:SMARTGREP_ARCHIVE) 'SHA256SUMS'
            if (Test-Path -LiteralPath $localSums) { $sums = $localSums }
            else { Write-Warning "no SHA256SUMS next to $env:SMARTGREP_ARCHIVE; skipping checksum verification" }
        } else {
            $tag = $null
            if ($env:SMARTGREP_VERSION) {
                $tag = 'v' + $env:SMARTGREP_VERSION.TrimStart('v')
            } else {
                try {
                    $tag = (Invoke-RestMethod -UseBasicParsing "https://api.github.com/repos/$repo/releases/latest").tag_name
                } catch { }
            }
            if ($tag) {
                $base = "https://github.com/$repo/releases/download/$tag"
                $label = $tag
            } else {
                # API unavailable (e.g. rate-limited): GitHub redirects this to the latest release.
                $base = "https://github.com/$repo/releases/latest/download"
                $label = '(latest)'
            }
            Write-Host "Downloading smartgrep $label ($target)..."
            try {
                Invoke-WebRequest -UseBasicParsing -Uri "$base/$asset" -OutFile $zip
            } catch {
                throw "Download failed: $base/$asset ($($_.Exception.Message)). If this release has no Windows binary, build from source: $fallback"
            }
            $remoteSums = Join-Path $tmp 'SHA256SUMS'
            try {
                Invoke-WebRequest -UseBasicParsing -Uri "$base/SHA256SUMS" -OutFile $remoteSums
                $sums = $remoteSums
            } catch {
                Write-Warning 'release has no SHA256SUMS; skipping checksum verification'
            }
        }

        # ── verify ───────────────────────────────────────────────────────────
        if ($sums) {
            $expected = $null
            foreach ($line in Get-Content -LiteralPath $sums) {
                $parts = $line.Trim() -split '\s+', 2
                if ($parts.Count -eq 2 -and $parts[1].TrimStart('*') -eq $asset) { $expected = $parts[0].ToLower(); break }
            }
            if (-not $expected) { throw "SHA256SUMS has no entry for $asset" }
            $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $zip).Hash.ToLower()
            if ($actual -ne $expected) {
                throw "Checksum mismatch for ${asset}: expected $expected, got $actual"
            }
            Write-Host 'Checksum verified.'
        }

        $extract = Join-Path $tmp 'x'
        Expand-Archive -LiteralPath $zip -DestinationPath $extract -Force
        $exe = Join-Path $extract 'smartgrep.exe'
        if (-not (Test-Path -LiteralPath $exe)) { throw "archive did not contain smartgrep.exe" }

        # ── install ──────────────────────────────────────────────────────────
        if ($env:SMARTGREP_INSTALL_DIR) { $dir = $env:SMARTGREP_INSTALL_DIR }
        else { $dir = Join-Path $env:LOCALAPPDATA 'Programs\smartgrep' }
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
        $dest = Join-Path $dir 'smartgrep.exe'
        $old = "$dest.old"

        # A running .exe can't be overwritten or deleted, but it can be renamed:
        # move the current one aside, then put the new one in place.
        if (Test-Path -LiteralPath $old) {
            try { Remove-Item -LiteralPath $old -Force } catch { }   # still locked by a running process
        }
        if (Test-Path -LiteralPath $dest) {
            if (Test-Path -LiteralPath $old) {
                $old = "$dest.$([Guid]::NewGuid().ToString('N').Substring(0, 8)).old"
            }
            Move-Item -LiteralPath $dest -Destination $old -Force
        }
        try {
            Copy-Item -LiteralPath $exe -Destination $dest -Force
        } catch {
            if (Test-Path -LiteralPath $old) { Move-Item -LiteralPath $old -Destination $dest -Force }
            throw
        }
        if (Test-Path -LiteralPath $old) {
            try { Remove-Item -LiteralPath $old -Force } catch { }   # removed on the next install if still running
        }

        Write-Host "Installed smartgrep $label to $dest"

        # ── PATH ─────────────────────────────────────────────────────────────
        if ($env:SMARTGREP_NO_PATH -ne '1') {
            # Edit the registry value directly so entries like %USERPROFILE%\bin stay
            # unexpanded (Environment.SetEnvironmentVariable would flatten them).
            $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)
            try {
                $userPath = [string]$key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
                $entries = @($userPath -split ';' | Where-Object { $_ })
                $expanded = @($entries | ForEach-Object { [Environment]::ExpandEnvironmentVariables($_).TrimEnd('\') })
                if ($expanded -notcontains $dir.TrimEnd('\')) {
                    $key.SetValue('Path', (@($entries) + $dir) -join ';', [Microsoft.Win32.RegistryValueKind]::ExpandString)
                    # Setting any user variable through .NET broadcasts WM_SETTINGCHANGE,
                    # so new terminals pick up the PATH change without logging out.
                    [Environment]::SetEnvironmentVariable('SMARTGREP_INSTALLER', '1', 'User')
                    [Environment]::SetEnvironmentVariable('SMARTGREP_INSTALLER', $null, 'User')
                    Write-Host "Added $dir to your user PATH. Restart your terminal to use 'smartgrep'."
                }
            } finally {
                $key.Close()
            }
            if (-not (($env:Path -split ';') | Where-Object { $_.TrimEnd('\') -ieq $dir.TrimEnd('\') })) {
                $env:Path = "$env:Path;$dir"
            }
        }
    } finally {
        Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
    }
}

Install-Smartgrep
