# Installs Quick label for this user: the exe and a Start menu item.
#
# How to use: put install.ps1 next to quicklabel.exe and run it (right click, Run with
# PowerShell). The hotkey is set in Quick label itself (Settings: stay ready in the background).
#
# Working principle: copies quicklabel.exe to %LOCALAPPDATA%\Programs\Quick label (no admin
# rights needed) and makes the Start menu shortcut "Quick label". Running it again stops a
# running Quick label, updates the exe and starts the background process again when it was on.
# To remove: switch staying ready off, delete the Start menu shortcut and that folder.

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

# A new shortcut file, not an update of the old one: Windows caches a shortcut's icon.
$link = Join-Path ([Environment]::GetFolderPath("Programs")) "Quick label.lnk"
Remove-Item -ErrorAction SilentlyContinue $link
$shortcut = (New-Object -ComObject WScript.Shell).CreateShortcut($link)
$shortcut.TargetPath = $exe
$shortcut.WorkingDirectory = $dir
$shortcut.Description = "Type a label, Ctrl+Enter prints it"
$shortcut.IconLocation = "$exe,0"
$shortcut.Save()

# Have Explorer reload its icon cache, so Start shows the new icon now.
Start-Process -NoNewWindow -Wait -ErrorAction SilentlyContinue "ie4uinit.exe" "-show"

# Staying ready was on (it starts at login): the background process again, from the new exe.
$run = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
if (Get-ItemProperty -Path $run -Name "Quick label" -ErrorAction SilentlyContinue) {
    Start-Process -FilePath $exe -ArgumentList "--resident"
}

Write-Output "Quick label installed in $dir and in the Start menu."
