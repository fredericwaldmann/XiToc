//! Preset controls shared by the top toolbar and existing Settings dialog.
use super::style;
use crate::pen::{MAX_PRESETS, PenStyle, PenType, Presets};

pub fn pen(ui: &mut egui::Ui, style: &mut PenStyle) {
    egui::ComboBox::from_id_salt("pen-type")
        .selected_text(style.pen_type.label())
        .show_ui(ui, |ui| {
            for kind in PenType::ALL {
                ui.selectable_value(&mut style.pen_type, kind, kind.label());
            }
        });
    egui::CollapsingHeader::new("Color picker").show(ui, |ui| {
        ui.spacing_mut().slider_width = 180.0;
        let mut color = egui::Color32::from_rgb(style.color[0], style.color[1], style.color[2]);
        if egui::color_picker::color_picker_color32(
            ui,
            &mut color,
            egui::color_picker::Alpha::Opaque,
        ) {
            style.color = [color.r(), color.g(), color.b()];
        }
    });
    ui.horizontal(|ui| {
        ui.label("Thickness");
        ui.add(
            egui::DragValue::new(&mut style.width)
                .range(0.25..=24.0)
                .speed(0.1)
                .suffix(" pt"),
        );
    });
    ui.spacing_mut().slider_width =
        style::SMOOTHING_SLIDER_WIDTH.min((ui.available_width() - 70.0).max(60.0));
    ui.label("Opacity");
    percentage(ui, &mut style.opacity, 1);
    ui.label("Smoothing strength");
    percentage(ui, &mut style.smoothing_strength, 0);
    ui.weak("0% Off · 50% Medium · 100% Maximum");
    if style.pen_type == PenType::Pressure {
        ui.separator();
        ui.checkbox(&mut style.pressure.enabled, "Enable pressure sensitivity");
        ui.add_enabled_ui(style.pressure.enabled, |ui| {
            ui.label("Sensitivity strength");
            percentage(ui, &mut style.pressure.strength, 0);
            ui.label("Minimum stroke width");
            percentage(ui, &mut style.pressure.minimum, 1);
        });
        ui.weak("Mouse and devices without pressure use constant width. Native Linux/macOS tablets currently use this fallback.");
    }
}

pub fn styled_slider(ui: &mut egui::Ui, slider: egui::Slider<'_>) -> egui::Response {
    ui.scope(|ui| {
        ui.visuals_mut().widgets.inactive.bg_fill = super::style::palette(ui).border;
        ui.add(slider.trailing_fill(true))
    })
    .inner
}

fn percentage(ui: &mut egui::Ui, value: &mut u8, minimum: u8) {
    styled_slider(ui, egui::Slider::new(value, minimum..=100).suffix("%"));
}

pub fn preset(ui: &mut egui::Ui, presets: &mut Presets) {
    ui.strong("Pen preset");
    ui.label("Name");
    ui.add(
        egui::TextEdit::singleline(&mut presets.active_mut().name)
            .char_limit(64)
            .desired_width(ui.available_width()),
    );
    presets.active_mut().name.retain(|c| !c.is_control());
    pen(ui, &mut presets.active_mut().style);
    ui.weak("Changes apply to new strokes. Existing ink keeps its captured style.");
    ui.separator();
    let index = presets
        .items
        .iter()
        .position(|p| p.id == presets.selected)
        .unwrap();
    ui.horizontal(|ui| {
        if ui
            .add_enabled(index > 0, egui::Button::new("Move left"))
            .clicked()
        {
            presets.move_selected(-1);
        }
        if ui
            .add_enabled(
                index + 1 < presets.items.len(),
                egui::Button::new("Move right"),
            )
            .clicked()
        {
            presets.move_selected(1);
        }
    });
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                presets.items.len() < MAX_PRESETS,
                egui::Button::new("New preset"),
            )
            .on_hover_text("Create a custom copy of this preset")
            .clicked()
        {
            presets.duplicate();
        }
        if ui
            .add_enabled(
                !presets.active().builtin(),
                egui::Button::new("Delete preset"),
            )
            .on_disabled_hover_text(
                "Default presets can be edited and reordered; only custom presets can be deleted",
            )
            .clicked()
        {
            presets.delete_selected();
        }
    });
    if ui
        .button("Restore default presets")
        .on_hover_text("Replace all presets, including custom presets, with the five defaults")
        .clicked()
    {
        presets.restore();
    }
}

pub fn eraser_radius(ui: &mut egui::Ui, radius: &mut f64) {
    ui.horizontal(|ui| {
        ui.label("Eraser radius");
        ui.add(
            egui::DragValue::new(radius)
                .range(4.0..=48.0)
                .speed(1.0)
                .suffix(" screen pt"),
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::settings::tests::{pointer, render};
    #[test]
    fn preset_controls_create_reorder_delete_and_restore_real_state() {
        let ctx = egui::Context::default();
        let mut presets = Presets::default();
        fn click(ctx: &egui::Context, presets: &mut Presets, label: &str) {
            render(ctx, vec![], |ui| preset(ui, presets));
            let labels = render(ctx, vec![], |ui| preset(ui, presets));
            let pos = labels[label];
            render(ctx, pointer(pos, true), |ui| preset(ui, presets));
            render(ctx, pointer(pos, false), |ui| preset(ui, presets));
        }
        click(&ctx, &mut presets, "New preset");
        let id = presets.selected;
        assert_eq!(presets.items.len(), 6);
        click(&ctx, &mut presets, "Move left");
        assert_eq!(presets.items[4].id, id);
        click(&ctx, &mut presets, "Delete preset");
        assert_eq!(presets.items.len(), 5);
        click(&ctx, &mut presets, "New preset");
        click(&ctx, &mut presets, "Restore default presets");
        assert_eq!(presets, Presets::default());
    }
}
