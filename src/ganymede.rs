use crate::{commands::Step, data::Zaap};
use std::{
    ffi::c_void,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, RwLock,
    },
    thread,
    time::Duration,
};
use uiautomation::{
    controls::ControlType,
    patterns::{UITextPattern, UIValuePattern},
    types::{TreeScope, UIProperty},
    UIAutomation, UIElement,
};

#[link(name = "kernel32")]
extern "system" {
    fn OpenProcess(access: u32, inherit_handle: i32, process_id: u32) -> *mut c_void;
    fn QueryFullProcessImageNameW(
        process: *mut c_void,
        flags: u32,
        name: *mut u16,
        length: *mut u32,
    ) -> i32;
    fn CloseHandle(handle: *mut c_void) -> i32;
}

fn is_ganymede_window(element: &UIElement) -> bool {
    if let Ok(title) = element.get_name() {
        if crate::data::normalize(&title).contains("ganymede") {
            return true;
        }
    }
    let Ok(pid) = element.get_process_id() else {
        return false;
    };
    unsafe {
        let handle = OpenProcess(0x1000, 0, pid);
        if handle.is_null() {
            return false;
        }
        let mut name = [0u16; 1024];
        let mut len = name.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, 0, name.as_mut_ptr(), &mut len);
        CloseHandle(handle);
        if ok == 0 {
            return false;
        }
        let path = String::from_utf16_lossy(&name[..len as usize]);
        let exe = path.rsplit(['\\', '/']).next().unwrap_or("");
        crate::data::normalize(exe).contains("ganymede")
    }
}

fn relevant_control(kind: ControlType, automation_id: &str) -> bool {
    matches!(
        kind,
        ControlType::Text | ControlType::Document | ControlType::Hyperlink | ControlType::Edit
    ) || (kind == ControlType::Button && automation_id.starts_with("copy-position-"))
}

fn read_window_text(
    automation: &UIAutomation,
    cached: &mut Option<UIElement>,
) -> Result<String, String> {
    if cached
        .as_ref()
        .is_some_and(|element| !is_ganymede_window(element))
    {
        *cached = None;
    }
    if cached.is_none() {
        let desktop = automation
            .get_root_element()
            .map_err(|_| "Bureau Windows inaccessible")?;
        let condition = automation
            .create_true_condition()
            .map_err(|_| "Filtre d'accessibilité indisponible")?;
        let windows = desktop
            .find_all(TreeScope::Children, &condition)
            .map_err(|_| "Fenêtres Windows inaccessibles")?;
        *cached = windows.into_iter().find(is_ganymede_window);
    }
    let window = cached.as_ref().ok_or("Fenêtre Ganymède introuvable")?;
    let all = automation
        .create_true_condition()
        .map_err(|_| "Filtre inaccessible")?;
    let request = automation
        .create_cache_request()
        .map_err(|_| "Cache Windows indisponible")?;
    for property in [
        UIProperty::Name,
        UIProperty::ControlType,
        UIProperty::IsOffscreen,
        UIProperty::AutomationId,
    ] {
        request
            .add_property(property)
            .map_err(|_| "Propriété Windows indisponible")?;
    }
    let controls = window
        .find_all_build_cache(TreeScope::Descendants, &all, &request)
        .map_err(|_| "Lecture Ganymède indisponible")?;
    if controls.len() > 4000 {
        return Err("Arborescence Ganymède trop volumineuse".into());
    }
    let mut text = Vec::new();
    let mut documents = Vec::new();
    for element in controls {
        if element.is_cached_offscreen().unwrap_or(true) {
            continue;
        }
        let Ok(kind) = element.get_cached_control_type() else {
            continue;
        };
        let automation_id = if kind == ControlType::Button {
            element.get_cached_automation_id().unwrap_or_default()
        } else {
            String::new()
        };
        if !relevant_control(kind, &automation_id) {
            continue;
        }
        if kind == ControlType::Document {
            documents.push(element);
            continue;
        }
        let mut name = element.get_cached_name().unwrap_or_default();
        if name.trim().is_empty() && kind == ControlType::Edit {
            if let Ok(pattern) = element.get_pattern::<UIValuePattern>() {
                name = pattern.get_value().unwrap_or_default();
            }
        }
        if !name.trim().is_empty() && name.len() < 1000 {
            text.push(name);
        }
    }
    if text.is_empty() {
        for document in documents {
            if let Ok(pattern) = document.get_pattern::<UITextPattern>() {
                if let Ok(range) = pattern.get_document_range() {
                    if let Ok(full) = range.get_text(12000) {
                        if !full.trim().is_empty() {
                            text.push(full);
                        }
                    }
                }
            }
        }
    }
    if text.is_empty() {
        return Err("Aucun texte accessible dans Ganymède".into());
    }
    Ok(text.join("\n"))
}

fn stable_step(previous: &mut Option<String>, text: String, directory: &[Zaap]) -> Step {
    let step = Step::from_text(&text, directory);
    let key = format!(
        "{:?}|{:?}|{:?}|{:?}",
        step.identity, step.zaap, step.travel, step.error
    );
    let stable = previous.as_ref() == Some(&key);
    *previous = Some(key);
    if stable {
        step
    } else {
        Step {
            identity: step.identity,
            ..Step::unavailable("Étape en cours de synchronisation")
        }
    }
}

pub(crate) fn refresh(
    cache: &RwLock<Step>,
    previous: &mut Option<String>,
    directory: &[Zaap],
    read: impl FnOnce() -> Result<String, String>,
) -> bool {
    let result = read();
    let available = result.is_ok();
    let update = match result {
        Ok(text) => stable_step(previous, text, directory),
        Err(message) => {
            *previous = None;
            Step::unavailable(message)
        }
    };
    if let Ok(mut state) = cache.write() {
        *state = update;
    }
    available
}

pub fn start(cache: Arc<RwLock<Step>>, directory: Vec<Zaap>, stop: Arc<AtomicBool>) {
    thread::Builder::new()
        .name("ganymede-reader".into())
        .spawn(move || {
            let automation = match UIAutomation::new() {
                Ok(automation) => automation,
                Err(_) => {
                    if let Ok(mut state) = cache.write() {
                        *state = Step::unavailable("Accessibilité Windows indisponible");
                    }
                    return;
                }
            };
            let mut window = None;
            let mut previous_text = None;
            while !stop.load(Ordering::Relaxed) {
                // Publish only completed observations. Never erase a confirmed
                // snapshot merely because another traversal has started.
                if !refresh(&cache, &mut previous_text, &directory, || {
                    read_window_text(&automation, &mut window)
                }) {
                    window = None;
                }
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                thread::sleep(Duration::from_millis(50));
            }
        })
        .expect("impossible de démarrer le lecteur Ganymède");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::Action;
    #[test]
    fn real_ganymede_tree_includes_destination_buttons_but_not_header_position() {
        let nodes = [
            (ControlType::Button, "", "[-32,40]"),
            (ControlType::Text, "", "497"),
            (ControlType::Text, "", "/"),
            (ControlType::Text, "", "774"),
            (ControlType::Text, "", "Ensuite"),
            (ControlType::Text, "", "allez en"),
            (ControlType::Button, "copy-position-24--27", "[24,-27]"),
            (ControlType::Text, "", "Zaap"),
            (ControlType::Text, "", "du"),
            (ControlType::Text, "", "Village de Pandala"),
        ];
        let text = nodes
            .iter()
            .filter(|(kind, id, _)| relevant_control(*kind, id))
            .map(|(_, _, name)| *name)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!text.contains("[-32,40]"));
        let step = Step::from_text(&text, &crate::data::embedded().unwrap());
        assert_eq!(step.identity.as_deref(), Some("step:497/774"));
        assert_eq!(
            step.command(Action::Combined).unwrap(),
            "/zaap 20,-29; /zaap 20,-29; /travel 24 -27"
        );
    }
    #[test]
    fn transition_invalidates_old_and_mixed_routes() {
        let known = crate::data::embedded().unwrap();
        let mut previous = None;
        let old = "1/10\nZaap de Bonta\nAllez en [1,2]";
        assert!(stable_step(&mut previous, old.into(), &known)
            .command(Action::Combined)
            .is_err());
        assert!(stable_step(&mut previous, old.into(), &known)
            .command(Action::Combined)
            .is_ok());
        for partial in [
            "2/10\nZaap de Bonta\nAllez en [1,2]",
            "2/10",
            "2/10\nAllez en [3,4]",
        ] {
            let step = stable_step(&mut previous, partial.into(), &known);
            assert!(step.zaap.is_none() && step.travel.is_none());
            assert!(step.command(Action::Combined).is_err());
        }
        assert_eq!(
            stable_step(&mut previous, "2/10\nAllez en [3,4]".into(), &known)
                .command(Action::Combined)
                .unwrap(),
            "/travel 3 4"
        );
    }
}
