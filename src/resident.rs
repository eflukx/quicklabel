//! Resident mode: a small background process holds the hotkey, so Quick label opens at once.
//!
//! How to use: switched on and off in the window (Settings); on Windows only. On: the process
//! `quicklabel --resident` starts now and with every Windows login (the `Run` key), and the
//! Start menu shortcut loses its key. Off: the process stops, the shortcut gets its key back.
//!
//! Working principle: a shortcut's key is handled by Explorer, which takes seconds to start the
//! program. The resident process registers the key itself (`global-hotkey`, a winit event loop
//! without a window) and on a press starts the window, as fast as the Start menu does. It shows
//! a tray icon: a click opens the window, its menu stops the process (until the next sign-in)
//! or switches resident mode off. The
//! window and the resident each listen on a fixed local port: a second window tells the first
//! to come forward and quits, a second resident quits, and the window stops the resident with
//! "quit". Only 127.0.0.1: nothing outside the PC can reach them.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

/// Where the open window listens ("show").
pub const WINDOW_PORT: u16 = 47_231;

/// Where the resident process listens ("quit", "ping").
pub const RESIDENT_PORT: u16 = 47_232;

fn local(port: u16) -> SocketAddr {
    SocketAddr::from((Ipv4Addr::LOCALHOST, port))
}

/// Takes `port` for this process: `None` when another one has it.
#[must_use]
pub fn claim(port: u16) -> Option<TcpListener> {
    TcpListener::bind(local(port)).ok()
}

/// Calls `on_message` with every message to `listener`, on a thread of its own.
pub fn serve(listener: TcpListener, on_message: impl Fn(&str) + Send + 'static) {
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut text = String::new();
            let _ = stream.take(64).read_to_string(&mut text);
            on_message(text.trim());
        }
    });
}

/// Sends `message` to the process on `port`; whether one took it.
#[must_use]
pub fn send(port: u16, message: &str) -> bool {
    TcpStream::connect_timeout(&local(port), Duration::from_millis(300))
        .and_then(|mut stream| stream.write_all(message.as_bytes()))
        .is_ok()
}

/// Whether the resident process runs.
#[must_use]
pub fn is_running() -> bool {
    send(RESIDENT_PORT, "ping")
}

#[cfg(windows)]
pub use windows::{disable, enable, run};

#[cfg(windows)]
mod windows {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    use std::time::{Duration, Instant};

    use crate::i18n::{t, tf};
    use global_hotkey::hotkey::HotKey;
    use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
    use winit::application::ApplicationHandler;
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, EventLoop};
    use winit::window::WindowId;

    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{
        Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    };

    use super::{RESIDENT_PORT, WINDOW_PORT, claim, is_running, send, serve};

    /// No console window for the helper programs (reg, PowerShell).
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    /// The `Run` key's value.
    const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    const RUN_NAME: &str = "Quick label";

    enum Wake {
        /// The hotkey or a click on the tray icon: the window.
        Open,
        /// Stop until the next sign-in.
        Quit,
        /// Resident mode off, then stop.
        SwitchOff,
    }

    struct Resident {
        hotkey: String,
        /// Kept: dropping it removes the icon.
        _tray: Option<TrayIcon>,
    }

    impl ApplicationHandler<Wake> for Resident {
        fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

        fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}

        fn user_event(&mut self, event_loop: &ActiveEventLoop, event: Wake) {
            match event {
                // The open window comes forward; else a new one starts.
                Wake::Open => {
                    if !send(WINDOW_PORT, "show")
                        && let Ok(exe) = std::env::current_exe()
                    {
                        let _ = Command::new(exe).spawn();
                    }
                }
                Wake::Quit => event_loop.exit(),
                Wake::SwitchOff => {
                    let mut settings = crate::settings::Settings::load();
                    settings.resident = false;
                    let _ = settings.save();
                    // Not running any more by then: the "quit" it sends finds nobody.
                    let _ = disable(&self.hotkey);
                    event_loop.exit();
                }
            }
        }
    }

    /// The resident process: holds `hotkey` until told to quit. Returns at once when another
    /// resident runs.
    pub fn run(hotkey: &str) -> Result<(), String> {
        let Some(listener) = claim(RESIDENT_PORT) else {
            return Ok(());
        };
        let key: HotKey = hotkey
            .parse()
            .map_err(|error| format!("{hotkey}: {error}"))?;
        let event_loop = EventLoop::<Wake>::with_user_event()
            .build()
            .map_err(|error| error.to_string())?;
        // On the event loop's thread, as Windows wants.
        let manager = GlobalHotKeyManager::new().map_err(|error| error.to_string())?;
        manager
            .register(key)
            .map_err(|error| format!("{hotkey}: {error}"))?;

        let proxy = event_loop.create_proxy();
        serve(listener, move |message| {
            if message == "quit" {
                let _ = proxy.send_event(Wake::Quit);
            }
        });
        let proxy = event_loop.create_proxy();
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            if event.state == HotKeyState::Pressed {
                let _ = proxy.send_event(Wake::Open);
            }
        }));

        // Without a tray icon (Explorer not there yet, say) the key still works.
        let shown = crate::settings::Settings::load().hotkey_shown();
        let tray = tray(&shown, &event_loop).ok();
        event_loop
            .run_app(&mut Resident {
                hotkey: hotkey.to_owned(),
                _tray: tray,
            })
            .map_err(|error| error.to_string())
    }

    /// The tray icon: a left click opens the window; the menu opens it, stops the process or
    /// switches resident mode off.
    fn tray(hotkey: &str, event_loop: &EventLoop<Wake>) -> Result<TrayIcon, String> {
        let open = MenuItem::new(t("Open Quick label"), true, None);
        let stop = MenuItem::new(t("Stop until the next sign-in"), true, None);
        let off = MenuItem::new(t("Switch off staying ready"), true, None);
        let menu = Menu::new();
        menu.append_items(&[&open, &PredefinedMenuItem::separator(), &stop, &off])
            .map_err(|error| error.to_string())?;

        let (open, stop, off) = (open.id().clone(), stop.id().clone(), off.id().clone());
        let proxy = event_loop.create_proxy();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let wake = if event.id == open {
                Wake::Open
            } else if event.id == stop {
                Wake::Quit
            } else if event.id == off {
                Wake::SwitchOff
            } else {
                return;
            };
            let _ = proxy.send_event(wake);
        }));
        let proxy = event_loop.create_proxy();
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = proxy.send_event(Wake::Open);
            }
        }));

        let image = image::load_from_memory(include_bytes!("../assets/icon.png"))
            .map_err(|error| error.to_string())?
            .resize(32, 32, image::imageops::FilterType::Lanczos3)
            .to_rgba8();
        let (width, height) = image.dimensions();
        let icon =
            Icon::from_rgba(image.into_raw(), width, height).map_err(|error| error.to_string())?;
        TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip(tf("Quick label · {hotkey}", &[("hotkey", &hotkey)]))
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .build()
            .map_err(|error| error.to_string())
    }

    /// Switches resident mode on: at every login, now, and the shortcut without its key.
    pub fn enable(hotkey: &str) -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|error| error.to_string())?;
        let command = format!("\"{}\" --resident", exe.display());
        quiet(
            "reg",
            &[
                "add", RUN_KEY, "/v", RUN_NAME, "/t", "REG_SZ", "/d", &command, "/f",
            ],
        )?;
        // Explorer would take the key too.
        shortcut_key("")?;

        if !is_running() {
            Command::new(&exe)
                .arg("--resident")
                .spawn()
                .map_err(|error| error.to_string())?;
        }
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if is_running() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Err(tf(
            "The background process didn't start: {hotkey} is taken. Just removed from the shortcut, Windows keeps it until you sign in again; else another program has it.",
            &[("hotkey", &hotkey)],
        ))
    }

    /// Switches resident mode off: not at login, stopped, and the shortcut gets `hotkey` back.
    pub fn disable(hotkey: &str) -> Result<(), String> {
        // Not there is fine.
        let _ = quiet("reg", &["delete", RUN_KEY, "/v", RUN_NAME, "/f"]);
        let _ = send(RESIDENT_PORT, "quit");
        shortcut_key(hotkey)
    }

    /// Sets the Start menu shortcut's key ("" for none), when the shortcut is there.
    fn shortcut_key(hotkey: &str) -> Result<(), String> {
        let script = format!(
            "$p = Join-Path ([Environment]::GetFolderPath('Programs')) 'Quick label.lnk'; \
             if (Test-Path $p) {{ $s = (New-Object -ComObject WScript.Shell).CreateShortcut($p); \
             $s.Hotkey = '{hotkey}'; $s.Save() }}"
        );
        quiet(
            "powershell",
            &[
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &script,
            ],
        )
    }

    /// Runs a helper program without a console window; its error output when it fails.
    fn quiet(program: &str, args: &[&str]) -> Result<(), String> {
        let output = Command::new(program)
            .args(args)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|error| format!("{program}: {error}"))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(format!(
                "{program}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_claimed_port_takes_messages_and_cant_be_claimed_twice() {
        // A port of its own, so a running Quick label doesn't interfere.
        let port = 47_299;
        let listener = claim(port).expect("free port");
        assert!(claim(port).is_none());

        let (sender, receiver) = std::sync::mpsc::channel();
        serve(listener, move |message| {
            let _ = sender.send(message.to_owned());
        });
        assert!(send(port, "show"));
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(2)).as_deref(),
            Ok("show")
        );
    }
}
