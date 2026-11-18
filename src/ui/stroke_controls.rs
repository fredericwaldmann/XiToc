//! Contextual controls emit one property edit at a time.
use crate::{
    drawing::{Stroke, StrokeStylePatch as Patch},
    pen::PenType,
};
use std::sync::Arc;

fn property<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    strokes: &[&Arc<Stroke>],
    label: &str,
    value: impl Fn(&Stroke) -> T,
) -> T {
    let first = value(strokes[0]);
    let mixed = strokes[1..].iter().any(|stroke| value(stroke) != first);
    ui.label(if mixed {
        format!("{label} · Mixed")
    } else {
        label.into()
    });
    first
}

fn width_field(ui: &mut egui::Ui, width: f64, mixed: bool) -> Option<(Patch, egui::Id)> {
    let id = ui.make_persistent_id("stroke-width-value");
    let focused = ui.ctx().memory(|m| m.has_focus(id));
    let draft = ui.data(|data| data.get_temp::<String>(id));
    let mut text = draft.unwrap_or_else(|| {
        if mixed {
            String::new()
        } else {
            format!("{width:.2}")
        }
    });
    let response = ui
        .add(
            egui::TextEdit::singleline(&mut text)
                .id(id)
                .hint_text("Mixed")
                .char_limit(12)
                .desired_width(50.0)
                .clip_text(true),
        )
        .on_hover_text("Base width in points");
    let has_focus = response.has_focus();
    ui.data_mut(|data| {
        if has_focus {
            data.insert_temp(id, text.clone());
        } else {
            data.remove::<String>(id);
        }
    });
    if focused
        && response.lost_focus()
        && let Ok(value) = text.trim().parse::<f64>()
        && value.is_finite()
        && (mixed || text != format!("{width:.2}"))
    {
        Some((Patch::Width(value), id))
    } else {
        None
    }
}

pub fn controls(ui: &mut egui::Ui, strokes: &[&Arc<Stroke>]) -> Vec<(Patch, egui::Id)> {
    let mut edits = Vec::new();
    if strokes.is_empty() {
        return edits;
    }
    super::shape_controls::menu(ui, "Stroke", |ui| {
        let height = (ui.ctx().content_rect().height() - 112.0).max(120.0);
        egui::ScrollArea::vertical()
            .max_height(height)
            .show(ui, |ui| {
                let count = strokes.len();
                ui.weak(if count == 1 {
                    "1 editable pen stroke".into()
                } else {
                    format!("{count} editable pen strokes")
                });
                let mut rgb = property(ui, strokes, "Color", |s| {
                    [s.color[0], s.color[1], s.color[2]]
                });
                if let Some(id) =
                    super::shape_controls::color_control(ui, &mut rgb, strokes[0].color[3])
                {
                    edits.push((Patch::Color(rgb), id));
                }
                let mut width = property(ui, strokes, "Width", |s| s.width);
                ui.horizontal(|ui| {
                    let minus = ui
                        .button("−")
                        .on_hover_text("Decrease base width by 0.25 pt");
                    if minus.clicked() {
                        edits.push((Patch::Width(width - 0.25), minus.id));
                    }
                    edits.extend(width_field(
                        ui,
                        width,
                        strokes.iter().any(|s| s.width != width),
                    ));
                    ui.label("pt");
                    let plus = ui
                        .button("+")
                        .on_hover_text("Increase base width by 0.25 pt");
                    if plus.clicked() {
                        edits.push((Patch::Width(width + 0.25), plus.id));
                    }
                });
                let response = super::drawing_controls::styled_slider(
                    ui,
                    egui::Slider::new(&mut width, 0.25..=24.0)
                        .show_value(false)
                        .max_decimals(2)
                        .clamping(egui::SliderClamping::Edits),
                );
                if response.changed() {
                    edits.push((Patch::Width(width), response.id));
                }
                let alpha = property(ui, strokes, "Opacity", |s| s.color[3]);
                let mut opacity = (f32::from(alpha) * 100.0 / 255.0).round();
                let response = super::drawing_controls::styled_slider(
                    ui,
                    egui::Slider::new(&mut opacity, 0.0..=100.0)
                        .suffix("%")
                        .step_by(1.0)
                        .fixed_decimals(0),
                );
                if response.changed() {
                    edits.push((
                        Patch::Opacity((opacity * 255.0 / 100.0).round() as u8),
                        response.id,
                    ));
                }
                let pen_type = property(ui, strokes, "Pen style", |s| s.pen_type);
                let common = strokes.iter().all(|s| s.pen_type == pen_type);
                super::shape_controls::menu(
                    ui,
                    if common {
                        pen_type.label()
                    } else {
                        "Choose style"
                    },
                    |ui| {
                        for pen_type in PenType::ALL {
                            let response = ui.selectable_label(
                                strokes.iter().all(|s| s.pen_type == pen_type),
                                pen_type.label(),
                            );
                            if response.clicked() {
                                edits.push((Patch::PenType(pen_type), response.id));
                            }
                        }
                    },
                );
                let _ = property(ui, strokes, "Pressure", |s| s.pressure_style.enabled);
                ui.horizontal(|ui| {
                    for (enabled, label) in [(true, "Enabled"), (false, "Disabled")] {
                        let response = ui.selectable_label(
                            strokes.iter().all(|s| s.pressure_style.enabled == enabled),
                            label,
                        );
                        if response.clicked() {
                            edits.push((Patch::Pressure(enabled), response.id));
                        }
                    }
                });
                ui.weak("Samples with no recorded pressure use the base width.");
            });
    });
    edits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        drawing::Surface,
        ui::settings::tests::{pointer, render},
    };

    #[test]
    fn mixed_controls_open_without_edit_and_accept_numeric_width() {
        let ctx = egui::Context::default();
        let mut strokes: Vec<_> = [
            Stroke::new(
                uuid::Uuid::new_v4(),
                Surface::Canvas,
                [30, 80, 90, 128],
                2.0,
            ),
            Stroke::new(
                uuid::Uuid::new_v4(),
                Surface::Canvas,
                [80, 40, 80, 255],
                5.0,
            ),
        ]
        .into_iter()
        .map(Arc::new)
        .collect();
        Arc::make_mut(&mut strokes[1]).pen_type = PenType::Highlighter;
        let mut edits = Vec::new();
        let mut frame = |events| {
            render(&ctx, events, |ui| {
                edits.extend(controls(ui, &strokes.iter().collect::<Vec<_>>()));
            })
        };
        frame(vec![]);
        let labels = frame(vec![]);
        frame(pointer(labels["Stroke"], true));
        frame(pointer(labels["Stroke"], false));
        let labels = frame(vec![]);
        for label in [
            "Color · Mixed",
            "Width · Mixed",
            "Opacity · Mixed",
            "Pen style · Mixed",
        ] {
            assert!(labels.contains_key(label), "{label}: {labels:?}");
        }
        let field = labels["Mixed"];
        frame(pointer(field, true));
        frame(pointer(field, false));
        frame(vec![egui::Event::Text("2.0".into())]);
        frame(vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        drop(frame);
        assert_eq!(
            edits.iter().map(|(patch, _)| *patch).collect::<Vec<_>>(),
            vec![Patch::Width(2.0)]
        );
    }
}
