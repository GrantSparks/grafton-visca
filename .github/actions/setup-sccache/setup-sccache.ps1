$ErrorActionPreference = "Stop"

function Write-WarningAnnotation([string]$Message) {
    Write-Host "::warning::$Message"
}

function Add-FileLine([string]$Path, [string]$Line) {
    $Line | Out-File -FilePath $Path -Encoding utf8 -Append
}

function Disable-Sccache {
    Add-FileLine $env:GITHUB_ENV "SCCACHE_GHA_ENABLED=false"
    Add-FileLine $env:GITHUB_ENV "RUSTC_WRAPPER="
    Add-FileLine $env:GITHUB_ENV "SCCACHE_DIR="
}

function Get-TargetTriple {
    switch ("$env:RUNNER_OS/$env:RUNNER_ARCH") {
        "Windows/X64" { return "x86_64-pc-windows-msvc" }
        "Windows/ARM64" { return "aarch64-pc-windows-msvc" }
        default { throw "Unsupported runner combination: $env:RUNNER_OS/$env:RUNNER_ARCH" }
    }
}

function Invoke-Download([string]$Uri, [string]$OutFile) {
    for ($attempt = 1; $attempt -le 5; $attempt++) {
        try {
            Invoke-WebRequest -Uri $Uri -OutFile $OutFile
            return
        }
        catch {
            if ($attempt -eq 5) {
                throw
            }
            Start-Sleep -Seconds 2
        }
    }
}

try {
    $tag = if ($env:SCCACHE_SETUP_VERSION) { $env:SCCACHE_SETUP_VERSION } else { "v0.12.0" }
    $version = $tag.TrimStart('v')
    $triple = Get-TargetTriple
    $archive = "sccache-v$version-$triple.tar.gz"
    $baseUrl = "https://github.com/mozilla/sccache/releases/download/$tag"
    $installDir = Join-Path $env:RUNNER_TEMP "sccache-$version-$triple"
    $archivePath = Join-Path $installDir $archive
    $checksumPath = "$archivePath.sha256"

    if (Test-Path $installDir) {
        Remove-Item -Path $installDir -Recurse -Force
    }
    New-Item -ItemType Directory -Path $installDir | Out-Null

    Invoke-Download "$baseUrl/$archive" $archivePath
    Invoke-Download "$baseUrl/$archive.sha256" $checksumPath

    $expected = (Get-Content $checksumPath -Raw).Trim().ToLowerInvariant()
    $actual = (Get-FileHash -Algorithm SHA256 -Path $archivePath).Hash.ToLowerInvariant()
    if ($expected -ne $actual) {
        throw "sccache checksum verification failed for $archive"
    }

    tar -xzf $archivePath -C $installDir

    $binary = Get-ChildItem -Path $installDir -Filter sccache.exe -File -Recurse | Select-Object -First 1
    if (-not $binary) {
        throw "sccache binary was not found after extracting $archive"
    }

    $binDir = $binary.Directory.FullName
    $cacheDir = Join-Path $env:USERPROFILE ".cache\sccache"
    New-Item -ItemType Directory -Path $cacheDir -Force | Out-Null

    Add-FileLine $env:GITHUB_PATH $binDir
    $env:Path = "$binDir;$env:Path"
    $env:ACTIONS_CACHE_SERVICE_V2 = "on"
    if (-not $env:ACTIONS_RESULTS_URL) { $env:ACTIONS_RESULTS_URL = "" }
    if (-not $env:ACTIONS_RUNTIME_TOKEN) { $env:ACTIONS_RUNTIME_TOKEN = "" }
    $env:SCCACHE_GHA_ENABLED = "true"
    $env:RUSTC_WRAPPER = "sccache"
    $env:SCCACHE_DIR = $cacheDir
    $env:SCCACHE_PATH = $binary.FullName

    Add-FileLine $env:GITHUB_ENV "ACTIONS_CACHE_SERVICE_V2=on"
    Add-FileLine $env:GITHUB_ENV "ACTIONS_RESULTS_URL=$env:ACTIONS_RESULTS_URL"
    Add-FileLine $env:GITHUB_ENV "ACTIONS_RUNTIME_TOKEN=$env:ACTIONS_RUNTIME_TOKEN"
    Add-FileLine $env:GITHUB_ENV "SCCACHE_GHA_ENABLED=true"
    Add-FileLine $env:GITHUB_ENV "RUSTC_WRAPPER=sccache"
    Add-FileLine $env:GITHUB_ENV "SCCACHE_DIR=$cacheDir"
    Add-FileLine $env:GITHUB_ENV "SCCACHE_PATH=$($binary.FullName)"

    & sccache --version
    & sccache --start-server
    & sccache --show-stats | Out-Null
}
catch {
    Write-WarningAnnotation "sccache setup failed; continuing without compiler caching. $($_.Exception.Message)"
    Disable-Sccache
}
