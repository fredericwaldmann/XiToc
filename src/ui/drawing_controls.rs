//! Shared ink controls for per-tab tools and application defaults.
pub fn show(ui: &mut egui::Ui, rgba: &mut [u8; 4], width: &mut f64, radius: &mut f64) {
    pen(ui, rgba, width);
    eraser_radius(ui, radius);
}

pub fn pen(ui: &mut egui::Ui, rgba: &mut [u8; 4], width: &mut f64) {
    let mut color = egui::Color32::from_rgb((*rgba)[0], (*rgba)[1], (*rgba)[2]);
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 3.0, color);
        ui.label("Pen color");
    });
    // Inline expansion avoids opening a second root popup from a menu.
    // egui's color-button popup would replace the settings popup.
    egui::CollapsingHeader::new("Edit color").show(ui, |ui| {
        ui.spacing_mut().slider_width = 180.0_f32.min(ui.available_width() - 40.0);
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
}

pub fn eraser_radius(ui: &mut egui::Ui, radius: &mut f64) {
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

pub fn response(ui: &mut egui::Ui, prefs: &mut crate::settings::DrawingPreferences) {
    ui.strong("Stroke Smoothing");
    ui.label("Applies to new strokes in every open tab.");
    ui.spacing_mut().slider_width =
        super::style::SMOOTHING_SLIDER_WIDTH.min(ui.available_width() - 70.0);
    ui.label("Strength");
    ui.scope(|ui| {
        let p = super::style::palette(ui);
        ui.visuals_mut().widgets.inactive.bg_fill = p.border;
        ui.add(
            egui::Slider::new(&mut prefs.smoothing_strength, 0..=100)
                .suffix("%")
                .trailing_fill(true),
        );
    });
    ui.weak("0% Off · 25% Light · 50% Medium · 75% Strong · 100% Maximum");
    ui.add_space(super::style::GAP);
    ui.strong("Pen pressure");
    ui.checkbox(&mut prefs.pressure.enabled, "Enable pressure sensitivity");
    ui.collapsing("Pressure options", |ui| {
        let p = super::style::palette(ui);
        ui.visuals_mut().widgets.inactive.bg_fill = p.border;
        ui.add_enabled_ui(prefs.pressure.enabled, |ui| {
            ui.label("Sensitivity strength");
            ui.add(
                egui::Slider::new(&mut prefs.pressure.strength, 0..=100)
                    .suffix("%")
                    .trailing_fill(true),
            );
            ui.label("Minimum stroke width");
            ui.add(
                egui::Slider::new(&mut prefs.pressure.minimum, 1..=100)
                    .suffix("%")
                    .trailing_fill(true),
            );
        });
    });
    ui.weak("Applies to new strokes with pen/touch force. Mouse and devices without pressure keep constant width. Native Linux/macOS tablets currently use this fallback.");
    ui.add_space(super::style::GAP);
}
