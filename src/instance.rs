//! Single-instance guard: a second copy exits instead of adding a second tray
//! icon that would fight the first one over the same registry values.

use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::Threading::CreateMutexW;

const MUTEX_NAME: &str = "Local\\ThemeToggle.6F1B2D0E-4C7A-4F5B-9E31-2A8D0C6B7F42";

pub struct SingleInstanceGuard {
    handle: HANDLE,
}

impl SingleInstanceGuard {
    pub fn acquire() -> anyhow::Result<Self> {
        let name = wide(MUTEX_NAME);
        let handle =
            unsafe { CreateMutexW(None, false, windows::core::PCWSTR::from_raw(name.as_ptr()))? };
        let already_running = unsafe { GetLastError() == ERROR_ALREADY_EXISTS };
        if already_running {
            unsafe {
                let _ = CloseHandle(handle);
            }
            anyhow::bail!("ThemeToggle is already running; use the existing tray icon");
        }
        Ok(Self { handle })
    }
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
