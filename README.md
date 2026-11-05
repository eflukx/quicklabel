# Quick label

Type a few lines, press **Ctrl+Enter**: printed on the Brother QL label printer.

- Enter starts a new line; `**bold**`, `*italic*`.
- Ctrl+D inserts today's date, Ctrl+T the time.
- Ctrl+Enter prints and closes; Esc closes.
- The text runs along the tape, as large as it fits: more lines, smaller letters. Continuous
  tape gets as long as the text; a die-cut label is filled in whichever direction gives the
  larger letters. The tape is the one the printer reports.
- The first time: open Printers and add the printer's address (for example `192.168.1.40`).
  Several printers: click the one to print on.
- English or Dutch (EN / NL, bottom right; at first the system's language).

## Install (Windows)

Put `quicklabel.exe` and `install.ps1` in one folder and run `install.ps1` (right click, Run
with PowerShell). Then **Ctrl+Alt+L** opens Quick label from anywhere. One file, nothing else
needed: the fonts and texts are inside the exe. Settings: `%APPDATA%\iotta\quicklabel.json`.

## Build

```bash
cargo build --release -p quicklabel --target x86_64-pc-windows-gnu
cargo run -p quicklabel -- [--text "…"] [--screenshot shot.png]
```
