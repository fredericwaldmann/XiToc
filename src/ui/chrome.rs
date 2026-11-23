use super::{
    Action, new_choices,
    style::{self, Theme},
};
use egui::{Align, Layout, RichText};

#[derive(Default)]
pub struct Chrome {
    details: bool,
}

impl Chrome {
    pub fn bottom(&mut self, ui: &mut egui::Ui, status: &str, context: &str) {
        let p = style::palette(ui);
        egui::Panel::bottom("status-bar")
            .resizable(false)
            .exact_size(28.0)
            .frame(style::frame(p.shell, 10, 1))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .small_button("…")
                        .on_hover_text("Show full status message")
                        .clicked()
                    {
                        self.details = !self.details;
                    }
                    let right = if ui.available_width() > 560.0 {
                        170.0
                    } else {
                        0.0
                    };
                    let width = (ui.available_width() - right).max(20.0);
                    ui.add_sized(
                        [width, 22.0],
                        egui::Label::new(RichText::new(status).small().color(p.muted)).truncate(),
                    )
                    .on_hover_text(status);
                    if right > 0.0 {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(RichText::new(context).small().color(p.muted));
                        });
                    }
                });
            });
        if self.details {
            egui::Window::new("Status")
                .open(&mut self.details)
                .resizable(true)
                .default_width(360.0)
                .show(ui.ctx(), |ui| {
                    ui.label(status);
                });
        }
    }
}

pub fn empty(ui: &mut egui::Ui, can_create: bool, folder: &str, action: &mut Option<Action>) {
    let p = style::palette(ui);
    let height = ui.available_height();
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.add_space((height * 0.25).clamp(12.0, 140.0));
        ui.vertical_centered(|ui| {
            ui.add(egui::Label::new(RichText::new("No document open").size(20.0).strong()).wrap());
            ui.add_space(4.0);
            ui.add(
                egui::Label::new(
                    RichText::new("Create a Page Note or Whiteboard, or open an existing PDF.")
                        .color(p.muted),
                )
                .wrap(),
            );
            ui.add_space(16.0);
            if can_create {
                ui.menu_button("New Document", |ui| new_choices(ui, folder, action));
            }
            #[cfg(not(target_arch = "wasm32"))]
            if !can_create && ui.button("Open workspace…").clicked() {
                *action = Some(Action::Workspace);
            }
            #[cfg(target_arch = "wasm32")]
            if ui.button("Import PDF…").clicked() {
                *action = Some(Action::Import);
            }
            ui.add_space(12.0);
            ui.add(
                egui::Label::new(
                    RichText::new(if cfg!(target_arch = "wasm32") {
                        "Export PDFs to keep your work. Reimport them after reloading."
                    } else {
                        "Choose a folder to open or create your PDF documents."
                    })
                    .small()
                    .color(p.muted),
                )
                .wrap(),
            );
        });
    });
}

pub fn identity(ui: &mut egui::Ui) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let id = egui::Id::unique("window-identity-icon");
        let icon = ui
            .ctx()
            .data(|data| data.get_temp::<egui::TextureHandle>(id));
        let icon = icon.unwrap_or_else(|| {
            let icon = crate::native_icon();
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [icon.width as usize, icon.height as usize],
                &icon.rgba,
            );
            let icon = ui
                .ctx()
                .load_texture("XiToc identity", image, egui::TextureOptions::LINEAR);
            ui.ctx().data_mut(|data| data.insert_temp(id, icon.clone()));
            icon
        });
        ui.image((icon.id(), egui::Vec2::splat(18.)));
    }
    ui.label(RichText::new("XiToc").size(16.).strong());
}

pub fn menu(
    ui: &mut egui::Ui,
    prefs: &mut crate::settings::Preferences,
    action: &mut Option<Action>,
    selection: super::selection_controls::MenuState,
    can_create: bool,
    folder: &str,
) {
    let (_, rect) = ui.allocate_space(egui::Vec2::splat(style::CONTROL));
    let button = ui.interact(
        rect,
        egui::Id::unique("main-application-menu"),
        egui::Sense::click(),
    );
    button.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), "Main menu")
    });
    let visuals = ui.style().interact(&button);
    if button.hovered() || button.has_focus() {
        ui.painter().rect(
            rect,
            4.,
            visuals.bg_fill,
            visuals.bg_stroke,
            egui::StrokeKind::Inside,
        );
    }
    let center = rect.center();
    for y in [-4., 0., 4.] {
        ui.painter().line_segment(
            [center + egui::vec2(-6., y), center + egui::vec2(6., y)],
            visuals.fg_stroke,
        );
    }
    let button = button.on_hover_text("Main menu");
    egui::Popup::menu(&button).show(|ui| {
        ui.add_enabled_ui(can_create, |ui| {
            ui.menu_button("New Document", |ui| new_choices(ui, folder, action));
        });
        #[cfg(not(target_arch = "wasm32"))]
        {
            if ui.button("Open Workspace…").clicked() {
                *action = Some(Action::Workspace);
                ui.close();
            }
            if ui.button("Refresh Explorer").clicked() {
                *action = Some(Action::Refresh);
                ui.close();
            }
        }
        #[cfg(target_arch = "wasm32")]
        if ui.button("Import PDF…").clicked() {
            *action = Some(Action::Import);
            ui.close();
        }
        ui.menu_button("Appearance", |ui| {
            ui.selectable_value(&mut prefs.theme, Theme::System, "System");
            ui.selectable_value(&mut prefs.theme, Theme::Light, "Light");
            ui.selectable_value(&mut prefs.theme, Theme::Dark, "Dark");
        });
        if ui.button("Settings…").clicked() {
            *action = Some(Action::Settings);
            ui.close();
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let fullscreen = ui.input(|input| input.viewport().fullscreen.unwrap_or(false));
            if ui
                .button(if fullscreen {
                    "Exit Full Screen"
                } else {
                    "Full Screen"
                })
                .clicked()
            {
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
                ui.close();
            }
        }
        ui.separator();
        ui.menu_button("Edit", |ui| {
            egui::ScrollArea::vertical()
                .max_height((ui.ctx().content_rect().height() - 120.).max(96.))
                .show(ui, |ui| {
                    if let Some(command) = super::selection_controls::menu(ui, selection) {
                        *action = Some(Action::Selection(command));
                        ui.close();
                    }
                });
        });
    });
}

pub fn window_buttons(_ui: &mut egui::Ui) {
    #[cfg(target_os = "linux")]
    {
        use egui::ViewportCommand as Command;
        let ui = _ui;
        let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
        for (label, command) in [
            ("Close window", Command::Close),
            (
                if maximized {
                    "Restore window"
                } else {
                    "Maximize window"
                },
                Command::Maximized(!maximized),
            ),
            ("Minimize window", Command::Minimized(true)),
        ] {
            let (_, response) =
                ui.allocate_exact_size(egui::Vec2::splat(28.), egui::Sense::click());
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label)
            });
            let center = response.rect.center();
            let color = ui.style().interact(&response).fg_stroke.color;
            let painter = ui.painter();
            let visuals = ui.style().interact(&response);
            if response.hovered() || response.is_pointer_button_down_on() {
                painter.circle_filled(center, 11., visuals.weak_bg_fill);
            }
            if response.has_focus() {
                painter.circle_stroke(center, 11., visuals.bg_stroke);
            }
            match command {
                Command::Close => {
                    super::icons::paint(painter, center, super::icons::Icon::Close, color)
                }
                Command::Minimized(_) => {
                    super::icons::paint(painter, center, super::icons::Icon::Minus, color)
                }
                _ => {
                    let stroke = egui::Stroke::new(1.25, color);
                    let square = egui::Rect::from_center_size(center, egui::Vec2::splat(10.));
                    if maximized {
                        painter.rect_stroke(
                            square.translate(egui::vec2(2., -2.)),
                            1.,
                            stroke,
                            egui::StrokeKind::Inside,
                        );
                        painter.rect_filled(
                            square,
                            1.,
                            if response.hovered() {
                                visuals.weak_bg_fill
                            } else {
                                style::palette(ui).shell
                            },
                        );
                    }
                    painter.rect_stroke(square, 1., stroke, egui::StrokeKind::Inside);
                }
            }
            if response.on_hover_text(label).clicked() {
                ui.ctx().send_viewport_cmd(command);
            }
        }
    }
}

pub fn drag_space(_ui: &mut egui::Ui, _width: f32) {
    #[cfg(target_os = "linux")]
    {
        let ui = _ui;
        let (_, drag) =
            ui.allocate_exact_size(egui::vec2(_width, 28.), egui::Sense::click_and_drag());
        if drag.double_clicked() {
            let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
        } else if drag.drag_started() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
    }
}

#[cfg(target_os = "linux")]
pub fn resize_edges(_ui: &egui::Ui) {
    #[cfg(target_os = "linux")]
    {
        use egui::{CursorIcon as Cursor, ResizeDirection as Edge};
        let ui = _ui;
        let area = ui.ctx().content_rect();
        let direction = ui.input(|input| {
            let viewport = input.viewport();
            if viewport.maximized.unwrap_or(false) || viewport.fullscreen.unwrap_or(false) {
                return None;
            }
            let p = input.pointer.hover_pos()?;
            let edge = 5.;
            let x = if p.x - area.left() < edge {
                -1
            } else if area.right() - p.x < edge {
                1
            } else {
                0
            };
            let y = if p.y - area.top() < edge {
                -1
            } else if area.bottom() - p.y < edge {
                1
            } else {
                0
            };
            let (edge, cursor) = match (x, y) {
                (-1, -1) => (Edge::NorthWest, Cursor::ResizeNwSe),
                (1, 1) => (Edge::SouthEast, Cursor::ResizeNwSe),
                (1, -1) => (Edge::NorthEast, Cursor::ResizeNeSw),
                (-1, 1) => (Edge::SouthWest, Cursor::ResizeNeSw),
                (-1, _) => (Edge::West, Cursor::ResizeHorizontal),
                (1, _) => (Edge::East, Cursor::ResizeHorizontal),
                (_, -1) => (Edge::North, Cursor::ResizeVertical),
                (_, 1) => (Edge::South, Cursor::ResizeVertical),
                _ => return None,
            };
            Some((edge, cursor, input.pointer.primary_pressed()))
        });
        if let Some((edge, cursor, pressed)) = direction {
            ui.ctx().set_cursor_icon(cursor);
            if pressed {
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::BeginResize(edge));
            }
        }
    }
}
