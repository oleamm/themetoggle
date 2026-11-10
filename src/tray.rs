//! Tray icon, context menu and the Win32 message pump that drives both.
//!
//! `tray-icon` delivers mouse events through a global handler and `muda`
//! delivers menu events the same way. Both callbacks only post a thread
//! message, so the registry write and the `WM_SETTINGCHANGE` broadcast always
//! run on the pump thread instead of inside another window's procedure.

use std::time::{Duration, Instant};

use anyhow::Result;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, KillTimer, MSG, PostThreadMessageW, SetTimer, WM_APP, WM_TIMER,
};

use crate::{autostart, icons, theme};

const WM_TOGGLE: u32 = WM_APP + 1;
const WM_TOGGLE_AUTOSTART: u32 = WM_APP + 2;
const WM_QUIT_REQUESTED: u32 = WM_APP + 3;
const REFRESH_TIMER: usize = 1;
const REFRESH_INTERVAL_MS: u32 = 2_000;
/// A double click produces two `button up` events; without this guard it would
/// flip the theme twice and look like nothing happened.
const DOUBLE_CLICK_GUARD: Duration = Duration::from_millis(400);

/// Creates the tray icon and runs the message loop until the user quits.
pub fn run() -> Result<()> {
    let thread_id = unsafe { GetCurrentThreadId() };
    let mut tray = Tray::new()?;

    let toggle_id = tray.toggle_item.id().clone();
    let autostart_id = tray.autostart_item.id().clone();
    let quit_id = tray.quit_item.id().clone();

    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event
        {
            post(thread_id, WM_TOGGLE);
        }
    }));

    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let message = if event.id == toggle_id {
            WM_TOGGLE
        } else if event.id == autostart_id {
            WM_TOGGLE_AUTOSTART
        } else if event.id == quit_id {
            WM_QUIT_REQUESTED
        } else {
            return;
        };
        post(thread_id, message);
    }));

    let timer = unsafe { SetTimer(None, REFRESH_TIMER, REFRESH_INTERVAL_MS, None) };
    if timer == 0 {
        log::warn!("cannot start the theme refresh timer; external changes will not be tracked");
    }
    log::info!("tray ready on thread {thread_id}");

    let mut message = MSG::default();
    loop {
        let result = unsafe { GetMessageW(&mut message, None, 0, 0) };
        if result.0 == 0 || result.0 == -1 {
            break;
        }
        match message.message {
            WM_TOGGLE => tray.toggle(),
            WM_TOGGLE_AUTOSTART => tray.toggle_autostart(),
            WM_QUIT_REQUESTED => break,
            WM_TIMER if message.wParam.0 == REFRESH_TIMER => tray.refresh(),
            _ => unsafe {
                let _ = DispatchMessageW(&message);
            },
        }
    }

    if timer != 0 {
        unsafe {
            let _ = KillTimer(None, REFRESH_TIMER);
        }
    }
    log::info!("tray message loop stopped");
    Ok(())
}

fn post(thread_id: u32, message: u32) {
    if let Err(error) = unsafe { PostThreadMessageW(thread_id, message, WPARAM(0), LPARAM(0)) } {
        log::error!("cannot post thread message {message:#x}: {error}");
    }
}

struct Tray {
    tray: TrayIcon,
    toggle_item: MenuItem,
    autostart_item: CheckMenuItem,
    quit_item: MenuItem,
    apps_dark: bool,
    taskbar_dark: bool,
    autostart_enabled: bool,
    last_toggle: Option<Instant>,
}

impl Tray {
    fn new() -> Result<Self> {
        let menu = Menu::new();
        let toggle_item = MenuItem::with_id("toggle", "Switch theme", true, None);
        let autostart_item = CheckMenuItem::with_id(
            "autostart",
            "Start with Windows",
            true,
            autostart::is_enabled().unwrap_or(false),
            None,
        );
        let quit_item = MenuItem::with_id("quit", "Quit ThemeToggle", true, None);
        menu.append(&toggle_item)?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&autostart_item)?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&quit_item)?;

        let apps_dark = theme::apps_are_dark().unwrap_or(false);
        let taskbar_dark = theme::taskbar_is_dark();
        let autostart_enabled = autostart::is_enabled().unwrap_or(false);

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            // Left click is the switch itself, so it must not open the menu.
            .with_menu_on_left_click(false)
            .with_icon(icons::tray_icon(taskbar_dark))
            .with_tooltip("ThemeToggle")
            .build()?;

        let mut tray = Self {
            tray,
            toggle_item,
            autostart_item,
            quit_item,
            apps_dark,
            taskbar_dark,
            autostart_enabled,
            last_toggle: None,
        };
        tray.render();
        Ok(tray)
    }

    fn toggle(&mut self) {
        let now = Instant::now();
        if let Some(previous) = self.last_toggle {
            let elapsed = now.duration_since(previous);
            if elapsed < DOUBLE_CLICK_GUARD {
                log::debug!("ignoring a click {} ms after the previous one", elapsed.as_millis());
                return;
            }
        }
        self.last_toggle = Some(now);

        match theme::toggle() {
            Ok(dark) => {
                self.apps_dark = dark;
                self.taskbar_dark = theme::taskbar_is_dark();
                self.render();
                log::info!("switched to the {} theme", describe(dark));
            }
            Err(error) => log::error!("cannot switch the theme: {error:#}"),
        }
    }

    fn toggle_autostart(&mut self) {
        let target = !self.autostart_enabled;
        match autostart::set_enabled(target) {
            Ok(()) => {
                self.autostart_enabled = autostart::is_enabled().unwrap_or(target);
                self.autostart_item.set_checked(self.autostart_enabled);
                log::info!(
                    "autostart {}",
                    if self.autostart_enabled { "enabled" } else { "disabled" }
                );
            }
            Err(error) => log::error!("cannot change autostart: {error:#}"),
        }
    }

    /// Keeps the glyph honest when the theme is changed somewhere else.
    fn refresh(&mut self) {
        let apps_dark = theme::apps_are_dark().unwrap_or(self.apps_dark);
        let taskbar_dark = theme::taskbar_is_dark();
        if apps_dark == self.apps_dark && taskbar_dark == self.taskbar_dark {
            return;
        }
        self.apps_dark = apps_dark;
        self.taskbar_dark = taskbar_dark;
        self.render();
        log::info!("theme changed externally to {}", describe(apps_dark));
    }

    fn render(&mut self) {
        if let Err(error) = self.tray.set_icon(Some(icons::tray_icon(self.taskbar_dark))) {
            log::error!("cannot update the tray icon: {error}");
        }
        if let Err(error) = self.tray.set_tooltip(Some(self.tooltip())) {
            log::error!("cannot update the tray tooltip: {error}");
        }
        self.toggle_item.set_text(self.toggle_label());
    }

    fn tooltip(&self) -> String {
        format!(
            "ThemeToggle — {} theme. Click to switch to {}.",
            describe(self.apps_dark),
            describe(!self.apps_dark)
        )
    }

    fn toggle_label(&self) -> String {
        format!("Switch to {} theme", describe(!self.apps_dark))
    }
}

fn describe(dark: bool) -> &'static str {
    if dark { "dark" } else { "light" }
}
