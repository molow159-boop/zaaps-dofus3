//! Optional, bidirectional attachment of two ordinary desktop windows.
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, RwLock,
    },
    thread,
    time::{Duration, Instant},
};
use windows::{
    core::{BOOL, PWSTR},
    Win32::{
        Foundation::{CloseHandle, HWND, LPARAM, RECT},
        Graphics::Gdi::{
            GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
        },
        System::Threading::{
            OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
            PROCESS_QUERY_LIMITED_INFORMATION,
        },
        UI::WindowsAndMessaging::{
            EnumWindows, GetGUIThreadInfo, GetWindow, GetWindowLongPtrW, GetWindowRect,
            GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible,
            IsZoomed, SetWindowPos, GUITHREADINFO, GWL_EXSTYLE, GW_OWNER, SWP_ASYNCWINDOWPOS,
            SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
        },
    },
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Side {
    Top,
    #[default]
    Bottom,
    Left,
    Right,
}

impl Side {
    pub const ALL: [Self; 4] = [Self::Top, Self::Bottom, Self::Left, Self::Right];
    pub fn key(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Bottom => "bottom",
            Self::Left => "left",
            Self::Right => "right",
        }
    }
    pub fn parse(value: &str) -> Self {
        match value {
            "top" => Self::Top,
            "left" => Self::Left,
            "right" => Self::Right,
            _ => Self::Bottom,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Top => "En haut",
            Self::Bottom => "En bas",
            Self::Left => "À gauche",
            Self::Right => "À droite",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    pub enabled: bool,
    pub side: Side,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Bounds {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}
impl Bounds {
    fn shift(self, dx: i32, dy: i32) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
            ..self
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Pair {
    app: Bounds,
    guide: Bounds,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Leader {
    App,
    Guide,
}

fn align(mut pair: Pair, side: Side, leader: Leader, work: Bounds) -> Option<Pair> {
    let a = pair.app;
    let g = pair.guide;
    match leader {
        Leader::Guide => {
            let (x, y) = match side {
                Side::Top => (g.x, g.y - a.h),
                Side::Bottom => (g.x, g.y + g.h),
                Side::Left => (g.x - a.w, g.y),
                Side::Right => (g.x + g.w, g.y),
            };
            pair.app = Bounds { x, y, ..a };
        }
        Leader::App => {
            let (x, y) = match side {
                Side::Top => (a.x, a.y + a.h),
                Side::Bottom => (a.x, a.y - g.h),
                Side::Left => (a.x + a.w, a.y),
                Side::Right => (a.x - g.w, a.y),
            };
            pair.guide = Bounds { x, y, ..g };
        }
    }
    let left = pair.app.x.min(pair.guide.x);
    let top = pair.app.y.min(pair.guide.y);
    let right = (pair.app.x + a.w).max(pair.guide.x + g.w);
    let bottom = (pair.app.y + a.h).max(pair.guide.y + g.h);
    if right - left > work.w || bottom - top > work.h {
        return None;
    }
    let dx = if left < work.x {
        work.x - left
    } else {
        (work.x + work.w - right).min(0)
    };
    let dy = if top < work.y {
        work.y - top
    } else {
        (work.y + work.h - bottom).min(0)
    };
    Some(Pair {
        app: pair.app.shift(dx, dy),
        guide: pair.guide.shift(dx, dy),
    })
}

#[derive(Default)]
struct Tracking {
    previous: Option<Pair>,
    pending: Option<(Pair, Pair, Instant)>,
}
impl Tracking {
    fn user_takes_over(&mut self, active: Option<Leader>) {
        if let Some((before, target, _)) = self.pending {
            let follower_grabbed = match active {
                Some(Leader::App) => before.app != target.app,
                Some(Leader::Guide) => before.guide != target.guide,
                None => false,
            };
            if follower_grabbed {
                self.pending = None;
                self.previous = Some(target);
            }
        }
    }
    // Only wait for windows we actually moved. A user may keep dragging the leader.
    fn observe(&mut self, now: Pair, time: Instant) -> Result<Option<Leader>, ()> {
        if let Some((before, target, since)) = self.pending {
            let app_done = before.app == target.app || now.app == target.app;
            let guide_done = before.guide == target.guide || now.guide == target.guide;
            if !app_done || !guide_done {
                if time.duration_since(since) < Duration::from_millis(350) {
                    return Ok(None);
                }
                self.pending = None;
                self.previous = Some(now);
                return Err(());
            }
            self.pending = None;
            self.previous = Some(target);
        }
        Ok(match self.previous {
            None => Some(Leader::Guide),
            Some(old) if old.guide != now.guide => Some(Leader::Guide),
            Some(old) if old.app != now.app => Some(Leader::App),
            _ => None,
        })
    }
    fn submitted(&mut self, before: Pair, target: Pair) {
        self.previous = Some(before);
        self.pending = (target != before).then(|| (before, target, Instant::now()));
    }
}

fn rect(window: HWND) -> Option<Bounds> {
    let mut value = RECT::default();
    unsafe {
        GetWindowRect(window, &mut value).ok()?;
    }
    let value = Bounds {
        x: value.left,
        y: value.top,
        w: value.right - value.left,
        h: value.bottom - value.top,
    };
    (value.w > 0 && value.h > 0).then_some(value)
}

fn normal(window: HWND) -> bool {
    unsafe {
        IsWindow(Some(window)).as_bool()
            && IsWindowVisible(window).as_bool()
            && !IsIconic(window).as_bool()
            && !IsZoomed(window).as_bool()
    }
}

fn branded(value: &str) -> bool {
    crate::data::normalize(value).contains("ganymede")
}

fn guide_process(window: HWND) -> bool {
    let mut title = [0u16; 512];
    let count = unsafe { GetWindowTextW(window, &mut title) };
    if count <= 0 || !branded(&String::from_utf16_lossy(&title[..count as usize])) {
        return false;
    }
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut pid));
    }
    if pid == std::process::id() {
        return false;
    }
    unsafe {
        let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return false;
        };
        let mut path = [0u16; 2048];
        let mut length = path.len() as u32;
        let read = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_FORMAT(0),
            PWSTR(path.as_mut_ptr()),
            &mut length,
        );
        let _ = CloseHandle(process);
        if read.is_err() {
            return false;
        }
        let path = String::from_utf16_lossy(&path[..length as usize]);
        branded(path.rsplit(['\\', '/']).next().unwrap_or_default())
    }
}

fn primary_candidate(visible: bool, owned: bool, has_area: bool, exstyle: u32) -> bool {
    // Ganymede's notification icon is itself a visible, titled 16x16 tool window.
    // Size alone cannot distinguish a real application window from a helper.
    visible && !owned && has_area && exstyle & (WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0) == 0
}

fn primary_window(window: HWND) -> bool {
    unsafe {
        primary_candidate(
            IsWindowVisible(window).as_bool(),
            GetWindow(window, GW_OWNER).is_ok_and(|owner| !owner.is_invalid()),
            rect(window).is_some(),
            GetWindowLongPtrW(window, GWL_EXSTYLE) as u32,
        )
    }
}

unsafe extern "system" fn collect(window: HWND, context: LPARAM) -> BOOL {
    if primary_window(window) && guide_process(window) {
        (*(context.0 as *mut Vec<HWND>)).push(window);
    }
    BOOL(1)
}

fn find_guide() -> Result<HWND, &'static str> {
    let mut found = Vec::<HWND>::new();
    unsafe {
        let _ = EnumWindows(Some(collect), LPARAM(&mut found as *mut _ as isize));
    }
    match found.as_slice() {
        [only] => Ok(*only),
        [] => Err("En attente de Ganymède"),
        _ => Err("Plusieurs fenêtres Ganymède : accrochage en pause"),
    }
}

fn work_area(window: HWND) -> Option<Bounds> {
    unsafe {
        let monitor = MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return None;
        }
        let r = info.rcWork;
        Some(Bounds {
            x: r.left,
            y: r.top,
            w: r.right - r.left,
            h: r.bottom - r.top,
        })
    }
}

fn position(window: HWND, bounds: Bounds) -> bool {
    unsafe {
        SetWindowPos(
            window,
            None,
            bounds.x,
            bounds.y,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS,
        )
        .is_ok()
    }
}

pub fn start(
    initial: Config,
    stop: Arc<AtomicBool>,
    ctx: eframe::egui::Context,
) -> (mpsc::Sender<Config>, Arc<RwLock<String>>) {
    let (tx, rx) = mpsc::channel();
    let status = Arc::new(RwLock::new(String::new()));
    let output = status.clone();
    thread::Builder::new().name("window-docking".into()).spawn(move || {
        let mut config = initial;
        let mut tracking = Tracking::default();
        let mut guide = HWND::default();
        let mut app = HWND::default();
        let mut last_search = Instant::now() - Duration::from_secs(2);
        let mut message = String::new();
        let mut failed = false;
        let mut settle_until = Instant::now() + Duration::from_millis(150);
        while !stop.load(Ordering::Relaxed) {
            match rx.recv_timeout(if config.enabled { Duration::from_millis(32) } else { Duration::from_millis(200) }) {
                Ok(value) => {
                    config = value;
                    while let Ok(value) = rx.try_recv() { config = value; }
                    tracking = Tracking::default(); failed = false;
                    settle_until = Instant::now() + Duration::from_millis(150);
                    last_search = Instant::now() - Duration::from_secs(2);
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            let next = if !config.enabled { "" } else if Instant::now() < settle_until {
                "Accrochage en cours…"
            } else if failed {
                "Déplacement indisponible : désactivez puis réactivez l'accrochage"
            } else {
                if last_search.elapsed() >= Duration::from_secs(1) {
                    let found = find_guide();
                    let next = found.unwrap_or_default();
                    if guide != next { tracking = Tracking::default(); }
                    guide = next;
                    app = HWND(crate::window_corners::find_window() as *mut _);
                    last_search = Instant::now();
                    if let Err(error) = found { message = error.into(); }
                }
                if guide.is_invalid() || app.is_invalid() {
                    tracking = Tracking::default();
                    message.as_str()
                } else if !normal(app) || !normal(guide) {
                    tracking = Tracking::default();
                    "En pause : fenêtres réduites, masquées ou agrandies"
                } else {
                    match (rect(app), rect(guide)) {
                        (Some(app_rect), Some(guide_rect)) => {
                            let pair = Pair { app: app_rect, guide: guide_rect };
                            let mut info = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..Default::default() };
                            let active = unsafe {
                                if GetGUIThreadInfo(0, &mut info).is_err() { None }
                                else if info.hwndMoveSize == app { Some(Leader::App) }
                                else if info.hwndMoveSize == guide { Some(Leader::Guide) }
                                else { None }
                            };
                            tracking.user_takes_over(active);
                            match tracking.observe(pair, Instant::now()) {
                                Err(()) => { failed = true; "Déplacement non confirmé : réactivez l'accrochage" }
                                Ok(None) => "Fenêtres accrochées",
                                Ok(Some(leader)) => {
                                    let leader = active.unwrap_or(leader);
                                    let work = work_area(if leader == Leader::App { app } else { guide });
                                    if let Some(target) = work.and_then(|area| align(pair, config.side, leader, area)) {
                                        // Revalidate the foreign process immediately before any movement.
                                        if !primary_window(guide) || !guide_process(guide) { tracking = Tracking::default(); "Ganymède indisponible" }
                                        else if (target.app == pair.app || position(app, target.app))
                                            && (target.guide == pair.guide || position(guide, target.guide)) {
                                            tracking.submitted(pair, target);
                                            "Fenêtres accrochées"
                                        } else { failed = true; "Déplacement refusé : réactivez l'accrochage" }
                                    } else {
                                        tracking = Tracking::default();
                                        "Place insuffisante : choisissez un autre côté ou réduisez les fenêtres"
                                    }
                                }
                            }
                        }
                        _ => { tracking = Tracking::default(); "Lecture des fenêtres indisponible" }
                    }
                }
            };
            if let Ok(mut value) = output.write() {
                if *value != next { *value = next.into(); ctx.request_repaint(); }
            }
        }
    }).expect("impossible de démarrer l'accrochage des fenêtres");
    (tx, status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ganymede_notification_window_is_not_a_second_main_window() {
        let real = primary_candidate(true, false, true, 0x40118);
        let notification = primary_candidate(true, false, true, 0x80800a0);
        assert!(real);
        assert!(!notification);
        assert_eq!(
            [real, notification]
                .into_iter()
                .filter(|value| *value)
                .count(),
            1
        );
    }

    #[test]
    fn helpers_are_ignored_but_two_real_windows_remain_ambiguous() {
        assert!(!primary_candidate(true, true, true, 0));
        assert!(!primary_candidate(false, false, true, 0));
        assert!(!primary_candidate(true, false, false, 0));
        assert!(!primary_candidate(true, false, true, WS_EX_TOOLWINDOW.0));
        assert!(!primary_candidate(true, false, true, WS_EX_NOACTIVATE.0));
        assert_eq!(
            [0x40118, 0x40118, 0x80800a0]
                .into_iter()
                .filter(|flags| primary_candidate(true, false, true, *flags))
                .count(),
            2
        );
    }

    #[test]
    #[ignore = "read-only inspection of a running Ganymede window"]
    fn native_running_guide_has_one_primary_window() {
        let mut found = Vec::<HWND>::new();
        unsafe {
            EnumWindows(Some(collect), LPARAM(&mut found as *mut _ as isize)).unwrap();
        }
        println!("Candidate count: {}", found.len());
        for window in &found {
            let bounds = rect(*window);
            unsafe {
                println!(
                    "Candidate {:?}: bounds={bounds:?}, owner={:?}, exstyle={:#x}",
                    window,
                    GetWindow(*window, GW_OWNER).ok(),
                    GetWindowLongPtrW(*window, GWL_EXSTYLE)
                );
            }
        }
        assert_eq!(found.len(), 1, "one main Ganymede window must be selected");
    }
    fn pair() -> Pair {
        Pair {
            app: Bounds {
                x: 100,
                y: 100,
                w: 420,
                h: 300,
            },
            guide: Bounds {
                x: 700,
                y: 400,
                w: 360,
                h: 500,
            },
        }
    }
    fn work() -> Bounds {
        Bounds {
            x: -2000,
            y: -1500,
            w: 5000,
            h: 4000,
        }
    }
    fn touching(p: Pair, side: Side) -> bool {
        match side {
            Side::Top => p.app.x == p.guide.x && p.app.y + p.app.h == p.guide.y,
            Side::Bottom => p.app.x == p.guide.x && p.app.y == p.guide.y + p.guide.h,
            Side::Left => p.app.y == p.guide.y && p.app.x + p.app.w == p.guide.x,
            Side::Right => p.app.y == p.guide.y && p.app.x == p.guide.x + p.guide.w,
        }
    }
    #[test]
    fn every_side_follows_either_window_without_resizing() {
        for side in Side::ALL {
            for leader in [Leader::App, Leader::Guide] {
                let before = pair();
                let after = align(before, side, leader, work()).unwrap();
                assert!(touching(after, side));
                assert_eq!((after.app.w, after.app.h), (before.app.w, before.app.h));
                assert_eq!(
                    (after.guide.w, after.guide.h),
                    (before.guide.w, before.guide.h)
                );
                assert_eq!(align(after, side, leader, work()), Some(after));
            }
        }
    }
    #[test]
    fn resizing_either_window_keeps_the_selected_edge() {
        for side in Side::ALL {
            for leader in [Leader::App, Leader::Guide] {
                let mut p = align(pair(), side, leader, work()).unwrap();
                match leader {
                    Leader::App => {
                        p.app.w += 87;
                        p.app.h += 125;
                    }
                    Leader::Guide => {
                        p.guide.w += 150;
                        p.guide.h -= 70;
                    }
                }
                assert!(touching(align(p, side, leader, work()).unwrap(), side));
            }
        }
    }
    #[test]
    fn group_stays_in_negative_coordinate_monitor_work_area() {
        let work = Bounds {
            x: -1920,
            y: -100,
            w: 1920,
            h: 1040,
        };
        for side in Side::ALL {
            for leader in [Leader::App, Leader::Guide] {
                let p = align(pair(), side, leader, work).unwrap();
                assert!(touching(p, side));
                for r in [p.app, p.guide] {
                    assert!(r.x >= work.x && r.y >= work.y && r.x + r.w <= 0 && r.y + r.h <= 940);
                }
            }
        }
    }
    #[test]
    fn insufficient_space_never_forces_a_window_resize() {
        assert!(align(
            pair(),
            Side::Bottom,
            Leader::Guide,
            Bounds {
                x: 0,
                y: 0,
                w: 1920,
                h: 700
            }
        )
        .is_none());
        assert!(align(
            pair(),
            Side::Right,
            Leader::App,
            Bounds {
                x: 0,
                y: 0,
                w: 700,
                h: 1080
            }
        )
        .is_none());
    }
    #[test]
    fn programmatic_movement_does_not_reverse_the_leader() {
        let mut t = Tracking::default();
        let before = pair();
        assert_eq!(t.observe(before, Instant::now()), Ok(Some(Leader::Guide)));
        let target = align(before, Side::Bottom, Leader::Guide, work()).unwrap();
        t.submitted(before, target);
        assert_eq!(t.observe(before, Instant::now()), Ok(None));
        assert_eq!(t.observe(target, Instant::now()), Ok(None));
        let dragged = Pair {
            app: target.app.shift(40, 70),
            ..target
        };
        assert_eq!(t.observe(dragged, Instant::now()), Ok(Some(Leader::App)));
    }
    #[test]
    fn continued_drag_is_not_lost_when_the_follower_acknowledges() {
        let mut t = Tracking::default();
        let before = pair();
        let target = align(before, Side::Right, Leader::Guide, work()).unwrap();
        t.submitted(before, target);
        let now = Pair {
            guide: target.guide.shift(20, 10),
            ..target
        };
        assert_eq!(t.observe(now, Instant::now()), Ok(Some(Leader::Guide)));
    }
    #[test]
    fn blocked_movement_times_out_without_an_endless_feedback_loop() {
        let mut t = Tracking::default();
        let before = pair();
        t.submitted(
            before,
            align(before, Side::Bottom, Leader::Guide, work()).unwrap(),
        );
        assert_eq!(
            t.observe(before, Instant::now() + Duration::from_secs(1)),
            Err(())
        );
        assert!(t.pending.is_none());
    }

    #[test]
    fn grabbing_the_follower_during_a_pending_move_changes_direction() {
        let before = pair();
        let target = align(before, Side::Bottom, Leader::Guide, work()).unwrap();
        let mut t = Tracking::default();
        t.submitted(before, target);
        t.user_takes_over(Some(Leader::App));
        let dragged = Pair {
            app: target.app.shift(10, 30),
            ..target
        };
        assert_eq!(t.observe(dragged, Instant::now()), Ok(Some(Leader::App)));
        assert!(t.pending.is_none());
    }

    #[test]
    fn pausing_discards_old_positions_before_reattaching() {
        let before = pair();
        let target = align(before, Side::Left, Leader::Guide, work()).unwrap();
        let mut t = Tracking::default();
        t.submitted(before, target);
        assert!(t.pending.is_some());
        t = Tracking::default();
        let restored = Pair {
            app: before.app.shift(300, 10),
            guide: before.guide.shift(-100, 0),
        };
        assert_eq!(t.observe(restored, Instant::now()), Ok(Some(Leader::Guide)));
        assert!(touching(
            align(restored, Side::Left, Leader::Guide, work()).unwrap(),
            Side::Left
        ));
    }

    #[test]
    #[ignore = "native Windows test using two hidden, owned fixture windows"]
    fn native_hidden_windows_move_in_both_directions_without_activation() {
        use windows::{
            core::w,
            Win32::UI::WindowsAndMessaging::{
                CreateWindowExW, DestroyWindow, GetForegroundWindow, WINDOW_EX_STYLE, WS_POPUP,
            },
        };
        struct Owned(HWND);
        impl Drop for Owned {
            fn drop(&mut self) {
                unsafe {
                    let _ = DestroyWindow(self.0);
                }
            }
        }
        unsafe {
            let a = Owned(
                CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    w!("STATIC"),
                    w!("Dock fixture A"),
                    WS_POPUP,
                    100,
                    100,
                    420,
                    300,
                    None,
                    None,
                    None,
                    None,
                )
                .unwrap(),
            );
            let g = Owned(
                CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    w!("STATIC"),
                    w!("Dock fixture B"),
                    WS_POPUP,
                    700,
                    400,
                    360,
                    500,
                    None,
                    None,
                    None,
                    None,
                )
                .unwrap(),
            );
            let foreground = GetForegroundWindow();
            for side in Side::ALL {
                for leader in [Leader::App, Leader::Guide] {
                    let before = Pair {
                        app: rect(a.0).unwrap(),
                        guide: rect(g.0).unwrap(),
                    };
                    let target = align(before, side, leader, work()).unwrap();
                    assert!(position(a.0, target.app));
                    assert!(position(g.0, target.guide));
                    assert_eq!(rect(a.0), Some(target.app));
                    assert_eq!(rect(g.0), Some(target.guide));
                    assert!(!IsWindowVisible(a.0).as_bool() && !IsWindowVisible(g.0).as_bool());
                    assert!(!normal(a.0) && !normal(g.0));
                }
            }
            assert_eq!(GetForegroundWindow(), foreground);
            assert!(!guide_process(a.0) && !guide_process(g.0));
        }
    }
}
