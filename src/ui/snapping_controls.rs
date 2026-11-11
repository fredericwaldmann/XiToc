//! Shared compact session controls and persisted creation defaults.
use crate::snapping::{Sensitivity, SnapSettings};
pub fn options(ui: &mut egui::Ui, settings: &mut SnapSettings) {
    ui.checkbox(&mut settings.objects, "Snap to Objects");
    ui.checkbox(&mut settings.guides, "Show Alignment Guides");
    ui.checkbox(&mut settings.grid, "Snap to Grid");
    ui.checkbox(&mut settings.show_grid, "Show Grid");
}
pub fn details(ui: &mut egui::Ui, settings: &mut SnapSettings) {
    egui::ComboBox::from_id_salt("snap-sensitivity")
        .selected_text(settings.sensitivity.label())
        .show_ui(ui, |ui| {
            for value in [Sensitivity::Low, Sensitivity::Normal, Sensitivity::High] {
                ui.selectable_value(&mut settings.sensitivity, value, value.label());
            }
        });
    ui.horizontal(|ui| {
        ui.label("Grid spacing (document pt)");
        ui.add(
            egui::DragValue::new(&mut settings.spacing)
                .range(1.0..=4096.0)
                .speed(1.),
        );
    });
    ui.weak("Hold Alt during a drag to bypass snapping. Alt-click still removes selection.");
}
pub fn menu(ui: &mut egui::Ui, settings: &mut SnapSettings) {
    options(ui, settings);
    ui.menu_button("Snapping Settings", |ui| details(ui, settings));
}
