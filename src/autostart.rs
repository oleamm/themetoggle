//! Autostart through the per-user Run key.
//!
//! The value is stored as a quoted executable path without shell syntax, so it
//! is independent of PowerShell or cmd.

use anyhow::{Context, Result, bail};
use std::ffi::OsString;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
    REG_VALUE_TYPE, RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
    RegSetValueExW,
};
use windows::core::w;

const RUN_KEY: windows::core::PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const VALUE_NAME: windows::core::PCWSTR = w!("ThemeToggle");

/// Whether the Run entry exists and points at this executable.
pub fn is_enabled() -> Result<bool> {
    let current_exe = current_exe().context("cannot determine the current executable")?;
    let key = match open_run_key(KEY_READ) {
        Ok(key) => key,
        Err(error) if is_not_found(&error) => return Ok(false),
        Err(error) => return Err(error).context("failed to open the Windows Run key"),
    };
    let value = match read_string_value(key.raw()) {
        Ok(value) => value,
        Err(error) if is_not_found(&error) => return Ok(false),
        Err(error) => return Err(error).context("failed to read the ThemeToggle Run value"),
    };
    let saved_exe = match first_command_line_argument(&value) {
        Ok(path) => path,
        Err(_) => return Ok(false),
    };
    Ok(paths_match(&current_exe, &saved_exe))
}

/// Adds or removes the Run entry for the current executable.
pub fn set_enabled(enabled: bool) -> Result<()> {
    let key = if enabled {
        create_run_key().context("failed to open the Windows Run key for writing")?
    } else {
        match open_run_key(KEY_SET_VALUE) {
            Ok(key) => key,
            Err(error) if is_not_found(&error) => return Ok(()),
            Err(error) => return Err(error).context("failed to open the Windows Run key"),
        }
    };

    if enabled {
        let command = quoted_executable(&current_exe()?)
            .context("cannot build a safe autostart command line")?;
        let bytes = utf16_bytes(&command);
        unsafe {
            RegSetValueExW(key.raw(), VALUE_NAME, Some(0), REG_SZ, Some(&bytes))
                .ok()
                .context("failed to save the ThemeToggle Run value")?;
        }
    } else {
        unsafe {
            let status = RegDeleteValueW(key.raw(), VALUE_NAME);
            if status != ERROR_FILE_NOT_FOUND {
                status
                    .ok()
                    .context("failed to remove the ThemeToggle Run value")?;
            }
        }
    }
    Ok(())
}

fn current_exe() -> Result<PathBuf> {
    std::env::current_exe().context("current_exe failed")
}

fn create_run_key() -> windows::core::Result<RegistryKey> {
    let mut raw = HKEY::default();
    unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            RUN_KEY,
            Some(0),
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut raw,
            None,
        )
        .ok()?;
    }
    Ok(RegistryKey(raw))
}

fn open_run_key(
    access: windows::Win32::System::Registry::REG_SAM_FLAGS,
) -> windows::core::Result<RegistryKey> {
    let mut raw = HKEY::default();
    unsafe {
        RegOpenKeyExW(HKEY_CURRENT_USER, RUN_KEY, Some(0), access, &mut raw).ok()?;
    }
    Ok(RegistryKey(raw))
}

fn read_string_value(key: HKEY) -> windows::core::Result<Vec<u16>> {
    let mut value_type = REG_VALUE_TYPE(0);
    let mut byte_len = 0u32;
    unsafe {
        RegQueryValueExW(
            key,
            VALUE_NAME,
            None,
            Some(&mut value_type as *mut REG_VALUE_TYPE),
            None,
            Some(&mut byte_len),
        )
        .ok()?;
    }
    if value_type != REG_SZ {
        bail_windows("autostart value is not REG_SZ")?;
    }
    if byte_len % 2 != 0 {
        bail_windows("autostart value has an invalid UTF-16 length")?;
    }
    let mut value = vec![0u16; (byte_len / 2) as usize];
    unsafe {
        RegQueryValueExW(
            key,
            VALUE_NAME,
            None,
            Some(&mut value_type as *mut REG_VALUE_TYPE),
            Some(value.as_mut_ptr().cast()),
            Some(&mut byte_len),
        )
        .ok()?;
    }
    value.truncate((byte_len / 2) as usize);
    while value.last() == Some(&0) {
        value.pop();
    }
    Ok(value)
}

fn quoted_executable(path: &Path) -> Result<Vec<u16>> {
    let wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    if wide.iter().any(|&unit| unit == 0 || unit == b'"' as u16) {
        bail!("executable path contains an unsupported NUL or quote character");
    }
    let mut command = Vec::with_capacity(wide.len() + 2);
    command.push(b'"' as u16);
    command.extend_from_slice(&wide);
    let trailing_backslashes = wide
        .iter()
        .rev()
        .take_while(|&&unit| unit == b'\\' as u16)
        .count();
    command.extend(std::iter::repeat_n(b'\\' as u16, trailing_backslashes));
    command.push(b'"' as u16);
    command.push(0);
    Ok(command)
}

fn first_command_line_argument(command: &[u16]) -> Result<PathBuf> {
    let mut index = 0;
    while command
        .get(index)
        .is_some_and(|&unit| is_ascii_whitespace(unit))
    {
        index += 1;
    }
    let quoted = command.get(index) == Some(&(b'"' as u16));
    if quoted {
        index += 1;
    }
    let start = index;
    while let Some(&unit) = command.get(index) {
        if (quoted && unit == b'"' as u16) || (!quoted && is_ascii_whitespace(unit)) {
            break;
        }
        index += 1;
    }
    if quoted && command.get(index) != Some(&(b'"' as u16)) {
        bail!("autostart command line has an unterminated quote");
    }
    if start == index {
        bail!("autostart command line has no executable path");
    }
    let path = OsString::from_wide(&command[start..index]);
    if path.encode_wide().any(|unit| unit == 0) {
        bail!("autostart command line contains NUL");
    }
    Ok(PathBuf::from(path))
}

fn is_ascii_whitespace(unit: u16) -> bool {
    matches!(unit, 0x09..=0x0D | 0x20)
}

fn paths_match(current: &Path, saved: &Path) -> bool {
    let current = std::fs::canonicalize(current).unwrap_or_else(|_| current.to_path_buf());
    let saved = std::fs::canonicalize(saved).unwrap_or_else(|_| saved.to_path_buf());
    normalize_path(&current) == normalize_path(&saved)
}

fn normalize_path(path: &Path) -> Vec<u16> {
    let mut normalized: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .map(|unit| match unit {
            0x2F => 0x5C,
            0x41..=0x5A => unit + 0x20,
            _ => unit,
        })
        .collect();
    while normalized.len() > 3 && normalized.last() == Some(&(b'\\' as u16)) {
        normalized.pop();
    }
    normalized
}

fn utf16_bytes(value: &[u16]) -> Vec<u8> {
    value.iter().flat_map(|unit| unit.to_le_bytes()).collect()
}

fn is_not_found(error: &windows::core::Error) -> bool {
    error.code() == windows::core::HRESULT::from_win32(ERROR_FILE_NOT_FOUND.0)
}

fn bail_windows(message: &str) -> windows::core::Result<()> {
    Err(windows::core::Error::new(
        windows::core::HRESULT(0x80004005u32 as i32),
        message,
    ))
}

struct RegistryKey(HKEY);

impl RegistryKey {
    fn raw(&self) -> HKEY {
        self.0
    }
}

impl Drop for RegistryKey {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{first_command_line_argument, quoted_executable};
    use std::path::Path;

    #[test]
    fn quoted_command_line_round_trips() {
        let command = quoted_executable(Path::new(r"C:\Apps\ThemeToggle.exe")).unwrap();
        let parsed = first_command_line_argument(&command).unwrap();
        assert_eq!(parsed.as_path(), Path::new(r"C:\Apps\ThemeToggle.exe"));
    }

    #[test]
    fn unquoted_command_line_with_spaces_keeps_only_the_executable() {
        let command: Vec<u16> = r"C:\Apps\Theme Toggle.exe --flag"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let parsed = first_command_line_argument(&command).unwrap();
        assert_eq!(parsed.as_path(), Path::new(r"C:\Apps\Theme"));
    }
}
