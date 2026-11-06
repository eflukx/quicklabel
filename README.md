# Quick label

Type a few lines, press **Ctrl+Enter**: printed on the Brother QL label printer.

- It opens with the last text, selected: typing replaces it, Ctrl+Enter prints it again.
  The number of labels starts at 1.
- Enter starts a new line; `**bold**`, `*italic*`.
- Ctrl+D inserts today's date, Ctrl+T the time.
- Ctrl+Enter prints and closes; Esc closes.
- The text is as large as it fits: more lines, smaller letters. The tape is the one the printer
  reports.
- Lengthwise (along the label's long side) or Sideways (Ctrl+R). On continuous tape the label
  is at most 1×, 2× (default) or 3× the tape's width long, or free; the text shrinks to fit.
  Die-cut labels have their own size.
- Cut: after each label (default), once after the batch, or off.
- Direction, length and cut are kept per tape size.
- Several labels at once: − n + next to Print (Ctrl+Up / Ctrl+Down), one job. `{n}` in the
  text is the label's number from 1; `{n+12}` adds 12 (13, 14, …); `{n+007}` keeps the zeros (008, …).
- The first time: Printers (top right) opens by itself; add the printer's address (for example `192.168.1.40`).
  Several printers: click the one to print on.
- English or Dutch (EN / NL, top right; at first the system's language). Light or dark as
  Windows is set, or by the moon / sun button (remembered).

## Instantly under the hotkey (resident mode)

Windows takes a few seconds for a shortcut's key. In Quick label: Settings → **Stay ready in
the background**. A small process (`quicklabel --resident`, a few MB, idle) then holds
Ctrl+Alt+L and starts with Windows; the window is there in about half a second. Switching it on
takes the key off the Start menu shortcut; Windows may hold on to it until the next sign-in.
Off: the process stops and the shortcut gets the key back.

## Install (Windows)

Put `quicklabel.exe` and `install.ps1` in one folder and run `install.ps1` (right click, Run
with PowerShell). Then **Ctrl+Alt+L** opens Quick label from anywhere. One file, nothing else
needed: the fonts and texts are inside the exe. Settings: `%APPDATA%\iotta\quicklabel.json`.

## Build

`build.rs` links `assets/icon.ico` into the Windows exe (needs `x86_64-w64-mingw32-windres`).

```bash
cargo build --release -p quicklabel --target x86_64-pc-windows-gnu
cargo run -p quicklabel -- [--text "…"] [--screenshot shot.png] [--dark | --light]
```
