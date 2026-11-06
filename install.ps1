# Installs Quick label for this user and puts it under a hotkey.
#
# How to use: put install.ps1 next to quicklabel.exe and run it (right click, Run with
# PowerShell), or from PowerShell:
#   .\install.ps1                      # Ctrl+Alt+L
#   .\install.ps1 -Hotkey "CTRL+ALT+P"
#
# Working principle: copies quicklabel.exe to %LOCALAPPDATA%\Programs\Quick label (no admin
# rights needed) and makes a Start menu shortcut. The key: by default on the shortcut (Windows
# takes a few seconds for it); with resident mode (switched on in Quick label's Settings) a
# small background process holds it and Quick label opens at once. Running it again stops a
# running Quick label, updates the exe and keeps resident mode as it was. To remove: switch
# resident mode off, delete the shortcut "Quick label" from the Start menu and that folder.

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

# A running Quick label (window or background process) locks the exe.
Get-Process -Name quicklabel -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 300
Copy-Item -Force $source $exe

# Resident mode on (it is at Windows login): start it from the new exe; the shortcut keeps no key.
$run = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
$resident = $null -ne (Get-ItemProperty -Path $run -Name "Quick label" -ErrorAction SilentlyContinue)
if ($resident) {
    Set-ItemProperty -Path $run -Name "Quick label" -Value "`"$exe`" --resident"
}

$programs = [Environment]::GetFolderPath("Programs")
# An earlier shortcut to ChopShop's quick mode, replaced by this one.
Remove-Item -ErrorAction SilentlyContinue (Join-Path $programs "ChopShop label.lnk")

# A new shortcut file, not an update of the old one: Windows caches a shortcut's icon, and an
# old one (from before the exe had its icon) would stay.
$link = Join-Path $programs "Quick label.lnk"
Remove-Item -ErrorAction SilentlyContinue $link

$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut($link)
$shortcut.TargetPath = $exe
$shortcut.WorkingDirectory = $dir
$shortcut.Hotkey = if ($resident) { "" } else { $Hotkey }
$shortcut.Description = "Type a label, Ctrl+Enter prints it"
$shortcut.IconLocation = "$exe,0"
$shortcut.Save()

# Have Explorer reload its icon cache, so Start shows the new icon now.
Start-Process -NoNewWindow -Wait -ErrorAction SilentlyContinue "ie4uinit.exe" "-show"

if ($resident) {
    Start-Process -FilePath $exe -ArgumentList "--resident"
    Write-Output "Quick label installed in $dir; resident mode: Ctrl+Alt+L opens it at once."
} else {
    Write-Output "Quick label installed in $dir; $Hotkey starts it (Settings: resident mode opens it at once)."
}
