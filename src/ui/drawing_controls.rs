//! Shared ink controls for per-tab tools and application defaults.
pub fn show(ui: &mut egui::Ui, rgba: &mut [u8; 4], width: &mut f64, radius: &mut f64) {
    let mut color = egui::Color32::from_rgb((*rgba)[0], (*rgba)[1], (*rgba)[2]);
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 3.0, color);
        ui.label("Pen color");
    });
    // Inline expansion avoids opening a second root popup from a menu.
    // egui's color-button popup would replace the settings popup.
    egui::CollapsingHeader::new("Edit color").show(ui, |ui| {
        ui.spacing_mut().slider_width = 210.0;
        if egui::color_picker::color_picker_color32(
            ui,
            &mut color,
            egui::color_picker::Alpha::Opaque,
        ) {
            (*rgba) = [color.r(), color.g(), color.b(), 255];
        }
    });
    ui.horizontal(|ui| {
        ui.label("Width");
        ui.add(
            egui::DragValue::new(&mut (*width))
                .range(0.25..=12.0)
                .speed(0.1)
                .suffix(" pt"),
        );
    });
    ui.horizontal(|ui| {
        ui.label("Eraser radius");
        ui.add(
            egui::DragValue::new(&mut (*radius))
                .range(4.0..=48.0)
                .speed(1.0)
                .suffix(" screen pt"),
        );
    });
}
