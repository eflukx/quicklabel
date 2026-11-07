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
- Typeface (right of the hints under the text): Barlow (ours, narrow), Roboto or Serif (Source
  Serif 4); one for the whole label, remembered. Roboto and Serif have real italics.
- Several labels at once: − n + next to Print (Ctrl+Up / Ctrl+Down; Ctrl+N or a click on the number to type it, Enter or Esc back), one job. `{n}` in the
  text is the label's number from 1; `{n+12}` adds 12 (13, 14, …); `{n+007}` keeps the zeros (008, …).
- The first time: Settings (top right) opens by itself; Search the network finds Brother QL
  printers (this PC's network and that of printers already set), or add the printer's address (for example `192.168.1.40`).
  Several printers: the printer chip under the label opens the list (automatic: the first
  that is ready, or one of them). **USB**: a QL plugged in shows up in Settings by
  itself, with Add, as `usb:<serial>` (no Brother driver needed: Windows' own "USB Printing Support"; not with
  WinUSB/libusb from Zadig). **HQ** per printer (on by default): high
  resolution, 600 dpi along the tape, on models that have it (QL-710W, 720NW, 800 series);
  a little slower.
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
cargo build --release --target x86_64-pc-windows-gnu      # from Linux/WSL, with mingw-w64
cargo run -- [--text "…"] [--screenshot shot.png] [--dark | --light]
cargo test                                                # includes: every text has a Dutch one
```

Printing, label drawing and the typefaces come from
[ql-raster](https://github.com/eflukx/ql-raster).
Texts are English in the code, wrapped in `t("…")` / `tf("…")`, and Dutch in `i18n/nl.toml`.

## From the command line

With a text Quick label prints without the window; without options it opens the window.
`quicklabel --help` lists every option.

```bat
quicklabel "Rack 4" "**do not move**"            :: two lines, on the first printer that is ready
quicklabel -p 2 -n 10 "Box {n}"                  :: ten labels, Box 1 to Box 10, on printer 2
quicklabel --sideways --length 2 --cut batch "Spare parts"
quicklabel --font serif --no-hq "Archive\n2026"  :: \n is a new line too
echo Hello | quicklabel -                        :: the text from stdin
quicklabel --png label.png "Test"                :: the label as an image, not printed
quicklabel --list-printers                       :: the printers and their numbers for -p (--search: new ones)
```

- Every argument is a line. The options are the window's chips (`--sideways`,
  `--fill-lines`, `--cut each|batch|off`, `--length free|1|2|3`), `-n` labels, `--font
  barlow|roboto|serif`, `--hq` / `--no-hq`; each `--x` has a `--no-x`.
- What isn't given comes from the settings: the layout kept for the loaded tape, the typeface,
  HQ per printer. The command line changes no setting.
- `-p` takes a number from `--list-printers` or an address (`192.168.1.57`,
  `usb:000H6Z560817`); without it the first printer that is ready. A USB printer that is also
  on the network is printed on there.
- Exit code 0 printed, 1 not printed, 2 wrong options.
- `quicklabel.exe` is a window program, which cmd doesn't wait for. Next to it is
  `quicklabel.com`, a small console program: typing `quicklabel` runs that one (`.com` comes
  before `.exe`), it starts the exe, waits and passes on the output and exit code. The Start
  menu and the hotkey start the exe, without a console.

`QUICKLABEL_JOB=x.bin` saves the job's bytes instead of printing; `QL_USB_TRACE=1` logs every
USB read and write; `--abort usb:…` stops a job a USB printer hangs on.

## License

MIT or Apache-2.0, at your option. Barlow Semi Condensed is under the SIL Open Font License
(`assets/fonts/OFL-Barlow.txt`). The iotta name and logo are iotta's.
