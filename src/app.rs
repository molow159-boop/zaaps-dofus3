use crate::{
    commands::Step,
    data::{self, Zaap},
    ganymede,
    hotkeys::{self, Control, Notice, Shortcuts},
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
        let shortcuts = Shortcuts::load();
        let (tx, notices) = mpsc::channel();
        ganymede::start(Arc::clone(&cache), directory.clone(), Arc::clone(&stop));
        let hotkey_settings = hotkeys::start(
            cache,
            shortcuts.clone(),
            tx,
            Arc::clone(&stop),
            cc.egui_ctx.clone(),
        );
        Self {
            entries: directory,
            query: String::new(),
            draft: shortcuts.clone(),
            shortcuts,
            settings_open: false,
            recording: None,
            toast: data_error.unwrap_or_else(|| "Prêt à copier".into()),
            success: false,
            toast_time: Instant::now(),
            notices,
            hotkey_settings,
            stop,
            maximized: false,
            icons: Icons::new(&cc.egui_ctx),
            window_corners: WindowCorners::default(),
        }
    }

    fn show_notice(&mut self, notice: Notice) {
        match notice {
            Notice::Copied(command) => {
                self.toast = format!("Commande copiée !  {command}");
                self.success = true;
            }
            Notice::Info(message) => {
                self.toast = message;
                self.success = true;
            }
            Notice::Error(message) => {
                self.toast = message;
                self.success = false;
            }
        }
        self.toast_time = Instant::now();
    }

    fn copy(&mut self, command: String) {
        self.show_notice(hotkeys::copy(&command));
    }

    fn toggle_automatic(&mut self) {
        self.shortcuts.automatic = !self.shortcuts.automatic;
        match self.shortcuts.save() {
            Ok(()) => {
                self.draft.automatic = self.shortcuts.automatic;
                let _ = self
                    .hotkey_settings
                    .send(Control::Automatic(self.shortcuts.automatic));
                let message = if self.shortcuts.automatic {
                    "Mode automatique activé — copie à la prochaine étape"
                } else {
                    "Mode automatique désactivé"
                };
                self.show_notice(Notice::Info(message.into()));
            }
            Err(error) => {
                self.shortcuts.automatic = !self.shortcuts.automatic;
                self.show_notice(Notice::Error(format!("Réglages non enregistrés : {error}")));
            }
        }
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

    fn save_settings(&mut self) {
        match self.draft.save() {
            Ok(()) => {
                self.shortcuts = self.draft.clone();
                self.close_settings();
                self.show_notice(Notice::Info("Raccourcis enregistrés".into()));
            }
            Err(error) => self.show_notice(Notice::Error(error)),
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
                _ => self.draft.combined = key,
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
        egui::Window::new("Raccourcis Ganymède")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(400.0)
            .max_width(480.0)
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
                        self.save_settings();
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
                    .inner_margin(egui::Margin::symmetric(13.0, 6.0)),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.horizontal(|ui| {
                    ui.add(egui::Image::new((self.icons.brand.id(), Vec2::splat(32.0))));
                    ui.add_space(3.0);
                    ui.label(text("Zaaps Dofus 3", 19.0, WHITE).strong());
                    let drag_space = (ui.available_width() - 244.0).max(4.0);
                    let drag = ui.add_sized(
                        [drag_space, 34.0],
                        egui::Label::new("").sense(Sense::click_and_drag()),
                    );
                    if drag.drag_started() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }
                    if drag.double_clicked() {
                        self.switch_maximize(ctx);
                    }
                    let auto_label = if self.shortcuts.automatic {
                        "Auto : ON"
                    } else {
                        "Auto : OFF"
                    };
                    let auto_color = if self.shortcuts.automatic {
                        Color32::from_rgb(39, 111, 69)
                    } else {
                        CARD
                    };
                    if ui
                        .add_sized(
                            [96.0, 30.0],
                            egui::Button::new(text(auto_label, 12.0, WHITE))
                                .fill(auto_color)
                                .rounding(7.0),
                        )
                        .on_hover_text(
                            "Copier le trajet disponible à chaque nouvelle étape Ganymède",
                        )
                        .clicked()
                    {
                        self.toggle_automatic();
                    }
                    if image_button(ui, &self.icons.gear, "Configurer les raccourcis").clicked() {
                        self.begin_settings();
                    }
                    if title_button(ui, "−", "Réduire").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    }
                    if title_button(ui, "□", "Agrandir ou restaurer").clicked() {
                        self.switch_maximize(ctx);
                    }
                    if title_button(ui, "×", "Fermer").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.horizontal(|ui| {
                    ui.add_space(43.0);
                    ui.label(text(
                        "Clique sur un zaap pour copier la commande",
                        11.0,
                        MUTED,
                    ));
                });
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
                let status = if self.toast_time.elapsed() > Duration::from_secs(5) {
                    "Prêt à copier"
                } else {
                    &self.toast
                };
                let color = if self.success { GREEN } else { MUTED };
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
                                if self.success {
                                    ui.add(egui::Image::new((
                                        self.icons.check.id(),
                                        Vec2::splat(13.0),
                                    )));
                                }
                                ui.label(text(
                                    if self.success {
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
                        if self.success {
                            ui.add(egui::Image::new((self.icons.check.id(), Vec2::splat(12.0))));
                        }
                        ui.label(text(
                            if self.success {
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
        while let Ok(notice) = self.notices.try_recv() {
            self.show_notice(notice);
        }
        self.heading(ctx);
        self.footer(ctx);
        self.directory(ctx);
        self.settings(ctx);
        self.resize_grips(ctx);
        self.window_corners.update(ctx.pixels_per_point());
        ctx.request_repaint_after(Duration::from_millis(200));
    }
}

impl Drop for ZaapsApp {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
