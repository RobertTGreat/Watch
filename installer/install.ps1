param(
    [string]$InstallDirectory = "$env:LOCALAPPDATA\Programs\Watch"
)

$ErrorActionPreference = "Stop"

$AppName = "Watch"
$ExecutableName = "Watch.exe"
$SourceDirectory = Split-Path -Parent $PSCommandPath
$InstallIconDirectory = Join-Path $InstallDirectory "icons"
$ExecutablePath = Join-Path $InstallDirectory $ExecutableName
$StartMenuProgramsDirectory = [Environment]::GetFolderPath([Environment+SpecialFolder]::Programs)
if ([string]::IsNullOrWhiteSpace($StartMenuProgramsDirectory)) {
    $StartMenuProgramsDirectory = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs"
}
$StartMenuShortcutPath = Join-Path $StartMenuProgramsDirectory "$AppName.lnk"
$VideoExtensions = @(
    "mkv", "mp4", "mov", "m4v", "3gp", "avi", "wmv", "asf", "ogm", "ogg",
    "flv", "webm", "mxf", "mpeg", "mpg", "m2ts", "ts", "vob", "divx", "dv"
)

function Set-RegistryString {
    param(
        [string]$Path,
        [string]$Name,
        [string]$Value
    )

    $registrySubKeyPath = $Path -replace "^HKCU:\\", ""
    $registryKey = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey($registrySubKeyPath)

    try {
        if ($Name.Length -eq 0) {
            $registryKey.SetValue("", $Value, [Microsoft.Win32.RegistryValueKind]::String)
        } else {
            $registryKey.SetValue($Name, $Value, [Microsoft.Win32.RegistryValueKind]::String)
        }
    } finally {
        $registryKey.Dispose()
    }
}

New-Item -ItemType Directory -Path $InstallDirectory -Force | Out-Null
New-Item -ItemType Directory -Path $InstallIconDirectory -Force | Out-Null
New-Item -ItemType Directory -Path $StartMenuProgramsDirectory -Force | Out-Null

Copy-Item -LiteralPath (Join-Path $SourceDirectory $ExecutableName) -Destination $ExecutablePath -Force
Copy-Item -LiteralPath (Join-Path $SourceDirectory "uninstall.ps1") -Destination (Join-Path $InstallDirectory "uninstall.ps1") -Force
if (Test-Path -LiteralPath (Join-Path $SourceDirectory "tools")) {
    Copy-Item -LiteralPath (Join-Path $SourceDirectory "tools") -Destination (Join-Path $InstallDirectory "tools") -Recurse -Force
}
Get-ChildItem -LiteralPath $SourceDirectory -Filter "*.svg" | ForEach-Object {
    Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $InstallIconDirectory $_.Name) -Force
}

$OpenCommand = "`"$ExecutablePath`" `"%1`""
$ApplicationRegistryPath = "HKCU:\Software\Classes\Applications\$ExecutableName"
Set-RegistryString -Path $ApplicationRegistryPath -Name "FriendlyAppName" -Value $AppName
Set-RegistryString -Path "$ApplicationRegistryPath\DefaultIcon" -Name "" -Value "`"$ExecutablePath`",0"
Set-RegistryString -Path "$ApplicationRegistryPath\shell\open\command" -Name "" -Value $OpenCommand

$ApplicationPathRegistryPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\App Paths\$ExecutableName"
Set-RegistryString -Path $ApplicationPathRegistryPath -Name "" -Value $ExecutablePath
Set-RegistryString -Path $ApplicationPathRegistryPath -Name "Path" -Value $InstallDirectory

foreach ($VideoExtension in $VideoExtensions) {
    $Extension = ".$VideoExtension"
    Set-RegistryString -Path "$ApplicationRegistryPath\SupportedTypes" -Name $Extension -Value ""

    $ContextMenuPath = "HKCU:\Software\Classes\SystemFileAssociations\$Extension\shell\OpenWithWatch"
    Set-RegistryString -Path $ContextMenuPath -Name "" -Value "Open with Watch"
    Set-RegistryString -Path $ContextMenuPath -Name "MUIVerb" -Value "Open with Watch"
    Set-RegistryString -Path $ContextMenuPath -Name "Icon" -Value "`"$ExecutablePath`",0"
    Set-RegistryString -Path "$ContextMenuPath\command" -Name "" -Value $OpenCommand
}

$ShortcutShell = New-Object -ComObject WScript.Shell
$Shortcut = $ShortcutShell.CreateShortcut($StartMenuShortcutPath)
$Shortcut.TargetPath = $ExecutablePath
$Shortcut.WorkingDirectory = $InstallDirectory
$Shortcut.IconLocation = "$ExecutablePath,0"
$Shortcut.Description = $AppName
$Shortcut.Save()

$UninstallRegistryPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Watch"
Set-RegistryString -Path $UninstallRegistryPath -Name "DisplayName" -Value $AppName
Set-RegistryString -Path $UninstallRegistryPath -Name "DisplayVersion" -Value "0.1.0"
Set-RegistryString -Path $UninstallRegistryPath -Name "Publisher" -Value $AppName
Set-RegistryString -Path $UninstallRegistryPath -Name "InstallLocation" -Value $InstallDirectory
Set-RegistryString -Path $UninstallRegistryPath -Name "DisplayIcon" -Value "$ExecutablePath,0"
Set-RegistryString -Path $UninstallRegistryPath -Name "UninstallString" -Value "powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$InstallDirectory\uninstall.ps1`""
Set-RegistryString -Path $UninstallRegistryPath -Name "QuietUninstallString" -Value "powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$InstallDirectory\uninstall.ps1`""
Set-RegistryString -Path $UninstallRegistryPath -Name "InstallDate" -Value (Get-Date -Format "yyyyMMdd")

Set-ItemProperty -Path $UninstallRegistryPath -Name "NoModify" -Value 1 -Type DWord
Set-ItemProperty -Path $UninstallRegistryPath -Name "NoRepair" -Value 1 -Type DWord
Set-ItemProperty -Path $UninstallRegistryPath -Name "WindowsInstaller" -Value 0 -Type DWord

$ExecutableSizeInKilobytes = [Math]::Max(1, [Math]::Ceiling((Get-Item -LiteralPath $ExecutablePath).Length / 1KB))
Set-ItemProperty -Path $UninstallRegistryPath -Name "EstimatedSize" -Value $ExecutableSizeInKilobytes -Type DWord

if (Get-Command ie4uinit.exe -ErrorAction SilentlyContinue) {
    Start-Process -FilePath "ie4uinit.exe" -ArgumentList "-show" -WindowStyle Hidden -ErrorAction SilentlyContinue
}

Write-Host "Watch installed to $InstallDirectory"
Write-Host "Start Menu shortcut created at $StartMenuShortcutPath"
