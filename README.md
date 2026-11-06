# Quick label

Type a few lines, press **Ctrl+Enter**: printed on the Brother QL label printer.

- It opens with the last text, selected: typing replaces it, Ctrl+Enter prints it again.
  The number of labels starts at 1.
- Enter starts a new line; `**bold**`, `*italic*`; Ctrl+B / Ctrl+I put them around the
  selection or take them off. `/` brings you back to the text from anywhere in the window.
- Copy and paste work with other programs.
- Ctrl+D inserts today's date, Ctrl+T the time.
- Ctrl+Enter prints, the window stays open (Settings: close after printing); Esc closes.
- The text is as large as it fits: more lines, smaller letters. The tape is the one the printer
  reports.
- Lengthwise (along the label's long side) or Sideways (Ctrl+R). On continuous tape the label
  is at most 1×, 2× (default) or 3× the tape's width long, or free; the text shrinks to fit.
  Die-cut labels have their own size.
- Fill lines (Ctrl+F): every line as large as fits the width, each its own size; together
  they shrink to fit the height. Off: one size for all lines.
- Cut: after each label (default), once after the batch, or off.
- Direction, fill, length and cut are kept per tape size.
- Several labels at once: − n + next to Print (Ctrl+Up / Ctrl+Down), one job. `{n}` in the
  text is the label's number from 1; `{n+12}` adds 12 (13, 14, …); `{n+007}` keeps the zeros (008, …).
- The first time: Settings (top right) opens by itself; Search the network finds Brother QL
  printers (this PC's network and that of printers already set), or add the printer's address (for example `192.168.1.40`).
  Several printers: click the one to print on.
- English or Dutch (the EN | NL switch, top right; at first the system's language). Light or dark as
  Windows is set, or by the moon / sun button (remembered).

## Instantly under the hotkey (resident mode)

Windows takes a few seconds for a shortcut's key. In Quick label: Settings → **Stay ready in
the background**. A small process (`quicklabel --resident`, a few MB, idle) then holds
Ctrl+Alt+L and starts with Windows; the window is there in about half a second. Switching it on
takes the key off the Start menu shortcut; Windows may hold on to it until the next sign-in.
Off: the process stops and the shortcut gets the key back. While it runs, a tray icon: click to
open Quick label; its menu stops it until the next sign-in or switches staying ready off.

## Install (Windows)

Put `quicklabel.exe` and `install.ps1` in one folder and run `install.ps1` (right click, Run
with PowerShell): it copies the exe and puts Quick label in the Start menu. The hotkey: Settings
→ **Stay ready in the background** (above). One file, nothing else
needed: the fonts and texts are inside the exe. Settings: `%APPDATA%\iotta\quicklabel.json`.

## Build

`build.rs` links `assets/icon.ico` into the Windows exe (needs `x86_64-w64-mingw32-windres`).

```bash
cargo build --release -p quicklabel --target x86_64-pc-windows-gnu
cargo run -p quicklabel -- [--text "…"] [--screenshot shot.png] [--dark | --light]
```
