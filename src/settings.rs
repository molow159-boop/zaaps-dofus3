use crate::hotkeys::parse_shortcut;
use std::{fs, path::PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feedback {
    Visual,
    Sound,
    Silent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shortcuts {
    pub zaap: String,
    pub travel: String,
    pub combined: String,
    pub toggle_auto: String,
    pub toggle_window: String,
    pub automatic: bool,
    pub compact: bool,
    pub always_on_top: bool,
    pub feedback: Feedback,
    pub startup: bool,
    pub check_updates: bool,
    pub watch_clipboard: bool,
    pub dock_enabled: bool,
    pub dock_side: crate::docking::Side,
}

impl Default for Shortcuts {
    fn default() -> Self {
        Self {
            zaap: "Ctrl+J".into(),
            travel: "Ctrl+K".into(),
            combined: "Ctrl+L".into(),
            toggle_auto: "Ctrl+Shift+A".into(),
            toggle_window: "Ctrl+Shift+Z".into(),
            automatic: false,
            compact: true,
            always_on_top: false,
            feedback: Feedback::Visual,
            startup: false,
            check_updates: true,
            watch_clipboard: true,
            dock_enabled: false,
            dock_side: crate::docking::Side::Bottom,
        }
    }
}

fn settings_path() -> PathBuf {
    let folder = std::env::var_os("ZAAPS_SETTINGS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("APPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
                .join("Zaaps Dofus 3")
        });
    folder.join("raccourcis.ini")
}

impl Shortcuts {
    pub fn window_level(&self) -> eframe::egui::WindowLevel {
        if self.always_on_top {
            eframe::egui::WindowLevel::AlwaysOnTop
        } else {
            eframe::egui::WindowLevel::Normal
        }
    }

    pub fn keys(&self) -> [&str; 5] {
        [
            &self.zaap,
            &self.travel,
            &self.combined,
            &self.toggle_auto,
            &self.toggle_window,
        ]
    }

    pub fn load() -> Self {
        fs::read_to_string(settings_path())
            .map(|text| Self::from_ini(&text))
            .unwrap_or_default()
    }

    fn from_ini(contents: &str) -> Self {
        let mut settings = Self::default();
        for line in contents.trim_start_matches('\u{feff}').lines() {
            let Some((field, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            let enabled = value.eq_ignore_ascii_case("on")
                || value == "1"
                || value.eq_ignore_ascii_case("true");
            match field.trim() {
                "automatic" => settings.automatic = enabled,
                "compact" => settings.compact = enabled,
                "always_on_top" => settings.always_on_top = enabled,
                "startup" => settings.startup = enabled,
                "check_updates" => settings.check_updates = enabled,
                "watch_clipboard" => settings.watch_clipboard = enabled,
                "dock_enabled" => settings.dock_enabled = enabled,
                "dock_side" => settings.dock_side = crate::docking::Side::parse(value),
                "feedback" => {
                    settings.feedback = match value {
                        "sound" => Feedback::Sound,
                        "silent" => Feedback::Silent,
                        _ => Feedback::Visual,
                    }
                }
                field if parse_shortcut(value).is_some() => match field {
                    "zaap" => settings.zaap = value.into(),
                    "travel" => settings.travel = value.into(),
                    "combined" => settings.combined = value.into(),
                    "toggle_auto" => settings.toggle_auto = value.into(),
                    "toggle_window" => settings.toggle_window = value.into(),
                    _ => {}
                },
                _ => {}
            }
        }
        let mut used = Vec::new();
        for key in [
            &mut settings.zaap,
            &mut settings.travel,
            &mut settings.combined,
            &mut settings.toggle_auto,
            &mut settings.toggle_window,
        ] {
            if parse_shortcut(key).is_none_or(|binding| used.contains(&binding)) {
                for fallback in [
                    "Ctrl+J",
                    "Ctrl+K",
                    "Ctrl+L",
                    "Ctrl+Shift+A",
                    "Ctrl+Shift+Z",
                    "Ctrl+Shift+F11",
                ] {
                    let binding = parse_shortcut(fallback).unwrap();
                    if !used.contains(&binding) {
                        *key = fallback.into();
                        break;
                    }
                }
            }
            used.push(parse_shortcut(key).unwrap());
        }
        settings
    }

    fn to_ini(&self) -> String {
        format!("zaap={}\r\ntravel={}\r\ncombined={}\r\ntoggle_auto={}\r\ntoggle_window={}\r\nautomatic={}\r\ncompact={}\r\nalways_on_top={}\r\nfeedback={}\r\nstartup={}\r\ncheck_updates={}\r\nwatch_clipboard={}\r\ndock_enabled={}\r\ndock_side={}\r\n",
            self.zaap, self.travel, self.combined, self.toggle_auto, self.toggle_window,
            self.automatic, self.compact, self.always_on_top, match self.feedback { Feedback::Visual => "visual", Feedback::Sound => "sound", Feedback::Silent => "silent" },
            self.startup, self.check_updates, self.watch_clipboard, self.dock_enabled, self.dock_side.key())
    }

    pub fn save(&self) -> Result<(), String> {
        static SAVE: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = SAVE
            .lock()
            .map_err(|_| "Enregistrement des réglages indisponible")?;
        self.validate()?;
        let path = settings_path();
        fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        let temporary = path.with_extension("ini.tmp");
        fs::write(&temporary, self.to_ini()).map_err(|e| e.to_string())?;
        fs::rename(temporary, path).map_err(|e| e.to_string())
    }

    pub fn validate(&self) -> Result<(), String> {
        let mut used = Vec::new();
        for key in self.keys() {
            let binding = parse_shortcut(key)
                .ok_or("Raccourci invalide : utilise Ctrl, Alt ou Shift avec A–Z, 0–9 ou F1–F12")?;
            if used.contains(&binding) {
                return Err("Choisis cinq raccourcis différents".into());
            }
            used.push(binding);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_preferences_keep_the_three_keys_and_do_not_enable_startup() {
        let value =
            Shortcuts::from_ini("\u{feff}zaap=Alt+J\ntravel=Alt+K\ncombined=Alt+L\nautomatic=on\n");
        assert_eq!(value.keys()[..3], ["Alt+J", "Alt+K", "Alt+L"]);
        assert!(value.automatic && value.compact);
        assert!(!value.startup);
        assert!(!value.always_on_top);
        assert!(!value.dock_enabled);
        assert!(value.validate().is_ok());
    }
    #[test]
    fn all_preferences_round_trip_and_semantic_conflicts_are_detected() {
        let value = Shortcuts {
            always_on_top: true,
            feedback: Feedback::Sound,
            compact: false,
            watch_clipboard: false,
            check_updates: false,
            startup: true,
            dock_enabled: true,
            dock_side: crate::docking::Side::Right,
            ..Shortcuts::default()
        };
        assert_eq!(Shortcuts::from_ini(&value.to_ini()), value);
        let mut conflict = value;
        conflict.toggle_auto = "Shift+Control+Z".into();
        assert!(conflict.validate().is_err());
        assert!(
            Shortcuts::from_ini("zaap=Ctrl+Shift+A\ncombined=bad\ntoggle_window=Shift+Ctrl+A")
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn disabling_always_on_top_survives_reload_without_changing_other_preferences() {
        let mut value = Shortcuts::from_ini(
            "zaap=Alt+J\ntravel=Alt+K\ncombined=Alt+L\nautomatic=true\ndock_enabled=true\ndock_side=left\nalways_on_top=true",
        );
        assert!(value.always_on_top);
        let expected = Shortcuts {
            always_on_top: false,
            ..value.clone()
        };
        value.always_on_top = false;
        assert_eq!(Shortcuts::from_ini(&value.to_ini()), expected);
    }

    #[test]
    fn docking_preferences_keep_each_side_and_unknown_values_are_safe() {
        for side in crate::docking::Side::ALL {
            let value = Shortcuts {
                dock_enabled: true,
                dock_side: side,
                ..Shortcuts::default()
            };
            assert_eq!(Shortcuts::from_ini(&value.to_ini()), value);
        }
        let value = Shortcuts::from_ini("dock_side=invalid\ndock_enabled=false");
        assert_eq!(value.dock_side, crate::docking::Side::Bottom);
        assert!(!value.dock_enabled);
    }
}
