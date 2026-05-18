param(
    [string]$Configuration = "release",
    [switch]$SkipCargoBuild,
    [string]$WaitForExecutableNewerThan,
    [int]$WaitTimeoutInSeconds = 300,
    [string]$LogPath
)

$ErrorActionPreference = "Stop"

$ProjectRoot = Split-Path -Parent (Split-Path -Parent $PSCommandPath)
$DistDirectory = Join-Path $ProjectRoot "dist"
$PayloadDirectory = Join-Path $DistDirectory "installer-payload"
$SetupPath = Join-Path $DistDirectory "WatchSetup.exe"
$SedPath = Join-Path $DistDirectory "WatchSetup.sed"
$ReleaseExecutable = Join-Path $ProjectRoot "target\$Configuration\Watch.exe"

Set-Location $ProjectRoot

function Invoke-CargoBuildForConfiguration {
    param(
        [string]$CargoConfiguration
    )

    $PreviousAutoDistributionValue = $env:WATCH_DISABLE_AUTO_DISTRIBUTION

    try {
        $env:WATCH_DISABLE_AUTO_DISTRIBUTION = "1"

        if ($CargoConfiguration -eq "debug") {
            cargo build
        } elseif ($CargoConfiguration -eq "release") {
            cargo build --release
        } else {
            cargo build --profile $CargoConfiguration
        }
    } finally {
        if ($null -eq $PreviousAutoDistributionValue) {
            Remove-Item Env:\WATCH_DISABLE_AUTO_DISTRIBUTION -ErrorAction SilentlyContinue
        } else {
            $env:WATCH_DISABLE_AUTO_DISTRIBUTION = $PreviousAutoDistributionValue
        }
    }
}

function Test-FileCanBeOpenedExclusively {
    param(
        [string]$Path
    )

    try {
        $FileStream = [System.IO.File]::Open(
            $Path,
            [System.IO.FileMode]::Open,
            [System.IO.FileAccess]::Read,
            [System.IO.FileShare]::None
        )
        $FileStream.Dispose()
        return $true
    } catch {
        return $false
    }
}

function Wait-ForExecutableToBeReady {
    param(
        [string]$ExecutablePath,
        [string]$MinimumTimestampPath,
        [int]$TimeoutInSeconds = 3600
    )

    $MinimumTimestampUtc = [DateTime]::MinValue
    if ($MinimumTimestampPath.Length -gt 0 -and (Test-Path -LiteralPath $MinimumTimestampPath)) {
        $MinimumTimestampUtc = (Get-Item -LiteralPath $MinimumTimestampPath).LastWriteTimeUtc
    }

    $Stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    while ($Stopwatch.Elapsed.TotalSeconds -lt $TimeoutInSeconds) {
        if (Test-Path -LiteralPath $ExecutablePath) {
            $Executable = Get-Item -LiteralPath $ExecutablePath
            $HasFreshTimestamp = $Executable.LastWriteTimeUtc -ge $MinimumTimestampUtc
            $HasContent = $Executable.Length -gt 0

            if ($HasFreshTimestamp -and $HasContent -and (Test-FileCanBeOpenedExclusively -Path $ExecutablePath)) {
                return
            }
        }

        Start-Sleep -Milliseconds 500
    }

    throw "Timed out waiting for release executable to be ready: $ExecutablePath"
}

if ($LogPath.Length -gt 0) {
    New-Item -ItemType Directory -Path (Split-Path -Parent $LogPath) -Force | Out-Null
    "[$([DateTime]::Now.ToString("s"))] Starting Watch installer packaging." | Out-File -LiteralPath $LogPath -Encoding UTF8 -Append
}

if ($SkipCargoBuild) {
    Wait-ForExecutableToBeReady -ExecutablePath $ReleaseExecutable -MinimumTimestampPath $WaitForExecutableNewerThan -TimeoutInSeconds $WaitTimeoutInSeconds
} else {
    Invoke-CargoBuildForConfiguration -CargoConfiguration $Configuration
}

if (!(Test-Path -LiteralPath $ReleaseExecutable)) {
    throw "Expected build output was not found: $ReleaseExecutable"
}

if (Test-Path -LiteralPath $PayloadDirectory) {
    Remove-Item -LiteralPath $PayloadDirectory -Recurse -Force
}

New-Item -ItemType Directory -Path $DistDirectory -Force | Out-Null
New-Item -ItemType Directory -Path $PayloadDirectory -Force | Out-Null

Copy-Item -LiteralPath $ReleaseExecutable -Destination (Join-Path $PayloadDirectory "Watch.exe") -Force
Copy-Item -LiteralPath (Join-Path $ProjectRoot "installer\install.ps1") -Destination $PayloadDirectory -Force
Copy-Item -LiteralPath (Join-Path $ProjectRoot "installer\uninstall.ps1") -Destination $PayloadDirectory -Force
Copy-Item -Path (Join-Path $ProjectRoot "src\icons\*.svg") -Destination $PayloadDirectory -Force

$PortableToolsDirectory = Join-Path $ProjectRoot "tools"
if (Test-Path -LiteralPath $PortableToolsDirectory) {
    Copy-Item -LiteralPath $PortableToolsDirectory -Destination (Join-Path $PayloadDirectory "tools") -Recurse -Force
    Write-Host "Bundled portable tools from $PortableToolsDirectory"
} else {
    Write-Host "No portable tools directory found. The installed app will look for mpv and ffprobe on PATH."
}

$PayloadFiles = Get-ChildItem -LiteralPath $PayloadDirectory -File | Sort-Object Name
$StringEntries = New-Object System.Collections.Generic.List[string]
$SourceEntries = New-Object System.Collections.Generic.List[string]

for ($FileIndex = 0; $FileIndex -lt $PayloadFiles.Count; $FileIndex++) {
    $FileName = $PayloadFiles[$FileIndex].Name
    $StringEntries.Add("FILE$FileIndex=`"$FileName`"")
    $SourceEntries.Add("%FILE$FileIndex%=")
}

$EscapedSetupPath = $SetupPath.Replace("\", "\\")
$EscapedPayloadDirectory = ($PayloadDirectory + "\").Replace("\", "\\")
$StringBlock = $StringEntries -join "`r`n"
$SourceBlock = $SourceEntries -join "`r`n"

$SedContent = @"
[Version]
Class=IEXPRESS
SEDVersion=3

[Options]
PackagePurpose=InstallApp
ShowInstallProgramWindow=0
HideExtractAnimation=1
UseLongFileName=1
InsideCompressed=0
CAB_FixedSize=0
CAB_ResvCodeSigning=0
RebootMode=N
InstallPrompt=%InstallPrompt%
DisplayLicense=%DisplayLicense%
FinishMessage=%FinishMessage%
TargetName=%TargetName%
FriendlyName=%FriendlyName%
AppLaunched=%AppLaunched%
PostInstallCmd=%PostInstallCmd%
AdminQuietInstCmd=%AdminQuietInstCmd%
UserQuietInstCmd=%UserQuietInstCmd%
SourceFiles=SourceFiles

[Strings]
InstallPrompt=
DisplayLicense=
FinishMessage=Watch has been installed.
TargetName=$EscapedSetupPath
FriendlyName=Watch Installer
AppLaunched=powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File install.ps1
PostInstallCmd=<None>
AdminQuietInstCmd=
UserQuietInstCmd=
$StringBlock

[SourceFiles]
SourceFiles0=$EscapedPayloadDirectory

[SourceFiles0]
$SourceBlock
"@

Set-Content -LiteralPath $SedPath -Value $SedContent -Encoding ASCII

$IExpressPath = Join-Path $env:SystemRoot "System32\iexpress.exe"
if (!(Test-Path -LiteralPath $IExpressPath)) {
    throw "IExpress was not found at $IExpressPath"
}

& $IExpressPath /N /Q $SedPath
$IExpressExitCode = $LASTEXITCODE

if (!(Test-Path -LiteralPath $SetupPath)) {
    throw "Installer was not created: $SetupPath (IExpress exit code: $IExpressExitCode)"
}

Write-Host "Created installer: $SetupPath"
exit 0
