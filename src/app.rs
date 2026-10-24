use crate::{
    commands::{Action, Step},
    data::{self, Zaap},
    desktop, diagnostics, docking, ganymede,
    hotkeys::{self, Control, Feedback, Notice, Shortcuts},
    tray, updates,
    window_corners::WindowCorners,
};
use eframe::egui::{
    self, viewport::ResizeDirection, Align2, Color32, ColorImage, CursorIcon, FontData,
    FontDefinitions, FontFamily, FontId, Pos2, Rect, RichText, Rounding, Sense, Stroke,
    TextureHandle, TextureOptions, Vec2,
};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, RwLock,
    },
    time::{Duration, Instant},
};

const BG: Color32 = Color32::from_rgb(10, 25, 35);
const BAR: Color32 = Color32::from_rgb(12, 29, 40);
const CARD: Color32 = Color32::from_rgb(23, 42, 54);
const CARD_HOVER: Color32 = Color32::from_rgb(30, 61, 67);
const BORDER: Color32 = Color32::from_rgb(48, 74, 85);
const GREEN: Color32 = Color32::from_rgb(155, 219, 101);
const WHITE: Color32 = Color32::from_rgb(243, 247, 250);
const MUTED: Color32 = Color32::from_rgb(168, 191, 205);

const CARD_HEIGHT: f32 = 49.0;
const CARD_GAP: f32 = 5.0;
const MIN_COLUMN_WIDTH: f32 = 340.0;

struct Icons {
    brand: TextureHandle,
    portal: TextureHandle,
    copy: TextureHandle,
    search: TextureHandle,
    gear: TextureHandle,
    check: TextureHandle,
}

fn icon(ctx: &egui::Context, label: &str, raw: &'static [u8], size: [usize; 2]) -> TextureHandle {
    ctx.load_texture(
        label,
        ColorImage::from_rgba_unmultiplied(size, raw),
        TextureOptions::LINEAR,
    )
}

impl Icons {
    fn new(ctx: &egui::Context) -> Self {
        Self {
            brand: icon(
                ctx,
                "brand",
                include_bytes!("../assets/brand.rgba"),
                [124, 124],
            ),
            portal: icon(
                ctx,
                "portal",
                include_bytes!("../assets/portal.rgba"),
                [132, 144],
            ),
            copy: icon(ctx, "copy", include_bytes!("../assets/copy.rgba"), [64, 80]),
            search: icon(
                ctx,
                "search",
                include_bytes!("../assets/search.rgba"),
                [76, 76],
            ),
            gear: icon(ctx, "gear", include_bytes!("../assets/gear.rgba"), [80, 80]),
            check: icon(
                ctx,
                "check",
                include_bytes!("../assets/check.rgba"),
                [56, 56],
            ),
        }
    }
}

pub struct ZaapsApp {
    entries: Vec<Zaap>,
    query: String,
    shortcuts: Shortcuts,
    draft: Shortcuts,
    settings_open: bool,
    recording: Option<usize>,
    toast: String,
    success: bool,
    toast_time: Instant,
    notices: mpsc::Receiver<Notice>,
    hotkey_settings: mpsc::Sender<Control>,
    stop: Arc<AtomicBool>,
    maximized: bool,
    icons: Icons,
    window_corners: WindowCorners,
    cache: Arc<RwLock<Step>>,
    tray: Option<tray::Tray>,
    tray_events: Option<mpsc::Receiver<tray::Event>>,
    update_requests: mpsc::Sender<updates::Request>,
    update_notices: mpsc::Receiver<updates::ResultNotice>,
    latest_release: Option<updates::Release>,
    update_status: String,
    manual_update_check: bool,
    hidden: bool,
    exiting: bool,
    initial_visibility_pending: bool,
    clipboard_replaced: bool,
    diagnostic_open: bool,
    diagnostic_text: String,
    compact_size: Vec2,
    directory_size: Vec2,
    dock_config: mpsc::Sender<docking::Config>,
    dock_status: Arc<RwLock<String>>,
    slim_layout: bool,
    manager_open: bool,
}

fn text(value: impl Into<String>, size: f32, color: Color32) -> RichText {
    RichText::new(value).size(size).color(color)
}

fn image_button(ui: &mut egui::Ui, icon: &TextureHandle, title: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(31.0), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, Rounding::same(5.0), CARD_HOVER);
    }
    ui.painter().image(
        icon.id(),
        Rect::from_center_size(rect.center(), Vec2::splat(18.0)),
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        WHITE,
    );
    response.on_hover_text(title)
}

fn title_button(ui: &mut egui::Ui, label: &str, title: &str) -> egui::Response {
    ui.add_sized(
        [31.0, 31.0],
        egui::Button::new(text(label, 16.0, WHITE))
            .fill(BAR)
            .stroke(Stroke::NONE),
    )
    .on_hover_text(title)
}

fn dock_controls(ui: &mut egui::Ui, prefs: &mut Shortcuts, id: &str) -> bool {
    let before = (prefs.dock_enabled, prefs.dock_side);
    ui.horizontal_wrapped(|ui| {
        ui.checkbox(&mut prefs.dock_enabled, "Accrocher à Ganymède");
        egui::ComboBox::from_id_salt(id)
            .selected_text(prefs.dock_side.label())
            .width(85.0)
            .show_ui(ui, |ui| {
                for side in docking::Side::ALL {
                    ui.selectable_value(&mut prefs.dock_side, side, side.label());
                }
            });
    });
    before != (prefs.dock_enabled, prefs.dock_side)
}

impl ZaapsApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut fonts = FontDefinitions::default();
        if let Ok(bytes) = std::fs::read(r"C:\Windows\Fonts\segoeui.ttf") {
            fonts
                .font_data
                .insert("Segoe UI".into(), FontData::from_owned(bytes));
            if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
                family.insert(0, "Segoe UI".into());
            }
        }
        cc.egui_ctx.set_fonts(fonts);
        let mut visuals = egui::Visuals::dark();
        visuals.window_fill = BG;
        visuals.panel_fill = BG;
        visuals.extreme_bg_color = BAR;
        visuals.widgets.inactive.bg_fill = CARD;
        visuals.widgets.hovered.bg_fill = CARD_HOVER;
        visuals.widgets.active.bg_fill = Color32::from_rgb(38, 79, 62);
        visuals.selection.bg_fill = Color32::from_rgb(36, 105, 66);
        cc.egui_ctx.set_visuals(visuals);
        let (directory, data_error) = match data::load() {
            Ok(directory) => (directory, None),
            Err(error) => (
                data::embedded().expect("répertoire interne invalide"),
                Some(format!("zaaps.csv invalide : {error}")),
            ),
        };
        let stop = Arc::new(AtomicBool::new(false));
        let cache = Arc::new(RwLock::new(Step::default()));
        let mut shortcuts = Shortcuts::load();
        if shortcuts.dock_enabled {
            shortcuts.compact = true;
        }
        shortcuts.startup = desktop::startup_registered();
        let (tx, notices) = mpsc::channel();
        let (tray, tray_events, tray_error) = match tray::start(cc.egui_ctx.clone(), stop.clone()) {
            Ok((icon, events)) => (Some(icon), Some(events), None),
            Err(error) => (None, None, Some(error)),
        };
        let (update_requests, update_notices) = updates::start(
            shortcuts.check_updates,
            cc.egui_ctx.clone(),
            stop.clone(),
            tray.clone(),
        );
        ganymede::start(Arc::clone(&cache), directory.clone(), Arc::clone(&stop));
        let hotkey_settings = hotkeys::start(
            cache.clone(),
            shortcuts.clone(),
            tx,
            Arc::clone(&stop),
            cc.egui_ctx.clone(),
            tray.clone(),
        );
        if let Some(tray) = &tray {
            tray.bind_controls(hotkey_settings.clone());
        }
        let (dock_config, dock_status) = docking::start(
            docking::Config {
                enabled: shortcuts.dock_enabled,
                side: shortcuts.dock_side,
            },
            stop.clone(),
            cc.egui_ctx.clone(),
        );
        Self {
            slim_layout: shortcuts.dock_enabled,
            manager_open: false,
            entries: directory,
            query: String::new(),
            draft: shortcuts.clone(),
            shortcuts,
            settings_open: false,
            recording: None,
            toast: data_error
                .or(tray_error)
                .unwrap_or_else(|| "Prêt à copier".into()),
            success: false,
            toast_time: Instant::now(),
            notices,
            hotkey_settings,
            stop,
            maximized: false,
            icons: Icons::new(&cc.egui_ctx),
            window_corners: WindowCorners::default(),
            cache,
            tray,
            tray_events,
            update_requests,
            update_notices,
            latest_release: None,
            update_status: "Vérification disponible dans les réglages".into(),
            manual_update_check: false,
            hidden: false,
            exiting: false,
            initial_visibility_pending: std::env::args().any(|arg| arg == "--tray"),
            clipboard_replaced: false,
            diagnostic_open: false,
            diagnostic_text: String::new(),
            compact_size: Vec2::new(460.0, 360.0),
            directory_size: Vec2::new(960.0, 750.0),
            dock_config,
            dock_status,
        }
    }

    fn show_notice(&mut self, notice: Notice) {
        match notice {
            Notice::Copied(command) => {
                self.toast = format!("Commande copiée !  {command}");
                self.success = true;
                self.clipboard_replaced = false;
            }
            Notice::Info(message) => {
                self.toast = message;
                self.success = false;
            }
            Notice::Error(message) => {
                self.toast = message;
                self.success = false;
            }
            Notice::ClipboardReplaced => {
                self.clipboard_replaced = true;
                self.toast = "Commande remplacée dans le presse-papiers".into();
                self.success = false;
            }
            Notice::AutomaticChanged(enabled) => {
                self.shortcuts.automatic = enabled;
                self.draft.automatic = enabled;
                self.toast = if enabled {
                    "Auto activé — copie à la prochaine étape"
                } else {
                    "Auto suspendu"
                }
                .into();
                self.success = false;
            }
            Notice::WindowVisibility(_) => return,
        }
        self.toast_time = Instant::now();
    }

    fn copy(&mut self, command: String) {
        let _ = self.hotkey_settings.send(Control::CopyText(command));
    }

    fn toggle_automatic(&mut self) {
        let _ = self.hotkey_settings.send(Control::ToggleAutomatic);
    }

    fn begin_settings(&mut self) {
        self.draft = self.shortcuts.clone();
        self.settings_open = true;
        self.recording = None;
        let _ = self.hotkey_settings.send(Control::Pause);
    }

    fn close_settings(&mut self) {
        self.settings_open = false;
        self.recording = None;
        let _ = self
            .hotkey_settings
            .send(Control::Configure(self.shortcuts.clone()));
    }

    fn save_settings(&mut self, ctx: &egui::Context) {
        if self.draft.dock_enabled {
            self.draft.compact = true;
        }
        if let Err(error) = self.draft.validate() {
            self.show_notice(Notice::Error(error));
            return;
        }
        let changed_startup = self.draft.startup != self.shortcuts.startup
            || (self.draft.startup && !desktop::startup_enabled());
        match self.draft.save() {
            Ok(()) => {
                if changed_startup {
                    if let Err(error) = desktop::set_startup(self.draft.startup) {
                        let rollback = self.shortcuts.save();
                        self.show_notice(Notice::Error(if rollback.is_ok() {
                            error
                        } else {
                            format!("{error}. Les préférences n'ont pas pu être restaurées.")
                        }));
                        return;
                    }
                }
                self.shortcuts = self.draft.clone();
                apply_window_level(ctx, &self.shortcuts);
                self.configure_docking();
                let _ = self
                    .update_requests
                    .send(updates::Request::Enabled(self.shortcuts.check_updates));
                self.close_settings();
                self.show_notice(Notice::Info("Réglages enregistrés".into()));
            }
            Err(error) => {
                self.show_notice(Notice::Error(error));
            }
        }
    }

    fn record_key(&mut self, ctx: &egui::Context) {
        let Some(index) = self.recording else { return };
        let key = ctx.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    if modifiers.command && !modifiers.ctrl {
                        return None;
                    }
                    let mut name = String::new();
                    if modifiers.ctrl {
                        name.push_str("Ctrl+");
                    }
                    if modifiers.alt {
                        name.push_str("Alt+");
                    }
                    if modifiers.shift {
                        name.push_str("Shift+");
                    }
                    if name.is_empty() {
                        return None;
                    }
                    let key = key.name().to_uppercase();
                    let allowed = key.len() == 1 && key.as_bytes()[0].is_ascii_alphanumeric()
                        || key
                            .strip_prefix('F')
                            .and_then(|n| n.parse::<u8>().ok())
                            .is_some_and(|n| (1..=12).contains(&n));
                    if allowed {
                        Some(name + &key)
                    } else {
                        None
                    }
                }
                _ => None,
            })
        });
        if let Some(key) = key {
            match index {
                0 => self.draft.zaap = key,
                1 => self.draft.travel = key,
                2 => self.draft.combined = key,
                3 => self.draft.toggle_auto = key,
                _ => self.draft.toggle_window = key,
            }
            self.recording = None;
        }
    }

    fn settings(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        self.record_key(ctx);
        let mut open = true;
        egui::Window::new("Réglages")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(380.0)
            .max_width(440.0)
            .max_height((ctx.screen_rect().height() - 45.0).max(200.0))
            .vscroll(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .frame(
                egui::Frame::none()
                    .fill(BAR)
                    .rounding(11.0)
                    .stroke(Stroke::new(1.0_f32, BORDER))
                    .inner_margin(16.0),
            )
            .show(ctx, |ui| {
                let width = ui.available_width().max(250.0);
                egui::ScrollArea::vertical()
                    .id_salt("settings-scroll")
                    .max_height((ctx.screen_rect().height() - 160.0).clamp(100.0, 510.0))
                    .show(ui, |ui| {
                        ui.label(text(
                            "Clique sur un raccourci puis appuie sur les touches.",
                            12.5,
                            MUTED,
                        ));
                        ui.add_space(12.0);
                        let fields = [
                            ("Zaap recommandé", self.draft.zaap.clone()),
                            ("Coordonnées de quête", self.draft.travel.clone()),
                            ("2 zaaps + trajet", self.draft.combined.clone()),
                            ("Suspendre / reprendre Auto", self.draft.toggle_auto.clone()),
                            ("Afficher / masquer", self.draft.toggle_window.clone()),
                        ];
                        for (index, (label, value)) in fields.iter().enumerate() {
                            let line = if self.recording == Some(index) {
                                format!("{label} : appuie sur les touches…")
                            } else {
                                format!("{label} : {value}")
                            };
                            if ui
                                .add_sized(
                                    [width, 39.0],
                                    egui::Button::new(text(line, 12.5, WHITE))
                                        .fill(if self.recording == Some(index) {
                                            Color32::from_rgb(43, 99, 68)
                                        } else {
                                            CARD
                                        })
                                        .stroke(Stroke::new(1.0_f32, BORDER))
                                        .rounding(7.0),
                                )
                                .clicked()
                            {
                                self.recording = Some(index);
                            }
                            ui.add_space(6.0);
                        }
                        ui.separator();
                        ui.checkbox(&mut self.draft.always_on_top, "Toujours au premier plan");
                        ui.label(text("Garder les fenêtres de Zaaps au-dessus des autres applications.", 11.0, MUTED));
                        ui.separator();
                        ui.label(text("Accrochage des fenêtres", 13.0, WHITE));
                        dock_controls(ui, &mut self.draft, "dock-settings");
                        ui.label(text("Déplacer l'une entraîne l'autre. En haut ou en bas, Zaaps suit la largeur de Ganymède. Pause si une fenêtre est réduite, masquée ou maximisée.", 11.0, MUTED));
                        ui.separator();
                        ui.label(text("Confirmation de copie", 13.0, WHITE));
                        ui.horizontal_wrapped(|ui| {
                            ui.selectable_value(
                                &mut self.draft.feedback,
                                Feedback::Visual,
                                "Visuelle",
                            );
                            ui.selectable_value(
                                &mut self.draft.feedback,
                                Feedback::Sound,
                                "Son léger",
                            );
                            ui.selectable_value(
                                &mut self.draft.feedback,
                                Feedback::Silent,
                                "Silence",
                            );
                        });
                        ui.checkbox(
                            &mut self.draft.watch_clipboard,
                            "Avertir si la commande copiée est remplacée",
                        );
                        ui.checkbox(
                            &mut self.draft.startup,
                            "Lancer avec Windows, près de l'horloge",
                        );
                        ui.checkbox(
                            &mut self.draft.check_updates,
                            "Signaler les nouvelles versions sur GitHub",
                        );
                        ui.label(text(
                            "Aucune installation automatique de mise à jour.",
                            11.0,
                            MUTED,
                        ));
                        if ui.button("Vérifier la version maintenant").clicked() {
                            self.manual_update_check = true;
                            self.update_status = "Vérification en cours…".into();
                            let _ = self.update_requests.send(updates::Request::Check);
                        }
                        ui.label(text(&self.update_status, 11.0, MUTED));
                        ui.hyperlink_to("Voir les téléchargements", updates::RELEASES_URL);
                        if ui.button("Diagnostic — aperçu avant partage").clicked() {
                            self.open_diagnostic();
                        }
                    });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui
                        .add_sized([100.0, 33.0], egui::Button::new("Annuler"))
                        .clicked()
                    {
                        self.close_settings();
                    }
                    if ui
                        .add_sized(
                            [130.0, 33.0],
                            egui::Button::new(text("Enregistrer", 13.0, WHITE))
                                .fill(Color32::from_rgb(39, 111, 69)),
                        )
                        .clicked()
                    {
                        self.save_settings(ctx);
                    }
                });
            });
        if !open {
            self.close_settings();
        }
    }

    fn heading(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("heading")
            .frame(
                egui::Frame::none()
                    .fill(BAR)
                    .inner_margin(egui::Margin::symmetric(10.0, 6.0)),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                ui.horizontal(|ui| {
                    ui.add(egui::Image::new((self.icons.brand.id(), Vec2::splat(28.0))));
                    let title = if self.shortcuts.dock_enabled && ctx.screen_rect().width() < 400.0
                    {
                        "Zaaps"
                    } else {
                        "Zaaps Dofus 3"
                    };
                    ui.label(text(title, 16.0, WHITE).strong());
                    let controls = if self.shortcuts.compact { 183.0 } else { 217.0 };
                    let drag = ui.add_sized(
                        [(ui.available_width() - controls).max(1.0), 30.0],
                        egui::Label::new("").sense(Sense::click_and_drag()),
                    );
                    if drag.drag_started() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }
                    if drag.double_clicked() && !self.shortcuts.dock_enabled {
                        self.switch_maximize(ctx);
                    }
                    if ui
                        .add_sized(
                            [78.0, 29.0],
                            egui::Button::new(text(
                                if self.shortcuts.automatic {
                                    "Auto : ON"
                                } else {
                                    "Auto : OFF"
                                },
                                12.0,
                                WHITE,
                            ))
                            .fill(if self.shortcuts.automatic {
                                Color32::from_rgb(39, 111, 69)
                            } else {
                                CARD
                            })
                            .rounding(7.0),
                        )
                        .on_hover_text(format!(
                            "Suspendre / reprendre : {}",
                            self.shortcuts.toggle_auto
                        ))
                        .clicked()
                    {
                        self.toggle_automatic();
                    }
                    if image_button(ui, &self.icons.gear, "Réglages").clicked() {
                        self.begin_settings();
                    }
                    if title_button(ui, "−", "Réduire près de l'horloge").clicked() {
                        self.hide(ctx);
                    }
                    if !self.shortcuts.compact
                        && title_button(ui, "□", "Agrandir ou restaurer").clicked()
                    {
                        self.switch_maximize(ctx);
                    }
                    if title_button(ui, "×", "Quitter l'application").clicked() {
                        self.quit(ctx);
                    }
                });
            });
    }

    fn tray_available(&self) -> bool {
        self.tray.as_ref().is_some_and(tray::Tray::available)
    }

    fn hide(&mut self, ctx: &egui::Context) {
        self.manager_open = false;
        if self.settings_open {
            self.close_settings();
        }
        self.diagnostic_open = false;
        if self.tray_available() {
            self.hidden = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        } else {
            self.hidden = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            self.show_notice(Notice::Info(
                "Icône indisponible : réduction dans la barre des tâches".into(),
            ));
        }
    }

    fn show(&mut self, ctx: &egui::Context) {
        self.hidden = false;
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }

    fn quit(&mut self, ctx: &egui::Context) {
        self.exiting = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn toggle_compact(&mut self, ctx: &egui::Context) {
        let size = ctx.screen_rect().size();
        if self.shortcuts.compact {
            self.compact_size = size;
        } else {
            self.directory_size = size;
        }
        self.shortcuts.compact = !self.shortcuts.compact;
        if let Err(error) = self.shortcuts.save() {
            self.shortcuts.compact = !self.shortcuts.compact;
            self.show_notice(Notice::Error(error));
            return;
        }
        self.draft.compact = self.shortcuts.compact;
        let _ = self
            .hotkey_settings
            .send(Control::Configure(self.shortcuts.clone()));
        self.maximized = false;
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(
            if self.shortcuts.compact {
                self.compact_size
            } else {
                self.directory_size
            },
        ));
    }

    fn copy_action(&self, action: Action) {
        let _ = self.hotkey_settings.send(Control::Copy(action));
    }

    fn open_diagnostic(&mut self) {
        if let Ok(step) = self.cache.read() {
            self.diagnostic_text = diagnostics::summary(
                &step,
                &self.shortcuts,
                self.tray_available(),
                self.clipboard_replaced,
            );
        }
        self.diagnostic_open = true;
    }

    fn diagnostic(&mut self, ctx: &egui::Context) {
        if !self.diagnostic_open {
            return;
        }
        let mut open = true;
        egui::Window::new("Diagnostic — aperçu avant partage").open(&mut open)
            .collapsible(false).default_width(390.0)
            .max_height((ctx.screen_rect().height() - 45.0).max(210.0))
            .vscroll(true).anchor(Align2::CENTER_CENTER, Vec2::ZERO).show(ctx, |ui| {
                ui.label("Vérifiez et modifiez ce résumé avant de le copier. Rien n'est envoyé automatiquement.");
                ui.add_space(6.0);
                ui.add(egui::TextEdit::multiline(&mut self.diagnostic_text).desired_rows(9)
                    .desired_width(f32::INFINITY).font(egui::TextStyle::Monospace));
                if ui.button("Copier ce diagnostic").clicked() {
                    let _ = self.hotkey_settings.send(Control::CopyDiagnostic(self.diagnostic_text.clone()));
                }
            });
        self.diagnostic_open = open;
    }

    fn configure_docking(&self) {
        let _ = self.dock_config.send(docking::Config {
            enabled: self.shortcuts.dock_enabled,
            side: self.shortcuts.dock_side,
        });
    }

    fn sync_slim_layout(&mut self, ctx: &egui::Context) {
        let slim = self.shortcuts.dock_enabled;
        if slim == self.slim_layout {
            return;
        }
        self.slim_layout = slim;
        self.maximized = false;
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(if slim {
            Vec2::new(320.0, 225.0)
        } else {
            Vec2::new(400.0, 300.0)
        }));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(if slim {
            Vec2::new(460.0, 225.0)
        } else {
            self.compact_size
        }));
        self.configure_docking();
    }

    fn docked_windows(&mut self, ctx: &egui::Context) {
        if self.manager_open {
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of("zaap-manager"),
                egui::ViewportBuilder::default()
                    .with_title("Gestionnaire de Zaaps")
                    .with_window_level(self.shortcuts.window_level())
                    .with_inner_size(self.directory_size)
                    .with_min_inner_size([400.0, 300.0]),
                |ctx, _| {
                    if ctx.input(|i| i.viewport().close_requested()) {
                        self.manager_open = false;
                    }
                    self.footer(ctx);
                    self.directory(ctx);
                },
            );
        }
        if self.settings_open && self.shortcuts.dock_enabled {
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of("zaap-settings"),
                egui::ViewportBuilder::default()
                    .with_title("Réglages — Zaaps Dofus 3")
                    .with_window_level(self.shortcuts.window_level())
                    .with_inner_size([460.0, 640.0])
                    .with_min_inner_size([400.0, 360.0]),
                |ctx, _| {
                    if ctx.input(|i| i.viewport().close_requested()) {
                        self.close_settings();
                    }
                    egui::CentralPanel::default()
                        .frame(egui::Frame::none().fill(BG))
                        .show(ctx, |_| {});
                    self.settings(ctx);
                },
            );
        }
        if self.diagnostic_open && self.shortcuts.dock_enabled {
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of("zaap-diagnostic"),
                egui::ViewportBuilder::default()
                    .with_title("Diagnostic — Zaaps Dofus 3")
                    .with_window_level(self.shortcuts.window_level())
                    .with_inner_size([460.0, 560.0])
                    .with_min_inner_size([400.0, 360.0]),
                |ctx, _| {
                    if ctx.input(|i| i.viewport().close_requested()) {
                        self.diagnostic_open = false;
                    }
                    egui::CentralPanel::default()
                        .frame(egui::Frame::none().fill(BG))
                        .show(ctx, |_| {});
                    self.diagnostic(ctx);
                },
            );
        }
    }

    fn route_panel(&mut self, ctx: &egui::Context) {
        let step = self
            .cache
            .read()
            .map(|step| step.clone())
            .unwrap_or_default();
        let max_height = if self.shortcuts.compact {
            (ctx.available_rect().height() - 22.0).max(130.0)
        } else {
            230.0
        };
        egui::TopBottomPanel::top("ganymede-route")
            .exact_height(if self.shortcuts.compact {
                ctx.available_rect().height()
            } else {
                255.0
            })
            .frame(
                egui::Frame::none()
                    .fill(BG)
                    .inner_margin(egui::Margin::symmetric(14.0, 10.0)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("route-scroll")
                    .max_height(max_height)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(text("GANYMÈDE", 11.0, MUTED).strong());
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(text(
                                        step.progress()
                                            .map(|value| {
                                                format!("Étape {}", value.replace('/', " / "))
                                            })
                                            .unwrap_or_else(|| "Étape non reconnue".into()),
                                        12.0,
                                        GREEN,
                                    ));
                                },
                            );
                        });
                        ui.add_space(8.0);
                        let zaap = step.command(Action::Zaap).ok();
                        let travel = step.command(Action::Travel).ok();
                        let zaap_name = zaap
                            .as_ref()
                            .and_then(|command| {
                                self.entries.iter().find(|item| item.command() == *command)
                            })
                            .map(|item| format!("{} [{},{}]", item.name, item.x, item.y))
                            .unwrap_or_else(|| {
                                zaap.clone().unwrap_or_else(|| "Aucun Zaap reconnu".into())
                            });
                        egui::Frame::none()
                            .fill(CARD)
                            .rounding(8.0)
                            .inner_margin(10.0)
                            .show(ui, |ui| {
                                ui.set_min_width((ui.available_width() - 1.0).max(100.0));
                                ui.label(text(zaap_name, 14.0, WHITE).strong());
                                ui.label(text(
                                    travel.as_deref().unwrap_or("Aucune destination reconnue"),
                                    13.0,
                                    MUTED,
                                ));
                                if travel.as_deref() == Some("/travel -22 -24")
                                    && zaap.as_deref() == Some("/zaap -20,-20")
                                {
                                    ui.label(text("Tour des Voyageurs Dimensionnels", 11.0, GREEN));
                                }
                            });
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            let width = ((ui.available_width() - 16.0) / 3.0).max(70.0);
                            for (label, action) in [
                                ("Zaap", Action::Zaap),
                                ("Travel", Action::Travel),
                                ("Trajet complet", Action::Combined),
                            ] {
                                if ui
                                    .add_enabled_ui(step.command(action).is_ok(), |ui| {
                                        ui.add_sized(
                                            [width, 33.0],
                                            egui::Button::new(text(label, 12.0, WHITE))
                                                .fill(CARD)
                                                .rounding(6.0),
                                        )
                                    })
                                    .inner
                                    .clicked()
                                {
                                    self.copy_action(action);
                                }
                            }
                        });
                        if let Err(error) = step.command(Action::Combined) {
                            ui.add_space(5.0);
                            ui.label(text(error, 11.0, MUTED));
                        }
                        if self.shortcuts.dock_enabled {
                            ui.horizontal(|ui| {
                                if ui.small_button("Afficher Zaaps").clicked() {
                                    self.manager_open = true;
                                }
                                let status = self
                                    .dock_status
                                    .read()
                                    .map(|s| s.clone())
                                    .unwrap_or_default();
                                let recent = self.toast_time.elapsed() < Duration::from_secs(4);
                                let message = if self.clipboard_replaced {
                                    "Commande remplacée — recopier le trajet"
                                } else if recent {
                                    if self.success {
                                        "Commande copiée"
                                    } else {
                                        self.toast.as_str()
                                    }
                                } else if status != "Fenêtres accrochées" {
                                    status.as_str()
                                } else {
                                    ""
                                };
                                ui.add(egui::Label::new(text(message, 10.0, MUTED)).truncate())
                                    .on_hover_text(message);
                            });
                            return;
                        }
                        if self.clipboard_replaced {
                            ui.add_space(5.0);
                            ui.label(text(
                                "La commande copiée a été remplacée.",
                                12.0,
                                Color32::from_rgb(244, 193, 94),
                            ));
                            if ui
                                .add_enabled(
                                    step.command(Action::Combined).is_ok(),
                                    egui::Button::new("Recopier le trajet courant"),
                                )
                                .clicked()
                            {
                                self.copy_action(Action::Combined);
                            }
                        }
                        if let Some(release) = &self.latest_release {
                            ui.hyperlink_to(
                                format!("Version {} disponible — télécharger", release.version),
                                &release.url,
                            );
                        }
                        ui.add_space(5.0);
                        let mut prefs = self.shortcuts.clone();
                        if dock_controls(ui, &mut prefs, "dock-route") {
                            if prefs.dock_enabled {
                                prefs.compact = true;
                            }
                            match prefs.save() {
                                Ok(()) => {
                                    self.shortcuts = prefs;
                                    self.draft.dock_enabled = self.shortcuts.dock_enabled;
                                    self.draft.dock_side = self.shortcuts.dock_side;
                                    self.configure_docking();
                                    if !self.settings_open {
                                        let _ = self
                                            .hotkey_settings
                                            .send(Control::Configure(self.shortcuts.clone()));
                                    }
                                }
                                Err(error) => self.show_notice(Notice::Error(error)),
                            }
                        }
                        if self.shortcuts.dock_enabled {
                            if let Ok(status) = self.dock_status.read() {
                                ui.label(text(status.as_str(), 11.0, MUTED));
                            }
                        }
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            let label = if self.shortcuts.compact {
                                format!("Afficher les {} Zaaps", self.entries.len())
                            } else {
                                "Revenir au mode compact".into()
                            };
                            if ui.button(label).clicked() {
                                self.toggle_compact(ctx);
                            }
                            if ui.small_button("Diagnostic").clicked() {
                                self.open_diagnostic();
                            }
                        });
                    });
            });
    }

    fn compact_footer(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("compact-footer")
            .frame(
                egui::Frame::none()
                    .fill(BAR)
                    .inner_margin(egui::Margin::symmetric(14.0, 9.0)),
            )
            .show(ctx, |ui| {
                let recent = self.toast_time.elapsed() < Duration::from_secs(5);
                let message = if recent {
                    self.toast.as_str()
                } else if self.shortcuts.automatic {
                    "Auto actif — en attente de la prochaine étape"
                } else {
                    "Auto suspendu — raccourcis disponibles"
                };
                ui.add(
                    egui::Label::new(text(
                        message,
                        11.0,
                        if recent && self.success { GREEN } else { MUTED },
                    ))
                    .truncate(),
                )
                .on_hover_text(message);
            });
    }

    fn switch_maximize(&mut self, ctx: &egui::Context) {
        self.maximized = !self.maximized;
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(self.maximized));
    }

    fn footer(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("footer")
            .frame(
                egui::Frame::none()
                    .fill(BG)
                    .inner_margin(egui::Margin::symmetric(14.0, 10.0)),
            )
            .show(ctx, |ui| {
                let wide = ui.available_width() >= 660.0;
                let copied_recently =
                    self.success && self.toast_time.elapsed() < Duration::from_secs(5);
                let status = if self.toast_time.elapsed() > Duration::from_secs(5) {
                    "Prêt à copier"
                } else {
                    &self.toast
                };
                let color = if copied_recently { GREEN } else { MUTED };
                let status_width = if wide { 200.0 } else { 0.0 };
                let search_width = if wide {
                    (ui.available_width() - status_width - 13.0).max(200.0)
                } else {
                    ui.available_width()
                };
                let search_icon = self.icons.search.id();
                let draw_search = |ui: &mut egui::Ui, query: &mut String| {
                    egui::Frame::none()
                        .fill(CARD)
                        .rounding(7.0)
                        .stroke(Stroke::new(1.0_f32, BORDER))
                        .inner_margin(egui::Margin::symmetric(11.0, 8.0))
                        .show(ui, |ui| {
                            ui.set_width(search_width - 23.0);
                            ui.horizontal(|ui| {
                                ui.add(egui::Image::new((search_icon, Vec2::splat(16.0))));
                                ui.add(
                                    egui::TextEdit::singleline(query)
                                        .hint_text("Rechercher un zaap…")
                                        .desired_width((search_width - 79.0).max(115.0))
                                        .frame(false),
                                );
                                if !query.is_empty() && ui.small_button("×").clicked() {
                                    query.clear();
                                }
                            });
                        });
                };
                if wide {
                    ui.horizontal(|ui| {
                        draw_search(ui, &mut self.query);
                        ui.add_space(10.0);
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                if copied_recently {
                                    ui.add(egui::Image::new((
                                        self.icons.check.id(),
                                        Vec2::splat(13.0),
                                    )));
                                }
                                ui.label(text(
                                    if copied_recently {
                                        "Commande copiée"
                                    } else {
                                        concat!("Zaaps Dofus 3 · ", env!("CARGO_PKG_VERSION"))
                                    },
                                    11.0,
                                    color,
                                ));
                            });
                            ui.add(egui::Label::new(text(status, 10.5, MUTED)).truncate());
                        });
                    });
                } else {
                    draw_search(ui, &mut self.query);
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if copied_recently {
                            ui.add(egui::Image::new((self.icons.check.id(), Vec2::splat(12.0))));
                        }
                        ui.label(text(
                            if copied_recently {
                                "Commande copiée"
                            } else {
                                concat!("Zaaps Dofus 3 · ", env!("CARGO_PKG_VERSION"))
                            },
                            10.5,
                            color,
                        ));
                        ui.add(egui::Label::new(text(status, 10.0, MUTED)).truncate());
                    });
                }
            });
    }

    fn trim_title(value: &str, max_width: f32, font_size: f32) -> String {
        let capacity = ((max_width / (font_size * 0.54)).floor() as usize).max(3);
        if value.chars().count() <= capacity {
            value.to_owned()
        } else {
            value
                .chars()
                .take(capacity.saturating_sub(1))
                .collect::<String>()
                + "…"
        }
    }

    fn zaap_card(ui: &mut egui::Ui, item: &Zaap, icons: &Icons) -> bool {
        let width = ui.available_width().max(180.0);
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(width, CARD_HEIGHT), Sense::click());
        let painter = ui.painter_at(rect);
        painter.rect(
            rect,
            Rounding::same(7.0),
            if response.hovered() { CARD_HOVER } else { CARD },
            Stroke::new(1.0_f32, BORDER),
        );

        let portal_rect = Rect::from_center_size(
            Pos2::new(rect.min.x + 21.5, rect.center().y),
            Vec2::new(33.0, 37.0),
        );
        painter.image(
            icons.portal.id(),
            portal_rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            WHITE,
        );
        let right_start = rect.max.x - 86.0;
        let max_text_width = (right_start - rect.min.x - 55.0).max(48.0);
        // Show the region inline where it fits, as in the original Windows edition.
        // On narrower cards, keep two lines rather than allow overlap with coordinates.
        let region_label = format!("({})", item.region);
        let name_width = painter
            .layout_no_wrap(item.name.clone(), FontId::proportional(12.5), WHITE)
            .size()
            .x;
        let region_width = painter
            .layout_no_wrap(region_label.clone(), FontId::proportional(10.0), MUTED)
            .size()
            .x;
        if name_width + region_width + 7.0 <= max_text_width {
            painter.text(
                Pos2::new(rect.min.x + 49.0, rect.center().y),
                Align2::LEFT_CENTER,
                &item.name,
                FontId::proportional(12.5),
                WHITE,
            );
            painter.text(
                Pos2::new(rect.min.x + 49.0 + name_width + 7.0, rect.center().y),
                Align2::LEFT_CENTER,
                region_label,
                FontId::proportional(10.0),
                MUTED,
            );
        } else {
            painter.text(
                Pos2::new(rect.min.x + 49.0, rect.min.y + 17.0),
                Align2::LEFT_CENTER,
                Self::trim_title(&item.name, max_text_width, 12.5),
                FontId::proportional(12.5),
                WHITE,
            );
            painter.text(
                Pos2::new(rect.min.x + 49.0, rect.min.y + 33.0),
                Align2::LEFT_CENTER,
                Self::trim_title(&region_label, max_text_width, 10.0),
                FontId::proportional(10.0),
                MUTED,
            );
        }
        painter.text(
            Pos2::new(rect.max.x - 46.0, rect.center().y),
            Align2::RIGHT_CENTER,
            format!("{} {}", item.x, item.y),
            FontId::monospace(11.0),
            WHITE,
        );
        let copy_rect = Rect::from_center_size(
            Pos2::new(rect.max.x - 17.0, rect.center().y),
            Vec2::new(27.0, 31.0),
        );
        painter.rect(
            copy_rect,
            Rounding::same(5.0),
            if response.hovered() {
                CARD_HOVER
            } else {
                Color32::from_rgb(30, 51, 63)
            },
            Stroke::new(1.0_f32, BORDER),
        );
        let copy_symbol = Rect::from_center_size(copy_rect.center(), Vec2::new(14.0, 18.0));
        painter.image(
            icons.copy.id(),
            copy_symbol,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            WHITE,
        );
        response
            .on_hover_text(format!(
                "{} ({}, {}) — cliquer pour copier",
                item.name, item.x, item.y
            ))
            .clicked()
    }

    fn directory(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(BG)
                    .inner_margin(egui::Margin::symmetric(13.0, 10.0)),
            )
            .show(ctx, |ui| {
                let query = data::normalize(self.query.trim());
                let filtered: Vec<_> = self
                    .entries
                    .iter()
                    .filter(|z| {
                        query.is_empty()
                            || data::normalize(&z.name).contains(&query)
                            || data::normalize(&z.region).contains(&query)
                            || format!("{},{}", z.x, z.y).contains(&query)
                    })
                    .collect();
                if filtered.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(32.0);
                        ui.label(text("Aucun zaap trouvé", 15.0, MUTED));
                    });
                    return;
                }
                let width = ui.available_width();
                let count =
                    (((width + 7.0) / (MIN_COLUMN_WIDTH + 7.0)).floor() as usize).clamp(1, 4);
                let rows = filtered.len().div_ceil(count);
                let mut clicked: Option<String> = None;
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.columns(count, |columns| {
                            for (column_index, col_ui) in columns.iter_mut().enumerate() {
                                col_ui.spacing_mut().item_spacing.y = 0.0;
                                for zaap in filtered.iter().skip(column_index * rows).take(rows) {
                                    if Self::zaap_card(col_ui, zaap, &self.icons) {
                                        clicked = Some(zaap.command());
                                    }
                                    col_ui.add_space(CARD_GAP);
                                }
                            }
                        });
                    });
                if let Some(command) = clicked {
                    self.copy(command);
                }
            });
    }

    /// Enables native drag-resizing from every edge and corner of the custom titlebar window.
    fn resize_grips(&self, ctx: &egui::Context) {
        if self.maximized {
            return;
        }
        let bounds = ctx.screen_rect();
        if bounds.width() < 200.0 || bounds.height() < 200.0 {
            return;
        }
        let edge = 5.0;
        let corner = 13.0;
        let grip =
            |id: &'static str, rect: Rect, direction: ResizeDirection, cursor: CursorIcon| {
                egui::Area::new(egui::Id::new(id))
                    .fixed_pos(rect.min)
                    .order(egui::Order::Foreground)
                    .interactable(true)
                    .show(ctx, |ui| {
                        let (_hit, response) =
                            ui.allocate_exact_size(rect.size(), Sense::click_and_drag());
                        if response.hovered() {
                            ui.output_mut(|out| out.cursor_icon = cursor);
                        }
                        if response.is_pointer_button_down_on() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
                        }
                    });
            };
        let b = bounds;
        grip(
            "resize-north",
            Rect::from_min_size(
                egui::pos2(b.left() + corner, b.top()),
                egui::vec2(b.width() - 2.0 * corner, edge),
            ),
            ResizeDirection::North,
            CursorIcon::ResizeVertical,
        );
        grip(
            "resize-south",
            Rect::from_min_size(
                egui::pos2(b.left() + corner, b.bottom() - edge),
                egui::vec2(b.width() - 2.0 * corner, edge),
            ),
            ResizeDirection::South,
            CursorIcon::ResizeVertical,
        );
        if self.shortcuts.dock_enabled
            && matches!(
                self.shortcuts.dock_side,
                docking::Side::Top | docking::Side::Bottom
            )
        {
            return;
        }
        grip(
            "resize-west",
            Rect::from_min_size(
                egui::pos2(b.left(), b.top() + corner),
                egui::vec2(edge, b.height() - 2.0 * corner),
            ),
            ResizeDirection::West,
            CursorIcon::ResizeHorizontal,
        );
        grip(
            "resize-east",
            Rect::from_min_size(
                egui::pos2(b.right() - edge, b.top() + corner),
                egui::vec2(edge, b.height() - 2.0 * corner),
            ),
            ResizeDirection::East,
            CursorIcon::ResizeHorizontal,
        );
        grip(
            "resize-nw",
            Rect::from_min_size(b.min, Vec2::splat(corner)),
            ResizeDirection::NorthWest,
            CursorIcon::ResizeNwSe,
        );
        grip(
            "resize-ne",
            Rect::from_min_size(egui::pos2(b.right() - corner, b.top()), Vec2::splat(corner)),
            ResizeDirection::NorthEast,
            CursorIcon::ResizeNeSw,
        );
        grip(
            "resize-sw",
            Rect::from_min_size(
                egui::pos2(b.left(), b.bottom() - corner),
                Vec2::splat(corner),
            ),
            ResizeDirection::SouthWest,
            CursorIcon::ResizeNeSw,
        );
        grip(
            "resize-se",
            Rect::from_min_size(
                egui::pos2(b.right() - corner, b.bottom() - corner),
                Vec2::splat(corner),
            ),
            ResizeDirection::SouthEast,
            CursorIcon::ResizeNwSe,
        );
    }
}

impl eframe::App for ZaapsApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        if self.initial_visibility_pending {
            self.initial_visibility_pending = false;
            self.hide(ctx);
        }
        while let Ok(notice) = self.notices.try_recv() {
            if let Notice::WindowVisibility(visible) = notice {
                self.hidden = !visible;
                if !visible {
                    self.manager_open = false;
                    self.diagnostic_open = false;
                    if self.settings_open {
                        self.close_settings();
                    }
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(visible));
            } else {
                self.show_notice(notice);
            }
        }
        while let Some(event) = self
            .tray_events
            .as_ref()
            .and_then(|events| events.try_recv().ok())
        {
            match event {
                tray::Event::Show => self.show(ctx),
                tray::Event::ToggleAutomatic => self.toggle_automatic(),
                tray::Event::CopyRoute => self.copy_action(Action::Combined),
                tray::Event::Quit => self.quit(ctx),
            }
        }
        while let Ok(notice) = self.update_notices.try_recv() {
            if !self.shortcuts.check_updates && !self.manual_update_check {
                continue;
            }
            self.manual_update_check = false;
            match notice {
                updates::ResultNotice::Available(release) => {
                    self.update_status = format!("Version {} disponible", release.version);
                    self.latest_release = Some(release);
                }
                updates::ResultNotice::Current => {
                    self.update_status = "Aucune version plus récente disponible".into()
                }
                updates::ResultNotice::Failed => {
                    self.update_status = "Vérification indisponible — réessayez plus tard".into()
                }
            }
        }
        if self.hidden && !self.tray_available() {
            self.show(ctx);
        }
        if self.exiting {
            return;
        }
        if let Some(tray) = &self.tray {
            // Tooltip changes on state transitions; copy notices have their own tooltip.
            if self.toast_time.elapsed() < Duration::from_millis(250)
                && self.toast.starts_with("Auto")
            {
                tray.tip(format!("Zaaps Dofus 3 — {}", self.toast));
            }
        }
        self.sync_slim_layout(ctx);
        self.heading(ctx);
        if self.shortcuts.dock_enabled {
            // The attached companion has no footer or directory below its route.
        } else if self.shortcuts.compact {
            self.compact_footer(ctx);
        } else {
            self.footer(ctx);
        }
        self.route_panel(ctx);
        if self.shortcuts.compact {
            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(BG))
                .show(ctx, |_| {});
        } else {
            self.directory(ctx);
        }
        if self.shortcuts.dock_enabled {
            self.docked_windows(ctx);
        } else {
            self.settings(ctx);
            self.diagnostic(ctx);
            if self.manager_open {
                self.docked_windows(ctx);
            }
        }
        self.resize_grips(ctx);
        self.window_corners.update(ctx.pixels_per_point());
        ctx.request_repaint_after(Duration::from_millis(200));
    }
}

impl Drop for ZaapsApp {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(tray) = &self.tray {
            tray.shutdown();
        }
    }
}

fn apply_window_level(ctx: &egui::Context, prefs: &Shortcuts) {
    // Les réglages peuvent être ouverts dans un viewport distinct de la fenêtre principale.
    ctx.send_viewport_cmd_to(
        egui::ViewportId::ROOT,
        egui::ViewportCommand::WindowLevel(prefs.window_level()),
    );
    ctx.request_repaint_of(egui::ViewportId::ROOT);
}

#[cfg(test)]
mod compact_tests {
    use super::*;

    #[test]
    fn foreground_setting_targets_root_and_can_be_disabled_without_creating_windows() {
        let ctx = egui::Context::default();
        for (enabled, expected) in [
            (true, egui::WindowLevel::AlwaysOnTop),
            (false, egui::WindowLevel::Normal),
        ] {
            let output = ctx.run(egui::RawInput::default(), |ctx| {
                apply_window_level(
                    ctx,
                    &Shortcuts {
                        always_on_top: enabled,
                        ..Shortcuts::default()
                    },
                );
            });
            assert_eq!(
                output.viewport_output.len(),
                1,
                "Aucune fenêtre supplémentaire ne doit être créée"
            );
            assert!(output.viewport_output[&egui::ViewportId::ROOT].commands.iter().any(|command|
                matches!(command, egui::ViewportCommand::WindowLevel(level) if *level == expected)
            ));
        }
    }

    #[test]
    fn attached_route_and_directory_button_fit_in_225_pixels() {
        for width in [320.0, 414.0, 460.0] {
            let ctx = egui::Context::default();
            let mut companion = None;
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 225.0))),
                    ..Default::default()
                },
                |ctx| {
                    let prefs = Shortcuts {
                        dock_enabled: true,
                        ..Shortcuts::default()
                    };
                    let mut app = ZaapsApp {
                        entries: data::embedded().unwrap(),
                        query: String::new(),
                        draft: prefs.clone(),
                        shortcuts: prefs,
                        settings_open: false,
                        recording: None,
                        toast: String::new(),
                        success: false,
                        toast_time: Instant::now(),
                        notices: mpsc::channel().1,
                        hotkey_settings: mpsc::channel().0,
                        stop: Arc::new(AtomicBool::new(true)),
                        maximized: false,
                        icons: Icons::new(ctx),
                        window_corners: WindowCorners::default(),
                        cache: Arc::new(RwLock::new(Step::from_text(
                            "697/774\nEnsuite allez en [-14,37]",
                            &data::embedded().unwrap(),
                        ))),
                        tray: None,
                        tray_events: None,
                        update_requests: mpsc::channel().0,
                        update_notices: mpsc::channel().1,
                        latest_release: None,
                        update_status: String::new(),
                        manual_update_check: false,
                        hidden: false,
                        exiting: false,
                        initial_visibility_pending: false,
                        clipboard_replaced: false,
                        diagnostic_open: false,
                        diagnostic_text: String::new(),
                        compact_size: Vec2::new(460.0, 360.0),
                        directory_size: Vec2::new(960.0, 750.0),
                        dock_config: mpsc::channel().0,
                        dock_status: Arc::new(RwLock::new("Fenêtres accrochées".into())),
                        slim_layout: true,
                        manager_open: false,
                    };
                    app.heading(ctx);
                    app.route_panel(ctx);
                    companion = Some(app);
                },
            );
            let labels: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::epaint::Shape::Text(t) => Some((
                        t.galley.job.text.as_str(),
                        t.pos.y + t.galley.rect.max.y,
                        t.pos.x + t.galley.rect.max.x,
                    )),
                    _ => None,
                })
                .collect();
            for label in ["Zaap", "Travel", "Trajet complet", "Afficher Zaaps"] {
                let bottom = labels
                    .iter()
                    .find(|(text, _, _)| *text == label)
                    .unwrap_or_else(|| panic!("Missing {label}: {labels:?}"))
                    .1;
                assert!(
                    bottom <= 225.0,
                    "{label} extends below the compact window: {bottom}"
                );
                let right = labels.iter().find(|(text, _, _)| *text == label).unwrap().2;
                assert!(
                    right <= width,
                    "{label} extends beyond width {width}: {right}"
                );
            }
            assert!(!labels
                .iter()
                .any(|(label, _, _)| label.contains("Accrocher à Ganymède")
                    || *label == "Diagnostic"));
            drop(companion);
        }
    }
}
