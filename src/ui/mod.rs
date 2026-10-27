//! Presentation components and view-only state. No filesystem or PDF operations.
pub mod chrome;
pub mod dialog;
pub mod explorer;
pub mod icons;
pub mod style;
pub mod tabs;
pub mod toolbar;

use crate::document::DocumentKind;

pub enum Action {
    #[cfg(not(target_arch = "wasm32"))]
    Workspace,
    #[cfg(not(target_arch = "wasm32"))]
    Refresh,
    New(DocumentKind, String),
    Open(String),
    #[cfg(target_arch = "wasm32")]
    Import,
    #[cfg(target_arch = "wasm32")]
    Export,
}

pub fn new_choices(ui: &mut egui::Ui, folder: &str, action: &mut Option<Action>) {
    for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
        ui.horizontal(|ui| {
            icons::show(ui, kind.into(), style::palette(ui).accent);
            if ui.button(format!("New {}", kind.label())).clicked() {
                *action = Some(Action::New(kind, folder.into()));
                ui.close();
            }
        });
    }
}
