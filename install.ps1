# Installs Quick label for this user and puts it under a hotkey.
#
# How to use: put install.ps1 next to quicklabel.exe and run it (right click, Run with
# PowerShell), or from PowerShell:
#   .\install.ps1                      # Ctrl+Alt+L
#   .\install.ps1 -Hotkey "CTRL+ALT+P"
#
# Working principle: copies quicklabel.exe to %LOCALAPPDATA%\Programs\Quick label (no admin
# rights needed) and makes a Start menu shortcut with a shortcut key. Windows itself listens for
# the key, from any program: no background process. Running it again updates the exe and the
# key. To remove: delete the shortcut "Quick label" from the Start menu and that folder.

param(
    [string]$Hotkey = "CTRL+ALT+L"
)

$source = Join-Path $PSScriptRoot "quicklabel.exe"
if (-not (Test-Path $source)) {
    Write-Error "quicklabel.exe not found next to install.ps1"
    exit 1
}

$dir = Join-Path $env:LOCALAPPDATA "Programs\Quick label"
New-Item -ItemType Directory -Force -Path $dir | Out-Null
$exe = Join-Path $dir "quicklabel.exe"
Copy-Item -Force $source $exe

$programs = [Environment]::GetFolderPath("Programs")
# An earlier shortcut to ChopShop's quick mode, replaced by this one.
Remove-Item -ErrorAction SilentlyContinue (Join-Path $programs "ChopShop label.lnk")

$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut((Join-Path $programs "Quick label.lnk"))
$shortcut.TargetPath = $exe
$shortcut.WorkingDirectory = $dir
$shortcut.Hotkey = $Hotkey
$shortcut.Description = "Type a label, Ctrl+Enter prints it"
$shortcut.IconLocation = "$exe,0"
$shortcut.Save()

Write-Output "Quick label installed in $dir; $Hotkey starts it."
