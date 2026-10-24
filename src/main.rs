#![cfg_attr(all(not(debug_assertions), not(test)), windows_subsystem = "windows")]

#[cfg(not(target_os = "windows"))]
compile_error!("Zaaps Dofus 3 est une application Windows uniquement.");

mod app;
mod commands;
mod data;
mod desktop;
mod diagnostics;
mod docking;
mod ganymede;
mod hotkeys;
mod settings;
mod tray;
mod updates;
mod window_corners;

fn main() -> eframe::Result {
    let prefs = hotkeys::Shortcuts::load();
    let compact = prefs.compact;
    let icon = eframe::egui::IconData {
        rgba: include_bytes!("../assets/icon.rgba").to_vec(),
        width: 64,
        height: 64,
    };
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Zaaps Dofus 3")
            .with_inner_size(if prefs.dock_enabled {
                [460.0, 225.0]
            } else if compact {
                [460.0, 360.0]
            } else {
                [960.0, 750.0]
            })
            .with_min_inner_size(if prefs.dock_enabled {
                [320.0, 225.0]
            } else {
                [400.0, 300.0]
            })
            .with_decorations(false)
            .with_resizable(true)
            .with_icon(icon),
        ..Default::default()
    };
    eframe::run_native(
        "Zaaps Dofus 3",
        options,
        Box::new(|cc| Ok(Box::new(app::ZaapsApp::new(cc)))),
    )
}
