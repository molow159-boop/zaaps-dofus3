use crate::commands::{Action, Step};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, RwLock,
    },
    thread,
    time::Duration,
};
use windows::Win32::UI::{
    Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS},
    WindowsAndMessaging::{PeekMessageW, MSG, PM_REMOVE, WM_HOTKEY},
};

pub use crate::settings::{Feedback, Shortcuts};

pub fn parse_shortcut(text: &str) -> Option<(u32, u32)> {
    let parts: Vec<_> = text.split('+').map(str::trim).collect();
    if !(2..=4).contains(&parts.len()) {
        return None;
    }
    let mut flags = 0u32;
    for part in &parts[..parts.len() - 1] {
        let bit = match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => 2,
            "alt" => 1,
            "shift" => 4,
            _ => return None,
        };
        if (flags & bit) != 0 {
            return None;
        }
        flags |= bit;
    }
    let name = parts.last()?.to_ascii_uppercase();
    let vk = if name.len() == 1 && name.as_bytes()[0].is_ascii_alphanumeric() {
        name.as_bytes()[0] as u32
    } else {
        let number = name.strip_prefix('F').and_then(|s| s.parse::<u32>().ok())?;
        if !(1..=12).contains(&number) {
            return None;
        }
        0x70 + number - 1
    };
    Some((flags | 0x4000, vk))
}

#[derive(Debug)]
pub enum Notice {
    Copied(String),
    Info(String),
    Error(String),
    ClipboardReplaced,
    AutomaticChanged(bool),
    WindowVisibility(bool),
}

fn copy_clipboard(text: &str) -> Result<(), String> {
    for _ in 0..5 {
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            if clipboard.set_text(text.to_owned()).is_ok() {
                return Ok(());
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    Err("Presse-papiers indisponible".into())
}

pub fn copy(text: &str) -> Notice {
    match copy_clipboard(text) {
        Ok(()) => Notice::Copied(text.to_owned()),
        Err(message) => Notice::Error(message),
    }
}

pub enum Control {
    Pause,
    Configure(Shortcuts),
    ToggleAutomatic,
    Copy(Action),
    CopyText(String),
    CopyDiagnostic(String),
}

#[derive(Default)]
struct ClipboardWatch {
    owned: Option<(String, u32)>,
}

impl ClipboardWatch {
    fn copied(&mut self, command: String, sequence: u32) {
        self.owned = Some((command, sequence));
    }
    fn changed(&self, sequence: u32) -> bool {
        sequence != 0
            && self
                .owned
                .as_ref()
                .is_some_and(|(_, previous)| *previous != sequence)
    }
    fn observe(&mut self, sequence: u32, content: Result<Option<String>, ()>) -> bool {
        if !self.changed(sequence) {
            return false;
        }
        let Ok(content) = content else { return false };
        if let Some((command, previous)) = &mut self.owned {
            if content.as_ref() == Some(command) {
                *previous = sequence;
                return false;
            }
        }
        self.owned = None;
        true
    }
}

fn report_copy(
    command: &str,
    config: &Shortcuts,
    watch: &mut ClipboardWatch,
    notifications: &mpsc::Sender<Notice>,
    tray: Option<&crate::tray::Tray>,
) {
    let notice = copy(command);
    if matches!(notice, Notice::Copied(_)) {
        let sequence =
            unsafe { windows::Win32::System::DataExchange::GetClipboardSequenceNumber() };
        watch.copied(command.to_owned(), sequence);
        if let Some(tray) = tray {
            tray.tip(format!("Zaaps Dofus 3 — copié : {command}"));
            if config.feedback == Feedback::Visual {
                tray.notify("Commande copiée", command);
            }
        }
        if config.feedback == Feedback::Sound {
            let _ = unsafe {
                windows::Win32::System::Diagnostics::Debug::MessageBeep(
                    windows::Win32::UI::WindowsAndMessaging::MB_OK,
                )
            };
        }
    }
    let _ = notifications.send(notice);
}

fn set_automatic(
    config: &mut Shortcuts,
    automatic: &mut AutoCopy,
    enabled: bool,
    notifications: &mpsc::Sender<Notice>,
) {
    let mut next = config.clone();
    next.automatic = enabled;
    match next.save() {
        Ok(()) => {
            *config = next;
            automatic.set_enabled(enabled);
            let _ = notifications.send(Notice::AutomaticChanged(enabled));
        }
        Err(error) => {
            let _ = notifications.send(Notice::Error(format!("Mode Auto non modifié : {error}")));
        }
    }
}

// Un changement doit être confirmé par deux lectures différentes de Ganymède.
// L'état de référence empêche toute copie à l'activation ou après une reconnexion.
struct AutoCopy {
    enabled: bool,
    baseline: Option<String>,
    pending: Option<(String, u8)>,
    last_update: Option<std::time::Instant>,
}

impl AutoCopy {
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            baseline: None,
            pending: None,
            last_update: None,
        }
    }

    fn set_enabled(&mut self, enabled: bool) {
        if self.enabled != enabled {
            *self = Self::new(enabled);
        }
    }

    fn observe(&mut self, step: &Step) -> Option<String> {
        if !self.enabled || step.updated.is_none() || step.updated == self.last_update {
            return None;
        }
        self.last_update = step.updated;
        let Some(identity) = step.identity.clone() else {
            self.pending = None;

            return None;
        };
        if self.baseline.is_none() {
            self.baseline = Some(identity);
            return None;
        }
        if self.baseline.as_ref() == Some(&identity) {
            self.pending = None;
            return None;
        }
        let Ok(command) = step.command(Action::Combined) else {
            self.pending = None;
            return None;
        };
        let candidate_key = format!("{identity}|{command}");
        match &mut self.pending {
            Some((candidate, count)) if candidate == &candidate_key => {
                *count += 1;
                if *count < 2 {
                    return None;
                }
            }
            _ => {
                self.pending = Some((candidate_key, 1));
                return None;
            }
        }
        self.pending = None;
        self.baseline = Some(identity);
        Some(command)
    }
}

pub fn start(
    cache: Arc<RwLock<Step>>,
    shortcuts: Shortcuts,
    notifications: mpsc::Sender<Notice>,
    stop: Arc<AtomicBool>,
    context: eframe::egui::Context,
    tray: Option<crate::tray::Tray>,
) -> mpsc::Sender<Control> {
    let (settings_tx, settings_rx) = mpsc::channel();
    thread::Builder::new().name("global-shortcuts".into()).spawn(move || {
        let mut config = shortcuts;
        let mut automatic = AutoCopy::new(config.automatic);
        let mut watch = ClipboardWatch::default();
        let mut active = [false; 5];
        let register = |keys: &Shortcuts, slots: &mut [bool; 5]| {
            for (index, shortcut) in keys.keys().iter().enumerate() {
                if let Some((flags, vk)) = parse_shortcut(shortcut) {
                    slots[index] = unsafe { RegisterHotKey(None, 0x52A1 + index as i32,
                        HOT_KEY_MODIFIERS(flags), vk).is_ok() };
                }
            }
        };
        let unregister = |slots: &mut [bool; 5]| {
            for (index, registered) in slots.iter_mut().enumerate() {
                if *registered { let _ = unsafe { UnregisterHotKey(None, 0x52A1 + index as i32) }; }
                *registered = false;
            }
        };
        register(&config, &mut active);
        if !active.iter().all(|item| *item) {
            let failed: Vec<_> = config.keys().into_iter().zip(active).filter_map(|(key, ok)| (!ok).then_some(key)).collect();
            let _ = notifications.send(Notice::Error(format!("Raccourcis indisponibles : {}", failed.join(", "))));
            context.request_repaint();
        }
        let mut msg = MSG::default();
        let mut paused = false;
        let mut last_check = std::time::Instant::now();
        while !stop.load(Ordering::Relaxed) {
            while let Ok(next) = settings_rx.try_recv() {
                match next {
                    Control::ToggleAutomatic => {
                        let enabled = !config.automatic;
                        set_automatic(&mut config, &mut automatic, enabled, &notifications);
                    }
                    Control::Pause => { unregister(&mut active); paused = true; }
                    Control::Configure(value) => {
                        unregister(&mut active);
                        automatic.set_enabled(value.automatic);
                        config = value;
                        paused = false;
                        register(&config, &mut active);
                        if !active.iter().all(|item| *item) {
                            let failed: Vec<_> = config.keys().into_iter().zip(active).filter_map(|(key, ok)| (!ok).then_some(key)).collect();
                            let _ = notifications.send(Notice::Error(format!("Raccourcis indisponibles : {}", failed.join(", "))));
                        }
                    }
                    Control::Copy(action) => {
                        let command = cache.read().map_err(|_| "Données indisponibles".to_owned())
                            .and_then(|step| step.command(action));
                        match command {
                            Ok(command) => report_copy(&command, &config, &mut watch, &notifications, tray.as_ref()),
                            Err(error) => { let _ = notifications.send(Notice::Error(error)); }
                        }
                    }
                    Control::CopyText(command) => report_copy(&command, &config, &mut watch, &notifications, tray.as_ref()),
                    Control::CopyDiagnostic(text) => {
                        if copy_clipboard(&text).is_ok() {
                            watch.owned = None;
                            let _ = notifications.send(Notice::Info("Diagnostic copié — prêt à partager".into()));
                        } else { let _ = notifications.send(Notice::Error("Presse-papiers indisponible".into())); }
                    }
                }
                context.request_repaint();
            }
            if !paused {
                if let Ok(state) = cache.read() {
                    if let Some(command) = automatic.observe(&state) {
                        report_copy(&command, &config, &mut watch, &notifications, tray.as_ref());
                        context.request_repaint();
                    }
                }
            }
            while unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() } {
                if msg.message != WM_HOTKEY || paused { continue; }
                match msg.wParam.0 as i32 {
                    0x52A4 => {
                        let enabled = !config.automatic;
                        set_automatic(&mut config, &mut automatic, enabled, &notifications);
                    }
                    0x52A5 => {
                        let visible = crate::desktop::toggle_window(tray.as_ref().is_some_and(crate::tray::Tray::available));
                        let _ = notifications.send(Notice::WindowVisibility(visible));
                    }
                    id @ 0x52A1..=0x52A3 => {
                        let action = match id { 0x52A1 => Action::Zaap, 0x52A2 => Action::Travel, _ => Action::Combined };
                        let command = cache.read().map_err(|_| "Données indisponibles".to_owned())
                            .and_then(|state| state.command(action));
                        match command {
                            Ok(command) => report_copy(&command, &config, &mut watch, &notifications, tray.as_ref()),
                            Err(error) => { let _ = notifications.send(Notice::Error(error)); }
                        }
                    }
                    _ => {},
                }
                context.request_repaint();
            }
            if config.watch_clipboard && last_check.elapsed() >= Duration::from_millis(150) {
                last_check = std::time::Instant::now();
                let sequence = unsafe { windows::Win32::System::DataExchange::GetClipboardSequenceNumber() };
                if watch.changed(sequence) {
                    let content = arboard::Clipboard::new().and_then(|mut clipboard| clipboard.get_text());
                    let content = match content { Ok(text) => Ok(Some(text)),
                        Err(arboard::Error::ContentNotAvailable) => Ok(None), Err(_) => Err(()) };
                    if watch.observe(sequence, content) {
                        let _ = notifications.send(Notice::ClipboardReplaced);
                        if config.feedback == Feedback::Sound {
                            let _ = unsafe { windows::Win32::System::Diagnostics::Debug::MessageBeep(
                                windows::Win32::UI::WindowsAndMessaging::MB_ICONEXCLAMATION) };
                        }
                        if let Some(tray) = &tray {
                            tray.tip("Zaaps Dofus 3 — commande remplacée dans le presse-papiers");
                            if config.feedback == Feedback::Visual {
                                tray.notify("Commande remplacée", "Une autre copie a remplacé la commande. Ouvrez Zaaps pour recopier le trajet courant.");
                            }
                        }
                        context.request_repaint();
                    }
                }
            }
            thread::sleep(Duration::from_millis(12));
        }
        unregister(&mut active);
    }).expect("impossible de démarrer les raccourcis Windows");
    settings_tx
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "utilise le presse-papiers et les raccourcis Windows réels"]
    fn native_background_copy_and_clipboard_replacement() {
        let directory = crate::data::embedded().unwrap();
        let cache = Arc::new(RwLock::new(Step::from_text(
            "630/774\nEnsuite allez en Srambad en [8,3]",
            &directory,
        )));
        let stop = Arc::new(AtomicBool::new(false));
        struct Cleanup(Arc<AtomicBool>);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Relaxed);
                thread::sleep(Duration::from_millis(50));
            }
        }
        let _cleanup = Cleanup(stop.clone());
        let (tx, rx) = mpsc::channel();
        let config = Shortcuts {
            zaap: "Ctrl+Alt+Shift+F6".into(),
            travel: "Ctrl+Alt+Shift+F7".into(),
            combined: "Ctrl+Alt+Shift+F8".into(),
            toggle_auto: "Ctrl+Alt+Shift+F9".into(),
            toggle_window: "Ctrl+Alt+Shift+F10".into(),
            feedback: Feedback::Silent,
            ..Shortcuts::default()
        };
        let controls = start(
            cache.clone(),
            config,
            tx,
            stop,
            eframe::egui::Context::default(),
            None,
        );
        controls.send(Control::Copy(Action::Combined)).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        loop {
            assert!(std::time::Instant::now() < deadline);
            if let Notice::Copied(command) = rx.recv_timeout(Duration::from_secs(2)).unwrap() {
                assert_eq!(command, "/zaap -20,-20; /zaap -20,-20; /travel -22 -24");
                break;
            }
        }
        arboard::Clipboard::new()
            .unwrap()
            .set_text("Texte de remplacement du test Windows".to_owned())
            .unwrap();
        loop {
            assert!(std::time::Instant::now() < deadline);
            if matches!(
                rx.recv_timeout(Duration::from_secs(2)).unwrap(),
                Notice::ClipboardReplaced
            ) {
                break;
            }
        }
        // Recopy must resolve the current step rather than restoring the last command.
        *cache.write().unwrap() = Step::from_text(
            "631/774\nEnsuite allez en [7,20]\nZaap du Rivage sufokien",
            &directory,
        );
        controls.send(Control::Copy(Action::Combined)).unwrap();
        if let Notice::Copied(command) = rx.recv_timeout(Duration::from_secs(2)).unwrap() {
            assert_eq!(command, "/zaap 10,22; /zaap 10,22; /travel 7 20");
        } else {
            panic!("La copie du trajet courant a échoué");
        }
        *cache.write().unwrap() = Step::unavailable("Fenêtre fermée");
        controls.send(Control::Copy(Action::Combined)).unwrap();
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(2)).unwrap(),
            Notice::Error(_)
        ));
    }
    #[test]
    fn clipboard_changes_warn_once_and_never_restore_old_content() {
        let mut watch = ClipboardWatch::default();
        watch.copied("/travel 1 2".into(), 10);
        assert!(!watch.observe(10, Ok(Some("unrelated".into()))));
        assert!(!watch.observe(11, Err(()))); // clipboard temporarily busy
        assert!(!watch.observe(11, Ok(Some("/travel 1 2".into()))));
        assert!(watch.observe(12, Ok(Some("autre copie".into()))));
        assert!(!watch.observe(13, Ok(Some("encore une copie".into()))));
        assert!(watch.owned.is_none());
        watch.copied("/travel 3 4".into(), 14);
        assert!(watch.observe(15, Ok(None))); // image or files instead of text
    }
    #[test]
    fn validates_shortcuts() {
        assert_eq!(parse_shortcut("Ctrl+J"), Some((0x4002, 0x4A)));
        assert!(parse_shortcut("Ctrl+Shift+F12").is_some());
        assert!(parse_shortcut("Ctrl+Ctrl+J").is_none());
        assert!(parse_shortcut("J").is_none());
    }
    fn snapshot(progress: u32, travel: &str) -> Step {
        Step {
            zaap: Some("/zaap 5,-18".into()),
            travel: Some(travel.into()),
            updated: Some(std::time::Instant::now()),
            identity: Some(format!("step:{progress}/774")),
            ..Step::default()
        }
    }

    #[test]
    fn automatic_copies_only_after_confirmed_step_change() {
        let mut watcher = AutoCopy::new(true);
        let first = snapshot(459, "/travel 4 -19");
        assert_eq!(watcher.observe(&first), None);
        assert_eq!(watcher.observe(&first), None); // même lecture UIA, pas une deuxième confirmation
        let next = snapshot(460, "/travel 5 -19");
        assert_eq!(watcher.observe(&next), None);
        let confirmed = snapshot(460, "/travel 5 -19");
        assert_eq!(
            watcher.observe(&confirmed).as_deref(),
            Some("/zaap 5,-18; /zaap 5,-18; /travel 5 -19")
        );
        assert_eq!(watcher.observe(&snapshot(460, "/travel 5 -19")), None);
    }

    #[test]
    fn automatic_cannot_copy_partial_or_on_enable() {
        let mut watcher = AutoCopy::new(false);
        assert_eq!(watcher.observe(&snapshot(458, "/travel 2 -3")), None);
        watcher.set_enabled(true);
        assert_eq!(watcher.observe(&snapshot(458, "/travel 2 -3")), None);
        let mut invalid = snapshot(459, "/travel 4 -19");
        invalid.zaap = None;
        assert_eq!(watcher.observe(&invalid), None);
        assert_eq!(watcher.observe(&snapshot(459, "/travel 4 -19")), None);
        assert!(watcher.observe(&snapshot(459, "/travel 4 -19")).is_some());
        watcher.set_enabled(false);
        assert_eq!(watcher.observe(&snapshot(460, "/travel 3 -19")), None);
    }

    #[test]
    fn automatic_initial_incomplete_step_does_not_trigger_copy_when_ready() {
        let mut watcher = AutoCopy::new(true);
        let mut initial = snapshot(459, "/travel 4 -19");
        initial.travel = None;
        assert!(watcher.observe(&initial).is_none());
        assert!(watcher.observe(&snapshot(459, "/travel 4 -19")).is_none());
        assert!(watcher.observe(&snapshot(460, "/travel 5 -19")).is_none());
        assert!(watcher.observe(&snapshot(460, "/travel 5 -19")).is_some());
    }

    #[test]
    fn automatic_detects_step_with_same_commands() {
        let mut watcher = AutoCopy::new(true);
        watcher.observe(&snapshot(459, "/travel 4 -19"));
        assert!(watcher.observe(&snapshot(460, "/travel 4 -19")).is_none());
        assert!(watcher.observe(&snapshot(460, "/travel 4 -19")).is_some());
    }

    #[test]
    fn rejects_conflicts() {
        let mut settings = Shortcuts::default();
        settings.combined = settings.zaap.clone();
        assert!(settings.validate().is_err());
    }
}

#[cfg(test)]
mod regression_tests {
    use super::*;
    fn step(number: u32, destination: &str) -> Step {
        Step::from_text(
            &format!("{number}/100\n{destination}"),
            &crate::data::embedded().unwrap(),
        )
    }
    #[test]
    fn auto_confirms_the_route_not_only_the_counter() {
        let mut auto = AutoCopy::new(true);
        assert!(auto.observe(&step(1, "Allez en [1,2]")).is_none());
        assert!(auto
            .observe(&step(2, "Zaap de Bonta\nAllez en [1,2]"))
            .is_none());
        assert!(auto.observe(&step(2, "Allez en [3,4]")).is_none());
        assert_eq!(
            auto.observe(&step(2, "Allez en [3,4]")).as_deref(),
            Some("/travel 3 4")
        );
        assert!(auto.observe(&step(2, "Allez en [3,4]")).is_none());
        assert!(auto.observe(&step(3, "Rien")).is_none());
        assert!(auto.observe(&step(4, "Allez en [5,6]")).is_none());
        assert_eq!(
            auto.observe(&step(4, "Allez en [5,6]")).as_deref(),
            Some("/travel 5 6")
        );
    }
    #[test]
    fn interrupted_confirmation_requires_two_new_samples() {
        let mut auto = AutoCopy::new(true);
        auto.observe(&step(1, "Allez en [1,2]"));
        auto.observe(&step(2, "Allez en [3,4]"));
        auto.observe(&Step::unavailable("transition"));
        assert!(auto.observe(&step(2, "Allez en [3,4]")).is_none());
        assert!(auto.observe(&step(2, "Allez en [3,4]")).is_some());
    }
}

#[cfg(test)]
mod reader_cycle_tests {
    use super::*;
    use crate::{data, ganymede};

    #[test]
    fn dimensional_routes_are_confirmed_and_copied_once_per_new_step() {
        let known = data::embedded().unwrap();
        let cache = RwLock::new(Step::default());
        let mut previous = None;
        let mut auto = AutoCopy::new(true);
        for (text, expected) in [
            ("629/774\nEnsuite allez en Enutrosor en [1,2]", None),
            ("630/774\nEnsuite allez en Srambad en [8,3]", Some("/zaap -20,-20; /zaap -20,-20; /travel -22 -24")),
            ("631/774\nEnsuite allez en Xélorium en [1,2]", Some("/zaap -20,-20; /zaap -20,-20; /travel -22 -24")),
            ("632/774\nParlez au personnage dans Srambad.", None),
            ("643/774\nUne fois dans la Dimension Obscure, combattez un Panterreur.\nEnsuite allez en [7,20]\nZaap du Rivage sufokien", Some("/zaap 10,22; /zaap 10,22; /travel 7 20")),
        ] {
            let mut copies = Vec::new();
            for cycle in 0..8 {
                ganymede::refresh(&cache, &mut previous, &known, || Ok(text.into()));
                let step = cache.read().unwrap();
                if cycle == 0 {
                    assert!(step.command(Action::Combined).is_err());
                    assert!(step.zaap.is_none() && step.travel.is_none());
                }
                if let Some(command) = auto.observe(&step) {
                    copies.push(command);
                }
            }
            assert_eq!(copies, expected.into_iter().map(str::to_owned).collect::<Vec<_>>(), "{text}");
        }
    }

    #[test]
    fn shortcuts_remain_available_during_refresh_and_auto_follows_real_changes() {
        let known = data::embedded().unwrap();
        let cache = RwLock::new(Step::default());
        let mut previous = None;
        let mut auto = AutoCopy::new(true);
        let initial = "496/774\nEnsuite allez en [-32,40]\nZaap de Brâkmar";
        // First identity is the activation baseline, even before it stabilizes.
        ganymede::refresh(&cache, &mut previous, &known, || Ok(initial.into()));
        assert!(auto.observe(&cache.read().unwrap()).is_none());
        ganymede::refresh(&cache, &mut previous, &known, || Ok(initial.into()));
        assert!(auto.observe(&cache.read().unwrap()).is_none());
        for noise in 0..8 {
            ganymede::refresh(&cache, &mut previous, &known, || {
                let step = cache.read().unwrap();
                assert_eq!(step.command(Action::Zaap).unwrap(), "/zaap -26,37");
                assert_eq!(step.command(Action::Travel).unwrap(), "/travel -32 40");
                assert_eq!(
                    step.command(Action::Combined).unwrap(),
                    "/zaap -26,37; /zaap -26,37; /travel -32 40"
                );
                Ok(format!("Horloge {noise}\n{initial}"))
            });
            assert!(auto.observe(&cache.read().unwrap()).is_none());
        }
        let next = "497/774\nEnsuite allez en [24,-27]\nZaap du\nVillage de Pandala";
        let mut copies = Vec::new();
        for _ in 0..8 {
            ganymede::refresh(&cache, &mut previous, &known, || Ok(next.into()));
            if let Some(command) = auto.observe(&cache.read().unwrap()) {
                copies.push(command);
            }
        }
        assert_eq!(copies, ["/zaap 20,-29; /zaap 20,-29; /travel 24 -27"]);
        // A failed read invalidates, rather than keeping the previous route.
        ganymede::refresh(&cache, &mut previous, &known, || {
            Err("Fenêtre fermée".into())
        });
        assert!(cache.read().unwrap().command(Action::Combined).is_err());
    }
}
