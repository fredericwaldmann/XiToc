use super::{
    Action,
    icons::{self, Icon},
    style,
};
use crate::{
    document::{Document, DocumentKind},
    editor::Editor,
};

pub fn show(
    ui: &mut egui::Ui,
    document: Option<(&Document, &mut Editor)>,
    action: &mut Option<Action>,
) {
    let p = style::palette(ui);
    let width = ui.available_width();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let Some((doc, editor)) = document else {
            ui.label(
                egui::RichText::new("Open a document to begin")
                    .small()
                    .color(p.muted),
            );
            return;
        };
        icons::show(ui, doc.data.kind().into(), p.accent);
        if width < 360.0 {
            ui.menu_button(doc.data.kind().label(), |ui| {
                view(ui, doc, editor);
                ui.separator();
                unavailable(ui);
                export(ui, action);
            });
            return;
        }
        ui.label(
            egui::RichText::new(doc.data.kind().label())
                .small()
                .color(p.muted),
        );
        ui.separator();
        if width >= 820.0 {
            unavailable(ui);
        } else {
            ui.menu_button("Tools", |ui| {
                ui.label("Not available yet");
                unavailable(ui);
            });
        }
        ui.separator();
        if width >= 600.0 {
            view(ui, doc, editor);
        } else {
            ui.menu_button("View", |ui| view(ui, doc, editor));
        }
        export(ui, action);
    });
}

fn unavailable(ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        for (name, milestone) in [
            ("Pen", "M1"),
            ("Eraser", "M1"),
            ("Select", "M6"),
            ("Text", "M6"),
            ("Shapes", "M6"),
            ("Undo", "M1"),
            ("Redo", "M1"),
        ] {
            ui.add_enabled(false, egui::Button::new(name).frame_when_inactive(false))
                .on_disabled_hover_text(format!("{name} is unavailable until {milestone}"));
        }
    });
}

fn view(ui: &mut egui::Ui, doc: &Document, editor: &mut Editor) {
    if doc.external {
        ui.label("Read-only · preview unavailable");
        return;
    }
    match doc.data.kind() {
        DocumentKind::Page => {
            ui.label(
                egui::RichText::new("A4 · Fit width")
                    .small()
                    .color(style::palette(ui).muted),
            );
        }
        DocumentKind::Whiteboard => {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                if icons::button(ui, Icon::Minus, "Zoom out", false).clicked() {
                    editor.zoom_by(0.8);
                }
                ui.add_sized(
                    [44.0, 26.0],
                    egui::Label::new(format!("{:.0}%", editor.zoom() * 100.0)),
                );
                if icons::button(ui, Icon::Plus, "Zoom in", false).clicked() {
                    editor.zoom_by(1.25);
                }
                if icons::button(ui, Icon::Reset, "Reset view", false).clicked() {
                    editor.reset_view();
                }
                if icons::button(
                    ui,
                    Icon::Grid,
                    "Toggle dot grid (session only)",
                    editor.grid_visible(),
                )
                .clicked()
                {
                    editor.toggle_grid();
                }
            });
        }
    }
}

fn export(_ui: &mut egui::Ui, _action: &mut Option<Action>) {
    #[cfg(target_arch = "wasm32")]
    if _ui.button("Export PDF…").clicked() {
        *_action = Some(Action::Export);
    }
}
