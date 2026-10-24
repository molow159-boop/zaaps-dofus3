//! Notification-area icon with an independent message loop.
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::Duration,
};
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM},
        System::LibraryLoader::GetModuleHandleW,
        UI::{Shell::*, WindowsAndMessaging::*},
    },
};

const CALLBACK: u32 = WM_APP + 41;
const KEY_SELECT: u32 = NIN_SELECT | 1;

#[derive(Clone, Copy, Debug)]
pub enum Event {
    Show,
    ToggleAutomatic,
    CopyRoute,
    Quit,
}

enum Command {
    Tip(String),
    Notify(String, String),
}

#[derive(Clone)]
pub struct Tray {
    tx: mpsc::Sender<Command>,
    available: Arc<AtomicBool>,
    thread: Arc<Mutex<Option<thread::JoinHandle<()>>>>,
    controls: Arc<Mutex<Option<mpsc::Sender<crate::hotkeys::Control>>>>,
}

impl Tray {
    pub fn bind_controls(&self, controls: mpsc::Sender<crate::hotkeys::Control>) {
        if let Ok(mut sender) = self.controls.lock() {
            *sender = Some(controls);
        }
    }
    pub fn shutdown(&self) {
        if let Ok(mut thread) = self.thread.lock() {
            if let Some(thread) = thread.take() {
                let _ = thread.join();
            }
        }
    }
    pub fn available(&self) -> bool {
        self.available.load(Ordering::Relaxed)
    }
    pub fn tip(&self, text: impl Into<String>) {
        let _ = self.tx.send(Command::Tip(text.into()));
    }
    pub fn notify(&self, title: impl Into<String>, message: impl Into<String>) {
        let _ = self.tx.send(Command::Notify(title.into(), message.into()));
    }
}

fn wide<const N: usize>(dest: &mut [u16; N], text: &str) {
    dest.fill(0);
    // Do not cut a surrogate pair at the end of a Windows text buffer.
    let mut used = 0;
    for character in text.chars().filter(|c| *c != '\0') {
        let mut buf = [0u16; 2];
        let units = character.encode_utf16(&mut buf);
        if used + units.len() >= N {
            break;
        }
        dest[used..used + units.len()].copy_from_slice(units);
        used += units.len();
    }
}

struct State {
    events: mpsc::Sender<Event>,
    context: eframe::egui::Context,
    controls: Arc<Mutex<Option<mpsc::Sender<crate::hotkeys::Control>>>>,
}

unsafe extern "system" fn procedure(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
    if !pointer.is_null() && message == CALLBACK {
        let state = &*pointer;
        let event = match lparam.0 as u32 {
            WM_LBUTTONUP | NIN_SELECT | KEY_SELECT | NIN_BALLOONUSERCLICK => Some(Event::Show),
            WM_RBUTTONUP | WM_CONTEXTMENU => {
                if let Ok(menu) = CreatePopupMenu() {
                    let _ = AppendMenuW(menu, MF_STRING, 1, w!("Afficher Zaaps Dofus 3"));
                    let _ = AppendMenuW(menu, MF_STRING, 2, w!("Suspendre / reprendre Auto"));
                    let _ = AppendMenuW(menu, MF_STRING, 3, w!("Recopier le trajet courant"));
                    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
                    let _ = AppendMenuW(menu, MF_STRING, 4, w!("Quitter"));
                    let mut position = POINT::default();
                    let _ = GetCursorPos(&mut position);
                    let _ = SetForegroundWindow(hwnd);
                    let choice = TrackPopupMenu(
                        menu,
                        TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
                        position.x,
                        position.y,
                        None,
                        hwnd,
                        None,
                    )
                    .0;
                    let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
                    let _ = DestroyMenu(menu);
                    match choice {
                        1 => Some(Event::Show),
                        2 => Some(Event::ToggleAutomatic),
                        3 => Some(Event::CopyRoute),
                        4 => Some(Event::Quit),
                        _ => None,
                    }
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(event) = event {
            let controls = state.controls.lock().ok().and_then(|sender| sender.clone());
            match (event, controls) {
                (Event::Show, _) => {
                    crate::desktop::reveal();
                    let _ = state.events.send(Event::Show);
                }
                (Event::Quit, _) => {
                    crate::desktop::close_window();
                    let _ = state.events.send(Event::Quit);
                }
                (Event::ToggleAutomatic, Some(sender)) => {
                    let _ = sender.send(crate::hotkeys::Control::ToggleAutomatic);
                }
                (Event::CopyRoute, Some(sender)) => {
                    let _ = sender.send(crate::hotkeys::Control::Copy(
                        crate::commands::Action::Combined,
                    ));
                }
                (event, None) => {
                    let _ = state.events.send(event);
                }
            }
            state.context.request_repaint();
        }
        return LRESULT(0);
    }
    DefWindowProcW(hwnd, message, wparam, lparam)
}

pub fn start(
    context: eframe::egui::Context,
    stop: Arc<AtomicBool>,
) -> Result<(Tray, mpsc::Receiver<Event>), String> {
    let (tx, rx) = mpsc::channel();
    let (events, event_rx) = mpsc::channel();
    let (ready, ready_rx) = mpsc::sync_channel(1);
    let available = Arc::new(AtomicBool::new(false));
    let status = available.clone();
    let controls = Arc::new(Mutex::new(None));
    let callback_controls = controls.clone();
    let worker = thread::Builder::new()
        .name("notification-area".into())
        .spawn(move || unsafe {
            let result = (|| -> windows::core::Result<()> {
                let instance = GetModuleHandleW(None)?;
                let class = WNDCLASSW {
                    lpfnWndProc: Some(procedure),
                    hInstance: instance.into(),
                    lpszClassName: w!("ZaapsDofus3NotificationArea"),
                    ..Default::default()
                };
                RegisterClassW(&class);
                let hwnd = CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    class.lpszClassName,
                    w!("Zaaps notification"),
                    WINDOW_STYLE::default(),
                    0,
                    0,
                    0,
                    0,
                    None,
                    None,
                    Some(instance.into()),
                    None,
                )?;
                // MAKEINTRESOURCE(1): the icon embedded by winresource.
                let icon = LoadIconW(
                    Some(instance.into()),
                    PCWSTR(std::ptr::without_provenance(1)),
                )
                .or_else(|_| LoadIconW(None, IDI_APPLICATION))?;
                let mut state = Box::new(State {
                    events,
                    context,
                    controls: callback_controls,
                });
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, (&mut *state as *mut State) as isize);
                let mut data = NOTIFYICONDATAW {
                    cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                    hWnd: hwnd,
                    uID: 1,
                    uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
                    uCallbackMessage: CALLBACK,
                    hIcon: icon,
                    ..Default::default()
                };
                wide(&mut data.szTip, "Zaaps Dofus 3 — prêt à copier");
                let installed = Shell_NotifyIconW(NIM_ADD, &data).as_bool();
                status.store(installed, Ordering::Relaxed);
                if ready.send(installed).is_err() || !installed {
                    let _ = Shell_NotifyIconW(NIM_DELETE, &data);
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    let _ = DestroyWindow(hwnd);
                    return Ok(());
                }
                let restart = RegisterWindowMessageW(w!("TaskbarCreated"));
                let mut msg = MSG::default();
                let mut last_retry = std::time::Instant::now();
                while !stop.load(Ordering::Relaxed) {
                    while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                        if restart != 0 && msg.message == restart {
                            status.store(
                                Shell_NotifyIconW(NIM_ADD, &data).as_bool(),
                                Ordering::Relaxed,
                            );
                        }
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                    if !status.load(Ordering::Relaxed)
                        && last_retry.elapsed() >= Duration::from_secs(5)
                    {
                        status.store(
                            Shell_NotifyIconW(NIM_ADD, &data).as_bool(),
                            Ordering::Relaxed,
                        );
                        last_retry = std::time::Instant::now();
                    }
                    if !status.load(Ordering::Relaxed) {
                        crate::desktop::reveal();
                        state.context.request_repaint();
                    }
                    while let Ok(command) = rx.try_recv() {
                        let mut update = NOTIFYICONDATAW {
                            cbSize: data.cbSize,
                            hWnd: hwnd,
                            uID: 1,
                            ..Default::default()
                        };
                        match command {
                            Command::Tip(tip) => {
                                wide(&mut data.szTip, &tip);
                                update.szTip = data.szTip;
                                update.uFlags = NIF_TIP;
                            }
                            Command::Notify(title, message) => {
                                update.uFlags = NIF_INFO | NIF_REALTIME;
                                update.dwInfoFlags =
                                    NIIF_INFO | NIIF_NOSOUND | NIIF_RESPECT_QUIET_TIME;
                                wide(&mut update.szInfoTitle, &title);
                                wide(&mut update.szInfo, &message);
                            }
                        }
                        let _ = Shell_NotifyIconW(NIM_MODIFY, &update);
                    }
                    thread::sleep(Duration::from_millis(30));
                }
                let _ = Shell_NotifyIconW(NIM_DELETE, &data);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                let _ = DestroyWindow(hwnd);
                status.store(false, Ordering::Relaxed);
                Ok(())
            })();
            if result.is_err() {
                let _ = ready.send(false);
                status.store(false, Ordering::Relaxed);
            }
        })
        .map_err(|e| e.to_string())?;
    // No hide is allowed until Windows has acknowledged the icon.
    if !ready_rx
        .recv_timeout(Duration::from_secs(3))
        .unwrap_or(false)
    {
        return Err("Icône près de l'horloge indisponible ; la fenêtre reste accessible".into());
    }
    Ok((
        Tray {
            tx,
            available,
            thread: Arc::new(Mutex::new(Some(worker))),
            controls,
        },
        event_rx,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn notification_text_is_terminated_without_splitting_unicode() {
        let mut value = [0u16; 5];
        wide(&mut value, "ab🌍cd");
        assert_eq!(String::from_utf16(&value[..4]).unwrap(), "ab🌍");
        assert_eq!(value[4], 0);
        wide(&mut value, "x");
        assert_eq!(value, [120, 0, 0, 0, 0]);
    }
}
