//! Optional Windows startup registration, scoped to this application and user.
use std::{os::windows::ffi::OsStrExt, path::Path};
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::ERROR_FILE_NOT_FOUND,
        System::Registry::{
            RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ,
            RRF_RT_REG_SZ,
        },
        UI::WindowsAndMessaging::{
            IsIconic, IsWindowVisible, PostMessageW, SetForegroundWindow, ShowWindowAsync, SW_HIDE,
            SW_MINIMIZE, SW_RESTORE, WM_CLOSE,
        },
    },
};

const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const RUN_VALUE: PCWSTR = w!("Zaaps Dofus 3");

fn window() -> Option<windows::Win32::Foundation::HWND> {
    let handle = crate::window_corners::find_window();
    (handle != 0).then_some(windows::Win32::Foundation::HWND(handle as *mut _))
}

pub fn reveal() {
    if let Some(window) = window() {
        unsafe {
            let _ = ShowWindowAsync(window, SW_RESTORE);
            let _ = SetForegroundWindow(window);
        }
    }
}

pub fn close_window() {
    if let Some(window) = window() {
        let _ = unsafe {
            PostMessageW(
                Some(window),
                WM_CLOSE,
                windows::Win32::Foundation::WPARAM(0),
                windows::Win32::Foundation::LPARAM(0),
            )
        };
    }
}

/// Runs on the shortcut thread, so restoration never depends on drawing a hidden UI.
pub fn toggle_window(can_hide: bool) -> bool {
    let Some(window) = window() else { return true };
    unsafe {
        if !IsWindowVisible(window).as_bool() || IsIconic(window).as_bool() {
            reveal();
            true
        } else {
            let _ = ShowWindowAsync(window, if can_hide { SW_HIDE } else { SW_MINIMIZE });
            !can_hide
        }
    }
}

pub fn startup_command(exe: &Path) -> Result<String, String> {
    let path = exe
        .to_str()
        .ok_or("Chemin de l'application non pris en charge")?;
    if path.contains(['"', '\r', '\n', '\0']) || !exe.is_absolute() {
        return Err("Chemin de l'application invalide".into());
    }
    Ok(format!("\"{path}\" --tray"))
}

pub fn startup_enabled() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let Ok(expected) = startup_command(&exe) else {
        return false;
    };
    registered_startup().as_deref() == Some(&expected)
}

pub fn startup_registered() -> bool {
    registered_startup().is_some()
}

fn registered_startup() -> Option<String> {
    let mut value = [0u16; 4096];
    let mut bytes = std::mem::size_of_val(&value) as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            RUN_KEY,
            RUN_VALUE,
            RRF_RT_REG_SZ,
            None,
            Some(value.as_mut_ptr().cast()),
            Some(&mut bytes),
        )
    };
    status.is_ok().then(|| {
        String::from_utf16_lossy(
            &value[..value.iter().position(|c| *c == 0).unwrap_or(value.len())],
        )
    })
}

pub fn set_startup(enabled: bool) -> Result<(), String> {
    let status = if enabled {
        let command = startup_command(&std::env::current_exe().map_err(|e| e.to_string())?)?;
        let value: Vec<u16> = std::ffi::OsStr::new(&command)
            .encode_wide()
            .chain(Some(0))
            .collect();
        unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                RUN_KEY,
                RUN_VALUE,
                REG_SZ.0,
                Some(value.as_ptr().cast()),
                (value.len() * 2) as u32,
            )
        }
    } else {
        unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, RUN_VALUE) }
    };
    if status.is_ok() || (!enabled && status == ERROR_FILE_NOT_FOUND) {
        Ok(())
    } else {
        Err(format!("Démarrage Windows non modifié : {}", status.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "crée une fenêtre et une icône Windows temporaires"]
    fn native_tray_and_hidden_window_restoration() {
        use windows::Win32::{
            Foundation::{HWND, LPARAM, WPARAM},
            UI::WindowsAndMessaging::*,
        };
        let root = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!("Zaaps Dofus 3"),
                WS_OVERLAPPEDWINDOW,
                60,
                60,
                420,
                300,
                None,
                None,
                None,
                None,
            )
        }
        .unwrap();
        struct Cleanup(HWND);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = unsafe { DestroyWindow(self.0) };
            }
        }
        let _cleanup = Cleanup(root);
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (tray, _) = crate::tray::start(eframe::egui::Context::default(), stop.clone()).unwrap();
        struct TrayCleanup(
            crate::tray::Tray,
            std::sync::Arc<std::sync::atomic::AtomicBool>,
        );
        impl Drop for TrayCleanup {
            fn drop(&mut self) {
                self.1.store(true, std::sync::atomic::Ordering::Relaxed);
                self.0.shutdown();
            }
        }
        let _tray_cleanup = TrayCleanup(tray.clone(), stop);
        assert!(tray.available());
        let pump = || unsafe {
            let until = std::time::Instant::now() + std::time::Duration::from_millis(60);
            while std::time::Instant::now() < until {
                let mut message = MSG::default();
                while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                    DispatchMessageW(&message);
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        };
        reveal();
        pump();
        assert!(unsafe { IsWindowVisible(root) }.as_bool());
        assert!(!toggle_window(true));
        pump();
        assert!(!unsafe { IsWindowVisible(root) }.as_bool());
        assert!(toggle_window(true));
        pump();
        assert!(unsafe { IsWindowVisible(root) }.as_bool());
        assert!(toggle_window(false));
        pump();
        assert!(unsafe { IsIconic(root) }.as_bool());
        reveal();
        pump();
        assert!(!unsafe { IsIconic(root) }.as_bool());
        let _ = unsafe { PostMessageW(Some(root), WM_NULL, WPARAM(0), LPARAM(0)) };
    }
    #[test]
    fn startup_quotes_the_executable_and_only_adds_the_tray_flag() {
        assert_eq!(
            startup_command(Path::new(r"C:\Mes applications\Zaaps Dofus 3.exe")).unwrap(),
            "\"C:\\Mes applications\\Zaaps Dofus 3.exe\" --tray"
        );
        assert!(startup_command(Path::new("relative.exe")).is_err());
        assert!(startup_command(Path::new("C:\\bad\".exe")).is_err());
    }
}
