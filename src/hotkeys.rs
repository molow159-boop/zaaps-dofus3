use crate::commands::{Action, Step};
use std::{
    fs,
    path::PathBuf,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shortcuts {
    pub zaap: String,
    pub travel: String,
    pub combined: String,
    pub automatic: bool,
}

impl Default for Shortcuts {
    fn default() -> Self {
        Self {
            zaap: "Ctrl+J".into(),
            travel: "Ctrl+K".into(),
            combined: "Ctrl+L".into(),
            automatic: false,
        }
    }
}

fn settings_path() -> PathBuf {
    let folder = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    folder.join("Zaaps Dofus 3").join("raccourcis.ini")
}

impl Shortcuts {
    pub fn load() -> Self {
        let mut settings = Self::default();
        let Ok(contents) = fs::read_to_string(settings_path()) else {
            return settings;
        };
        for line in contents.trim_start_matches('\u{feff}').lines() {
            let Some((field, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            if field.trim() == "automatic" {
                settings.automatic = value.eq_ignore_ascii_case("on")
                    || value == "1"
                    || value.eq_ignore_ascii_case("true");
                continue;
            }
            if parse_shortcut(value).is_none() {
                continue;
            }
            match field.trim() {
                "zaap" => settings.zaap = value.to_owned(),
                "travel" => settings.travel = value.to_owned(),
                "combined" => settings.combined = value.to_owned(),
                _ => {}
            }
        }
        if settings.zaap.eq_ignore_ascii_case(&settings.travel) {
            settings.travel = if settings.zaap.eq_ignore_ascii_case("Ctrl+K") {
                "Ctrl+J"
            } else {
                "Ctrl+K"
            }
            .into();
        }
        if settings.combined.eq_ignore_ascii_case(&settings.zaap)
            || settings.combined.eq_ignore_ascii_case(&settings.travel)
        {
            for candidate in ["Ctrl+L", "Ctrl+M", "Ctrl+N", "Ctrl+O", "Ctrl+P"] {
                if !candidate.eq_ignore_ascii_case(&settings.zaap)
                    && !candidate.eq_ignore_ascii_case(&settings.travel)
                {
                    settings.combined = candidate.into();
                    break;
                }
            }
        }
        settings
    }

    pub fn save(&self) -> Result<(), String> {
        self.validate()?;
        let path = settings_path();
        fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        fs::write(
            path,
            format!(
                "zaap={}\r\ntravel={}\r\ncombined={}\r\nautomatic={}\r\n",
                self.zaap,
                self.travel,
                self.combined,
                if self.automatic { "on" } else { "off" }
            ),
        )
        .map_err(|e| e.to_string())
    }

    pub fn validate(&self) -> Result<(), String> {
        let keys = [&self.zaap, &self.travel, &self.combined];
        if keys.iter().any(|s| parse_shortcut(s).is_none()) {
            return Err(
                "Raccourci invalide : utilise Ctrl, Alt ou Shift avec A–Z, 0–9 ou F1–F12".into(),
            );
        }
        if keys[0].eq_ignore_ascii_case(keys[1])
            || keys[0].eq_ignore_ascii_case(keys[2])
            || keys[1].eq_ignore_ascii_case(keys[2])
        {
            return Err("Choisis trois raccourcis différents".into());
        }
        Ok(())
    }
}

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
    Automatic(bool),
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
) -> mpsc::Sender<Control> {
    let (settings_tx, settings_rx) = mpsc::channel();
    thread::Builder::new()
        .name("global-shortcuts".into())
        .spawn(move || {
            let mut config = shortcuts;
            let mut automatic = AutoCopy::new(config.automatic);
            let mut active = [false; 3];
            let register = |keys: &Shortcuts, slots: &mut [bool; 3]| {
                for (index, shortcut) in [&keys.zaap, &keys.travel, &keys.combined]
                    .iter()
                    .enumerate()
                {
                    if let Some((flags, vk)) = parse_shortcut(shortcut) {
                        slots[index] = unsafe {
                            RegisterHotKey(
                                None,
                                0x52A1 + index as i32,
                                HOT_KEY_MODIFIERS(flags),
                                vk,
                            )
                            .is_ok()
                        };
                    }
                }
            };
            register(&config, &mut active);
            if !active.iter().all(|item| *item) {
                let _ = notifications.send(Notice::Error(
                    "Un raccourci est déjà utilisé : change-le dans les réglages".into(),
                ));
                context.request_repaint();
            }
            let mut msg = MSG::default();
            let mut paused = false;
            while !stop.load(Ordering::Relaxed) {
                while let Ok(next) = settings_rx.try_recv() {
                    match next {
                        Control::Automatic(enabled) => {
                            automatic.set_enabled(enabled);
                            config.automatic = enabled;
                        }
                        Control::Pause => {
                            for (index, registered) in active.iter().enumerate() {
                                if *registered {
                                    let _ =
                                        unsafe { UnregisterHotKey(None, 0x52A1 + index as i32) };
                                }
                            }
                            active = [false; 3];
                            paused = true;
                        }
                        Control::Configure(value) => {
                            for (index, registered) in active.iter().enumerate() {
                                if *registered {
                                    let _ =
                                        unsafe { UnregisterHotKey(None, 0x52A1 + index as i32) };
                                }
                            }
                            active = [false; 3];
                            automatic.set_enabled(value.automatic);
                            config = value;
                            paused = false;
                            register(&config, &mut active);
                            if !active.iter().all(|item| *item) {
                                let _ = notifications
                                    .send(Notice::Error("Un raccourci est indisponible".into()));
                                context.request_repaint();
                            }
                        }
                    }
                }
                if !paused {
                    if let Ok(state) = cache.read() {
                        if let Some(command) = automatic.observe(&state) {
                            let notice = copy(&command);
                            let _ = notifications.send(notice);
                            context.request_repaint();
                        }
                    }
                }
                while unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() } {
                    if msg.message != WM_HOTKEY || paused {
                        continue;
                    }
                    let action = match msg.wParam.0 as i32 {
                        0x52A1 => Action::Zaap,
                        0x52A2 => Action::Travel,
                        0x52A3 => Action::Combined,
                        _ => continue,
                    };
                    let command = cache
                        .read()
                        .map_err(|_| "Données indisponibles".to_owned())
                        .and_then(|state| state.command(action));
                    let notice = match command {
                        Ok(command) => copy(&command),
                        Err(message) => Notice::Error(message),
                    };
                    let _ = notifications.send(notice);
                    context.request_repaint();
                }
                thread::sleep(Duration::from_millis(12));
            }
            for (index, registered) in active.iter().enumerate() {
                if *registered {
                    let _ = unsafe { UnregisterHotKey(None, 0x52A1 + index as i32) };
                }
            }
        })
        .expect("impossible de démarrer les raccourcis Windows");
    settings_tx
}

#[cfg(test)]
mod tests {
    use super::*;
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
