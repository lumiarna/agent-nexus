[CmdletBinding()]
param(
    [switch] $SetupOnly,

    [string] $CommandArgsBase64,

    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]] $Command
)

$ErrorActionPreference = "Stop"

$SQLiteVersion = "3530200"
$ArchiveName = "sqlite-dll-win-x64-$SQLiteVersion.zip"
$DownloadUrl = "https://www.sqlite.org/2026/$ArchiveName"
$ExpectedArchiveSha256 = "5D40DE68DA94CEE0FBB01A7CAAE96C9226872549FB007E826F63CD7BB464B463"

$ScriptRoot = Split-Path -Parent $PSCommandPath
$RepoRoot = Resolve-Path (Join-Path $ScriptRoot "..")
$TauriRoot = Join-Path $RepoRoot "src-tauri"
$CacheRoot = Join-Path $TauriRoot "target\sqlite-official-$SQLiteVersion"
$ArchivePath = Join-Path $CacheRoot $ArchiveName
$ExtractDir = Join-Path $CacheRoot "extract"
$VendorRoot = Join-Path $TauriRoot "vendor\sqlite\windows-x64"
$BinDir = Join-Path $VendorRoot "bin"
$DefDir = Join-Path $VendorRoot "def"
$LibDir = Join-Path $VendorRoot "lib"
$DllPath = Join-Path $BinDir "sqlite3.dll"
$DefPath = Join-Path $DefDir "sqlite3.def"
$LibPath = Join-Path $LibDir "sqlite3.lib"

function Ensure-Directory([string] $Path) {
    New-Item -ItemType Directory -Force -Path $Path | Out-Null
}

function Test-ExpectedHash([string] $Path, [string] $ExpectedSha256) {
    if (!(Test-Path -LiteralPath $Path)) {
        return $false
    }

    $actual = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToUpperInvariant()
    return $actual -eq $ExpectedSha256
}

function Find-MsvcLibExe {
    $existing = Get-Command lib.exe -ErrorAction SilentlyContinue
    if ($existing) {
        return $existing.Source
    }

    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path -LiteralPath $vswhere) {
        $installationPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        if ($LASTEXITCODE -eq 0 -and $installationPath) {
            $toolsRoot = Join-Path $installationPath "VC\Tools\MSVC"
            if (Test-Path -LiteralPath $toolsRoot) {
                $candidate = Get-ChildItem -LiteralPath $toolsRoot -Directory |
                    Sort-Object Name -Descending |
                    ForEach-Object { Join-Path $_.FullName "bin\Hostx64\x64\lib.exe" } |
                    Where-Object { Test-Path -LiteralPath $_ } |
                    Select-Object -First 1
                if ($candidate) {
                    return $candidate
                }
            }
        }
    }

    $fallbackRoot = Join-Path $env:ProgramFiles "Microsoft Visual Studio\2022"
    if (Test-Path -LiteralPath $fallbackRoot) {
        $candidate = Get-ChildItem -Path $fallbackRoot -Recurse -Filter lib.exe -ErrorAction SilentlyContinue |
            Where-Object { $_.FullName -like "*\bin\Hostx64\x64\lib.exe" } |
            Sort-Object FullName -Descending |
            Select-Object -First 1
        if ($candidate) {
            return $candidate.FullName
        }
    }

    throw "Cannot find MSVC lib.exe. Install Microsoft C++ Build Tools with the x64 toolchain."
}

function Ensure-SqliteArchive {
    Ensure-Directory $CacheRoot

    if (Test-ExpectedHash $ArchivePath $ExpectedArchiveSha256) {
        return
    }

    if (Test-Path -LiteralPath $ArchivePath) {
        Remove-Item -LiteralPath $ArchivePath -Force
    }

    Invoke-WebRequest -Uri $DownloadUrl -OutFile $ArchivePath -UseBasicParsing

    if (!(Test-ExpectedHash $ArchivePath $ExpectedArchiveSha256)) {
        $actual = (Get-FileHash -LiteralPath $ArchivePath -Algorithm SHA256).Hash.ToUpperInvariant()
        throw "SQLite archive hash mismatch. Expected $ExpectedArchiveSha256 but got $actual."
    }
}

function Ensure-SqliteFiles {
    Ensure-SqliteArchive

    $extractDll = Join-Path $ExtractDir "sqlite3.dll"
    $extractDef = Join-Path $ExtractDir "sqlite3.def"
    if (!(Test-Path -LiteralPath $extractDll) -or !(Test-Path -LiteralPath $extractDef)) {
        if (Test-Path -LiteralPath $ExtractDir) {
            Remove-Item -LiteralPath $ExtractDir -Recurse -Force
        }
        Ensure-Directory $ExtractDir
        Expand-Archive -LiteralPath $ArchivePath -DestinationPath $ExtractDir -Force
    }

    Ensure-Directory $BinDir
    Ensure-Directory $DefDir
    Ensure-Directory $LibDir
    Copy-Item -LiteralPath $extractDll -Destination $DllPath -Force
    Copy-Item -LiteralPath $extractDef -Destination $DefPath -Force

    $libExe = Find-MsvcLibExe
    Push-Location $LibDir
    try {
        & $libExe /NOLOGO /MACHINE:X64 "/DEF:$DefPath" /OUT:sqlite3.lib
        if ($LASTEXITCODE -ne 0) {
            throw "lib.exe failed with exit code $LASTEXITCODE."
        }
    }
    finally {
        Pop-Location
    }

    if (!(Test-Path -LiteralPath $LibPath)) {
        throw "sqlite3.lib was not created at $LibPath."
    }
}

function Copy-RuntimeDll {
    $targetRoot = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $RepoRoot "target" }
    foreach ($profile in @("debug", "release")) {
        $dir = Join-Path $targetRoot $profile
        if (Test-Path -LiteralPath $dir) {
            Copy-Item -LiteralPath $DllPath -Destination (Join-Path $dir "sqlite3.dll") -Force
        }
    }
}

Ensure-SqliteFiles

$env:SQLITE3_LIB_DIR = $LibDir
$env:PATH = "$BinDir;$env:PATH"
$env:AGENT_NEXUS_SQLITE3_VERSION = $SQLiteVersion
$env:AGENT_NEXUS_SQLITE3_DLL = $DllPath

Copy-RuntimeDll

if ($CommandArgsBase64) {
    $Command = @(
        $CommandArgsBase64 -split "," |
            Where-Object { $_ } |
            ForEach-Object { [System.Text.Encoding]::UTF8.GetString([System.Convert]::FromBase64String($_)) }
    )
}

if ($SetupOnly -or $Command.Count -eq 0) {
    Write-Output "SQLITE3_LIB_DIR=$LibDir"
    Write-Output "sqlite3.dll=$DllPath"
    Write-Output "sqlite3.lib=$LibPath"
    exit 0
}

$executable = $Command[0]
$arguments = @()
if ($Command.Count -gt 1) {
    $arguments = $Command[1..($Command.Count - 1)]
}

& $executable @arguments
$exitCode = if ($null -ne $LASTEXITCODE) { $LASTEXITCODE } else { 0 }

Copy-RuntimeDll
exit $exitCode
