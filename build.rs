//! Gives the Windows exe its icon (`assets/icon.ico`) in Explorer, the Start menu and the
//! taskbar.
//!
//! Working principle: for a GNU Windows target, `windres` turns a one-line resource script
//! into an object file that is linked into the exe. Other targets need nothing; a missing
//! `windres` is a warning and an exe with the default icon.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if os != "windows" || env != "gnu" {
        return;
    }

    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("set by cargo"));
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("set by cargo"));
    let icon = manifest.join("assets/icon.ico");
    let script = out.join("icon.rc");
    let object = out.join("icon.o");
    std::fs::write(
        &script,
        format!(
            "1 ICON \"{}\"\n",
            icon.display().to_string().replace('\\', "/")
        ),
    )
    .expect("OUT_DIR is writable");

    let windres = if cfg!(windows) {
        "windres"
    } else {
        "x86_64-w64-mingw32-windres"
    };
    let status = Command::new(windres)
        .arg(&script)
        .args(["-O", "coff", "-o"])
        .arg(&object)
        .status();
    match status {
        Ok(status) if status.success() => {
            println!("cargo:rustc-link-arg-bins={}", object.display());
        }
        _ => println!("cargo:warning={windres} failed: the exe gets no icon"),
    }
}
