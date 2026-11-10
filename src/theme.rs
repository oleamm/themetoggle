//! Windows light/dark appearance control.
//!
//! Windows keeps the per-user appearance in the registry under
//! `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize`:
//! `AppsUseLightTheme` drives application chrome, `SystemUsesLightTheme` drives
//! the shell (taskbar, Start, Action Center). `0` is dark, `1` is light.
//!
//! Writing those values alone is not enough: running processes only re-read
//! them when they receive `WM_SETTINGCHANGE` with `lParam = "ImmersiveColorSet"`,
//! which is exactly what the Settings app broadcasts. We do the same, so the
//! change is visible immediately instead of after a sign-out.

use anyhow::{Context, Result};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_DWORD, REG_SAM_FLAGS, REG_VALUE_TYPE,
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
};
use windows::core::{PCWSTR, w};

const PERSONALIZE: PCWSTR =
    w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
const APPS_USE_LIGHT: PCWSTR = w!("AppsUseLightTheme");
const SYSTEM_USES_LIGHT: PCWSTR = w!("SystemUsesLightTheme");
const IMMERSIVE_COLOR_SET: &str = "ImmersiveColorSet";
const BROADCAST_TIMEOUT_MS: u32 = 2_000;

/// Whether applications currently use the dark appearance. `None` when the
/// value is missing or unreadable.
pub fn apps_are_dark() -> Option<bool> {
    read_flag(APPS_USE_LIGHT).map(is_dark_value)
}

/// Whether the shell (taskbar, Start, tray) currently uses the dark appearance.
pub fn system_is_dark() -> Option<bool> {
    read_flag(SYSTEM_USES_LIGHT).map(is_dark_value)
}

/// Background the tray icon is drawn on. The taskbar follows the system value,
/// so a custom "light apps on a dark taskbar" setup must pick the light glyph.
pub fn taskbar_is_dark() -> bool {
    system_is_dark()
        .or_else(apps_are_dark)
        .unwrap_or(false)
}

/// Switch both application and shell appearance, then tell running programs.
pub fn set_dark(dark: bool) -> Result<()> {
    let value = registry_value(dark);
    let key = Key::open(KEY_READ | KEY_SET_VALUE)?;
    key.write_dword(APPS_USE_LIGHT, value)
        .context("failed to write AppsUseLightTheme")?;
    key.write_dword(SYSTEM_USES_LIGHT, value)
        .context("failed to write SystemUsesLightTheme")?;
    drop(key);
    broadcast_theme_change();
    Ok(())
}

/// Flip the current appearance and report the new state.
pub fn toggle() -> Result<bool> {
    let target = !apps_are_dark().unwrap_or(false);
    set_dark(target)?;
    Ok(target)
}

fn is_dark_value(light: u32) -> bool {
    light == 0
}

fn registry_value(dark: bool) -> u32 {
    u32::from(!dark)
}

fn read_flag(name: PCWSTR) -> Option<u32> {
    let key = Key::open(KEY_READ).ok()?;
    let mut value_type = REG_VALUE_TYPE(0);
    let mut value: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegQueryValueExW(
            key.raw(),
            name,
            None,
            Some(&mut value_type as *mut REG_VALUE_TYPE),
            Some(&mut value as *mut u32 as *mut u8),
            Some(&mut size),
        )
    };
    if status.0 != 0 || value_type != REG_DWORD {
        return None;
    }
    Some(value)
}

fn broadcast_theme_change() {
    let parameter = wide(IMMERSIVE_COLOR_SET);
    let result = unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            WPARAM(0),
            LPARAM(parameter.as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            BROADCAST_TIMEOUT_MS,
            None,
        )
    };
    if result.0 == 0 {
        log::warn!("WM_SETTINGCHANGE broadcast did not reach any window");
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

struct Key(HKEY);

impl Key {
    fn open(access: REG_SAM_FLAGS) -> Result<Self> {
        let mut raw = HKEY::default();
        unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, PERSONALIZE, Some(0), access, &mut raw) }
            .ok()
            .context("cannot open HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize")?;
        Ok(Self(raw))
    }

    fn raw(&self) -> HKEY {
        self.0
    }

    fn write_dword(&self, name: PCWSTR, value: u32) -> windows::core::Result<()> {
        let bytes = value.to_ne_bytes();
        unsafe { RegSetValueExW(self.0, name, Some(0), REG_DWORD, Some(bytes.as_slice())).ok() }
    }
}

impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{is_dark_value, registry_value, wide};

    #[test]
    fn registry_values_use_zero_for_dark() {
        assert_eq!(registry_value(true), 0);
        assert_eq!(registry_value(false), 1);
        assert!(is_dark_value(0));
        assert!(!is_dark_value(1));
    }

    #[test]
    fn wide_strings_are_nul_terminated_utf16() {
        let value = wide("ImmersiveColorSet");
        assert_eq!(value.last(), Some(&0));
        assert_eq!(value.len(), "ImmersiveColorSet".len() + 1);
    }
}
