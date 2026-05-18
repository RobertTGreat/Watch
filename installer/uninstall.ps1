param(
    [string]$InstallDirectory = (Split-Path -Parent $PSCommandPath)
)

$ErrorActionPreference = "Stop"

$AppName = "Watch"
$ExecutableName = "Watch.exe"
$StartMenuProgramsDirectory = [Environment]::GetFolderPath([Environment+SpecialFolder]::Programs)
if ([string]::IsNullOrWhiteSpace($StartMenuProgramsDirectory)) {
    $StartMenuProgramsDirectory = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs"
}
$VideoExtensions = @(
    "mkv", "mp4", "mov", "m4v", "3gp", "avi", "wmv", "asf", "ogm", "ogg",
    "flv", "webm", "mxf", "mpeg", "mpg", "m2ts", "ts", "vob", "divx", "dv"
)

foreach ($VideoExtension in $VideoExtensions) {
    $Extension = ".$VideoExtension"
    Remove-Item -Path "HKCU:\Software\Classes\SystemFileAssociations\$Extension\shell\OpenWithWatch" -Recurse -Force -ErrorAction SilentlyContinue
}

Remove-Item -Path "HKCU:\Software\Classes\Applications\$ExecutableName" -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\App Paths\$ExecutableName" -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Watch" -Recurse -Force -ErrorAction SilentlyContinue

Remove-Item -LiteralPath (Join-Path $StartMenuProgramsDirectory "$AppName.lnk") -Force -ErrorAction SilentlyContinue

if (Get-Command ie4uinit.exe -ErrorAction SilentlyContinue) {
    Start-Process -FilePath "ie4uinit.exe" -ArgumentList "-show" -WindowStyle Hidden -ErrorAction SilentlyContinue
}

$DeleteCommand = @"
Start-Sleep -Seconds 1
Remove-Item -LiteralPath '$InstallDirectory' -Recurse -Force -ErrorAction SilentlyContinue
"@

Start-Process -FilePath "powershell.exe" -ArgumentList @(
    "-NoProfile",
    "-ExecutionPolicy",
    "Bypass",
    "-Command",
    $DeleteCommand
) -WindowStyle Hidden

Write-Host "Watch uninstalled."
