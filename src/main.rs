//! ThemeToggle: a one-icon tray app that flips Windows between the light and
//! dark appearance.

#![windows_subsystem = "windows"]

mod autostart;
mod icons;
mod instance;
mod logfile;
mod theme;
mod tray;

use anyhow::Result;
use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
use windows::core::PCWSTR;

fn main() {
    logfile::init();
    if let Err(error) = run() {
        log::error!("fatal: {error:#}");
        show_error(&format!("{error:#}"));
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    match instance::SingleInstanceGuard::acquire() {
        // Another copy already owns the tray icon: stay silent, that is not an
        // error the user needs to see.
        Err(error) => {
            log::info!("{error}");
            Ok(())
        }
        Ok(_guard) => {
            log::info!(
                "starting with the {} theme",
                if theme::apps_are_dark().unwrap_or(false) { "dark" } else { "light" }
            );
            tray::run()
        }
    }
}

fn show_error(message: &str) {
    let text = wide(message);
    let caption = wide("ThemeToggle");
    unsafe {
        MessageBoxW(
            None,
            PCWSTR::from_raw(text.as_ptr()),
            PCWSTR::from_raw(caption.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
