# CLAUDE.md

**Quick label**: iotta's one-window tool to print a few lines on a Brother QL label printer
(Ctrl+Enter), from a hotkey or the command line. Rust, egui. Open source (MIT/Apache-2.0).

- Windows is the production platform: develop on WSL, cross-compile with
  `--target x86_64-pc-windows-gnu`, test the `.exe`. Every build also goes to
  `C:\Users\Rogier\chopshop\quicklabel\` (with `quicklabel.com` and `install.ps1`).
- Printing and label drawing live in ql-raster (`/mnt/c/Users/Rogier/code/ql-raster`, path
  dependency; build it with `RUSTC_WRAPPER= CARGO_TARGET_DIR=~/.cache/ql-raster-target`).
  Generic printing code goes there, not here. Never depend on ChopShop (iotta-internal).
- Map: `src/app.rs` window, `src/cli.rs` command line, `src/printer.rs` the print worker and
  polling, `src/resident.rs` background process and hotkey, `src/i18n/` texts, `i18n/nl.toml`
  Dutch, `src/bin/quicklabel-console.rs` the `.com` launcher, `install.ps1`.
- GUI texts English in the code, `t("…")` / `tf("…", &[…])`; Dutch in `i18n/nl.toml`; the test
  `every_text_has_a_dutch_one` fails for a missing one.
- After a user-visible change: bump `version` in `Cargo.toml`, add a row to `REVISIONS.md`.

```bash
cargo test && cargo clippy --all-targets -- -D warnings && cargo +nightly fmt
cargo build --release --target x86_64-pc-windows-gnu
```
