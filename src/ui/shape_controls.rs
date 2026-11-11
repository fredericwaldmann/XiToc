//! Shared formatting widgets. Property patches are the only mutation interface.
use crate::shape::{BorderStyle, Shape, ShapeGeometry, ShapeKind, StylePatch};

type Edits = Vec<(StylePatch, egui::Id)>;
fn changed(edits: &mut Edits, response: egui::Response, patch: StylePatch) {
    if response.changed() {
        edits.push((patch, response.id));
    }
}
fn mixed<T: PartialEq>(ui: &mut egui::Ui, shapes: &[Shape], value: impl Fn(&Shape) -> T) {
    // Keep one fixed header slot even after mixed values become equal. This
    // preserves slider IDs/positions throughout the continuous group gesture.
    let mixed = shapes.iter().skip(1).any(|s| value(s) != value(&shapes[0]));
    ui.weak(if mixed {
        "Mixed values"
    } else {
        "Selected objects"
    })
    .on_hover_text("Changes apply to all compatible selected objects.");
}
pub(crate) fn menu(ui: &mut egui::Ui, name: &str, content: impl FnOnce(&mut egui::Ui)) {
    let config = egui::containers::menu::MenuConfig::new()
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
    if egui::containers::menu::is_in_menu(ui) {
        egui::containers::menu::SubMenuButton::new(name)
            .config(config)
            .ui(ui, content);
    } else {
        egui::containers::menu::MenuButton::new(name)
            .config(config)
            .ui(ui, content);
    }
}
fn opacity(
    ui: &mut egui::Ui,
    label: &str,
    alpha: u8,
    edits: &mut Edits,
    patch: impl Fn(u8) -> StylePatch,
) {
    let mut percent = (f32::from(alpha) * 100. / 255.).round();
    let r = super::drawing_controls::styled_slider(
        ui,
        egui::Slider::new(&mut percent, 0.0..=100.0)
            .text(label)
            .suffix("%")
            .fixed_decimals(0)
            .step_by(1.0),
    );
    changed(edits, r, patch((percent * 255. / 100.).round() as u8));
}
/// Inline picker: nested root popups close the parent formatting menu in egui.
pub(crate) fn color_control(ui: &mut egui::Ui, rgb: &mut [u8; 3], alpha: u8) -> Option<egui::Id> {
    let original = *rgb;
    let toggle = ui.make_persistent_id("color-expanded");
    let mut open = ui.data(|d| d.get_temp::<bool>(toggle).unwrap_or(false));
    ui.horizontal(|ui| {
        let preview = egui::color_picker::show_color(
            ui,
            egui::Color32::from_rgba_unmultiplied(rgb[0], rgb[1], rgb[2], alpha),
            egui::vec2(38., 24.),
        );
        let swatch = ui
            .interact(
                preview.rect,
                ui.make_persistent_id("color-swatch"),
                egui::Sense::click(),
            )
            .on_hover_text("Open color picker");
        if ui.button("Color picker").clicked() || swatch.clicked() {
            open = !open;
        }
    });
    ui.data_mut(|d| d.insert_temp(toggle, open));
    if open {
        ui.spacing_mut().slider_width = super::style::COLOR_PICKER_WIDTH;
        let mut value = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
        if egui::color_picker::color_picker_color32(
            ui,
            &mut value,
            egui::color_picker::Alpha::Opaque,
        ) {
            *rgb = [value.r(), value.g(), value.b()];
        }
        ui.weak("RGB color · opacity is adjusted separately");
    }
    let id = ui.make_persistent_id("hex-rgb");
    let focused = ui.ctx().memory(|m| m.has_focus(id));
    let mut hex = if focused {
        ui.data(|d| d.get_temp::<String>(id))
            .unwrap_or_else(|| format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]))
    } else {
        format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])
    };
    ui.horizontal(|ui| {
        ui.label("Hex RGB");
        let response = ui.add(
            egui::TextEdit::singleline(&mut hex)
                .id(id)
                .desired_width(90.)
                .char_limit(7),
        );
        if response.lost_focus()
            && let Some(parsed) = parse_hex(&hex)
        {
            *rgb = parsed;
        }
    });
    ui.data_mut(|d| d.insert_temp(id, hex));
    (*rgb != original).then(|| ui.make_persistent_id("color-edit"))
}
fn parse_hex(text: &str) -> Option<[u8; 3]> {
    let text = text.trim().strip_prefix('#').unwrap_or(text.trim());
    if text.len() != 6 || !text.is_ascii() {
        return None;
    }
    Some([
        u8::from_str_radix(&text[..2], 16).ok()?,
        u8::from_str_radix(&text[2..4], 16).ok()?,
        u8::from_str_radix(&text[4..], 16).ok()?,
    ])
}
fn pattern_length(
    ui: &mut egui::Ui,
    shapes: &[Shape],
    label: &str,
    value: impl Fn(&Shape) -> f64,
    patch: impl Fn(f64) -> StylePatch,
    edits: &mut Edits,
) {
    ui.push_id(label, |ui| {
        mixed(ui, shapes, &value);
        let mut v = value(&shapes[0]);
        let response = super::drawing_controls::styled_slider(
            ui,
            egui::Slider::new(&mut v, 0.25..=256.)
                .logarithmic(true)
                .text(label)
                .suffix(" pt"),
        );
        changed(edits, response, patch(v));
    });
}
/// Horizontal contextual controls or the same menu hierarchy inside right-click.
pub fn controls(ui: &mut egui::Ui, shapes: &[Shape]) -> Edits {
    let mut edits = Vec::new();
    let Some(first) = shapes.first() else {
        return edits;
    };
    let closed: Vec<_> = shapes
        .iter()
        .filter(|s| !s.kind().segment())
        .cloned()
        .collect();
    if let Some(shape) = closed.first() {
        menu(ui, "Fill", |ui| {
            mixed(ui, &closed, |s| (s.style.fill, s.style.fill_opacity));
            let mut enabled = shape.style.fill.is_some();
            changed(
                &mut edits,
                ui.checkbox(&mut enabled, "Fill enabled"),
                StylePatch::Fill(enabled.then_some(shape.style.fill.unwrap_or([90, 150, 220]))),
            );
            let mut color = shape.style.fill.unwrap_or([90, 150, 220]);
            if let Some(id) = color_control(ui, &mut color, shape.style.fill_opacity) {
                edits.push((StylePatch::Fill(Some(color)), id));
            }
            opacity(
                ui,
                "Fill opacity",
                shape.style.fill_opacity,
                &mut edits,
                StylePatch::FillOpacity,
            );
        });
    }
    menu(
        ui,
        if closed.is_empty() { "Line" } else { "Border" },
        |ui| {
            mixed(ui, shapes, |s| (s.style.border, s.style.border_enabled));
            let mut enabled = first.style.border_enabled;
            changed(
                &mut edits,
                ui.checkbox(&mut enabled, "Border / line enabled"),
                StylePatch::BorderEnabled(enabled),
            );
            let mut color = [
                first.style.border[0],
                first.style.border[1],
                first.style.border[2],
            ];
            if let Some(id) = color_control(ui, &mut color, first.style.border[3]) {
                edits.push((StylePatch::Border(color), id));
            }
            opacity(
                ui,
                "Border / line opacity",
                first.style.border[3],
                &mut edits,
                StylePatch::BorderOpacity,
            );
        },
    );
    menu(ui, "Width", |ui| {
        mixed(ui, shapes, |s| s.style.border_width);
        let mut width = first.style.border_width;
        changed(
            &mut edits,
            super::drawing_controls::styled_slider(
                ui,
                egui::Slider::new(&mut width, 0.1..=24.)
                    .text("Width")
                    .suffix(" pt"),
            ),
            StylePatch::Width(width),
        );
    });
    menu(ui, "Style", |ui| {
        mixed(ui, shapes, |s| s.style.border_style);
        for (value, label) in [
            (BorderStyle::Solid, "Solid"),
            (BorderStyle::Dashed, "Dashed"),
            (BorderStyle::Dotted, "Dotted"),
        ] {
            let r =
                ui.selectable_label(shapes.iter().all(|s| s.style.border_style == value), label);
            if r.clicked() {
                edits.push((StylePatch::Pattern(value), r.id));
            }
        }
        let dashed: Vec<_> = shapes
            .iter()
            .filter(|s| s.style.border_style == BorderStyle::Dashed)
            .cloned()
            .collect();
        if !dashed.is_empty() {
            ui.separator();
            pattern_length(
                ui,
                &dashed,
                "Dash length",
                |s| s.style.pattern.dash,
                StylePatch::DashLength,
                &mut edits,
            );
            pattern_length(
                ui,
                &dashed,
                "Gap length",
                |s| s.style.pattern.gap,
                StylePatch::GapLength,
                &mut edits,
            );
        }
        let dotted: Vec<_> = shapes
            .iter()
            .filter(|s| s.style.border_style == BorderStyle::Dotted)
            .cloned()
            .collect();
        if !dotted.is_empty() {
            ui.separator();
            pattern_length(
                ui,
                &dotted,
                "Dot spacing",
                |s| s.style.pattern.dot_spacing,
                StylePatch::DotSpacing,
                &mut edits,
            );
            pattern_length(
                ui,
                &dotted,
                "Dot size",
                |s| s.style.pattern.dot_size,
                StylePatch::DotSize,
                &mut edits,
            );
        }
    });
    menu(ui, "Opacity", |ui| {
        mixed(ui, shapes, |s| s.style.border[3]);
        opacity(
            ui,
            "Border / line opacity",
            first.style.border[3],
            &mut edits,
            StylePatch::BorderOpacity,
        );
        if let Some(shape) = closed.first() {
            mixed(ui, &closed, |s| s.style.fill_opacity);
            opacity(
                ui,
                "Fill opacity",
                shape.style.fill_opacity,
                &mut edits,
                StylePatch::FillOpacity,
            );
        }
    });
    let segments: Vec<_> = shapes
        .iter()
        .filter(|s| s.kind().segment())
        .cloned()
        .collect();
    if !segments.is_empty() {
        menu(ui, "Arrowheads", |ui| {
            mixed(ui, &segments, |s| {
                (
                    s.style.start_head,
                    matches!(s.geometry, ShapeGeometry::Segment { head: Some(_), .. }),
                )
            });
            for (start, end, label) in [
                (false, false, "None"),
                (true, false, "Start"),
                (false, true, "End"),
                (true, true, "Both ends"),
            ] {
                let selected = segments.iter().all(|s| {
                    s.style.start_head == start
                        && matches!(s.geometry, ShapeGeometry::Segment { head: Some(_), .. }) == end
                });
                let r = ui.selectable_label(selected, label);
                if r.clicked() {
                    edits.push((StylePatch::Heads(start, end), r.id));
                }
            }
        });
    }
    let rounded: Vec<_> = shapes
        .iter()
        .filter(|s| s.kind() == ShapeKind::RoundedRectangle)
        .cloned()
        .collect();
    menu(ui, "More", |ui| {
        if !closed.is_empty() {
            let response = ui.button("No fill");
            if response.clicked() {
                edits.push((StylePatch::Fill(None), response.id));
            }
        }
        let response = ui.button("No border / line");
        if response.clicked() {
            edits.push((StylePatch::BorderEnabled(false), response.id));
        }
        if let Some(shape) = rounded.first() {
            mixed(ui, &rounded, |s| match s.geometry {
                ShapeGeometry::Box { corner_radius, .. } => corner_radius,
                _ => 0.,
            });
            if let ShapeGeometry::Box {
                size,
                corner_radius,
                ..
            } = shape.geometry
            {
                let mut radius = corner_radius.min(size[0].min(size[1]) * 0.5);
                changed(
                    &mut edits,
                    super::drawing_controls::styled_slider(
                        ui,
                        egui::Slider::new(&mut radius, 0.0..=size[0].min(size[1]) * 0.5)
                            .text("Corner radius")
                            .suffix(" pt"),
                    ),
                    StylePatch::Radius(radius),
                );
            }
        }
    });
    edits
}

/// Sticky-note appearance stays separate from the shared Text/Paragraph controls.
pub fn sticky_controls(ui: &mut egui::Ui, notes: &[Shape]) -> Edits {
    let mut edits = Vec::new();
    let Some(first) = notes.first() else {
        return edits;
    };
    menu(ui, "Note", |ui| {
        ui.strong("Note appearance");
        let mixed_color = notes
            .iter()
            .skip(1)
            .any(|note| note.style.fill != first.style.fill);
        ui.weak(if mixed_color {
            "Mixed background colors"
        } else {
            "Background color"
        });
        ui.horizontal_wrapped(|ui| {
            for (name, color) in crate::shape::STICKY_NOTE_PALETTE
                .iter()
                .filter(|(name, _)| *name != "Custom")
            {
                let selected = notes.iter().all(|note| note.style.fill == Some(*color));
                if ui
                    .add(
                        egui::Button::new("  ")
                            .fill(egui::Color32::from_rgb(color[0], color[1], color[2]))
                            .selected(selected),
                    )
                    .on_hover_text(*name)
                    .clicked()
                {
                    edits.push((
                        StylePatch::Fill(Some(*color)),
                        ui.make_persistent_id(("note-color", name)),
                    ));
                }
            }
        });
        let mut color = first
            .style
            .fill
            .unwrap_or(crate::shape::STICKY_NOTE_PALETTE[0].1);
        if let Some(id) = color_control(ui, &mut color, 255) {
            edits.push((StylePatch::Fill(Some(color)), id));
        }
        if notes.iter().any(|note| !note.text_background_contrast_ok()) {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                "Text color has low contrast on this background; adjust it in Text formatting.",
            );
        }
        let mut shadow = first.sticky_note.unwrap_or_default().shadow;
        let response = ui.checkbox(&mut shadow, "Subtle shadow");
        if response.changed() {
            edits.push((StylePatch::StickyShadow(shadow), response.id));
        }
        let mut auto_height = first.sticky_note.unwrap_or_default().auto_height;
        let response = ui.checkbox(&mut auto_height, "Automatically grow height while typing");
        if response.changed() {
            edits.push((StylePatch::StickyAutoHeight(auto_height), response.id));
        }
        ui.weak("Manual resize turns automatic height off for that note.");
    });
    edits
}

pub fn defaults(ui: &mut egui::Ui, defaults: &mut crate::shape::ShapeDefaults) {
    ui.strong("Default shape style");
    ui.weak("Applies only to newly created shapes. Arrowheads configure new arrows.");
    let owner = uuid::Uuid::new_v4();
    let mut shapes: Vec<_> = [ShapeKind::RoundedRectangle, ShapeKind::Arrow]
        .into_iter()
        .map(|kind| {
            let mut shape = Shape::create(
                owner,
                crate::drawing::Surface::Canvas,
                kind,
                [0., 0.],
                [100., 100.],
                false,
            )
            .unwrap();
            defaults.apply(&mut shape);
            shape
        })
        .collect();
    ui.horizontal_wrapped(|ui| {
        for (patch, _) in controls(ui, &shapes) {
            match patch {
                StylePatch::Radius(value) => defaults.corner_radius = Some(value),
                StylePatch::Heads(_, end) => defaults.arrow_end = end,
                _ => (),
            }
            for shape in &mut shapes {
                patch.apply(shape);
            }
        }
    });
    defaults.style = shapes[0].style.clone();
    defaults.style.start_head = shapes[1].style.start_head;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mixed_to_common_values_keep_slider_identity_and_position() {
        let ctx = egui::Context::default();
        let owner = uuid::Uuid::new_v4();
        let mut shapes: Vec<_> = [2., 8.]
            .into_iter()
            .map(|width| {
                let mut shape = Shape::create(
                    owner,
                    crate::drawing::Surface::Canvas,
                    ShapeKind::Rectangle,
                    [0., 0.],
                    [100., 80.],
                    false,
                )
                .unwrap();
                shape.style.border_width = width;
                shape
            })
            .collect();
        let render = |shapes: &[Shape]| {
            let mut response = None;
            super::super::settings::tests::render(&ctx, vec![], |ui| {
                mixed(ui, shapes, |s| s.style.border_width);
                let mut width = shapes[0].style.border_width;
                response = Some(super::super::drawing_controls::styled_slider(
                    ui,
                    egui::Slider::new(&mut width, 0.1..=24.).text("Width"),
                ));
            });
            let response = response.unwrap();
            (response.id, response.rect)
        };
        render(&shapes);
        let before = render(&shapes);
        shapes[1].style.border_width = 2.;
        assert_eq!(render(&shapes), before);
    }
    #[test]
    fn displaying_opacity_controls_does_not_quantize_existing_alpha() {
        let ctx = egui::Context::default();
        for alpha in [0, 1, 35, 64, 127, 128, 254, 255] {
            let mut edits = Vec::new();
            for _ in 0..3 {
                super::super::settings::tests::render(&ctx, vec![], |ui| {
                    opacity(
                        ui,
                        "Fill opacity",
                        alpha,
                        &mut edits,
                        StylePatch::FillOpacity,
                    );
                });
            }
            assert!(
                edits.is_empty(),
                "Displaying alpha {alpha} must not edit the document"
            );
        }
    }
    #[test]
    fn inline_picker_and_custom_hex_recolor_shapes_without_touching_alpha() {
        use super::super::settings::tests::{pointer, render};
        let ctx = egui::Context::default();
        let mut shape = Shape::create(
            uuid::Uuid::new_v4(),
            crate::drawing::Surface::Canvas,
            ShapeKind::Rectangle,
            [0., 0.],
            [100., 80.],
            false,
        )
        .unwrap();
        shape.style.border[3] = 127;
        let mut changes = Vec::new();
        let mut frame = |events| {
            render(&ctx, events, |ui| {
                for (patch, _) in controls(ui, &[shape.clone()]) {
                    changes.push(patch);
                    patch.apply(&mut shape);
                }
            })
        };
        frame(vec![]);
        let labels = frame(vec![]);
        frame(pointer(labels["Fill"], true));
        frame(pointer(labels["Fill"], false));
        let labels = frame(vec![]);
        let picker = labels["Color picker"];
        frame(pointer(picker, true));
        frame(pointer(picker, false));
        let labels = frame(vec![]);
        assert!(labels.contains_key("RGB color · opacity is adjusted separately"));
        assert!(
            labels.contains_key("Fill opacity"),
            "Parent menu must stay open"
        );
        let label = labels["Hex RGB"];
        let input = label + egui::vec2(80., 0.);
        frame(pointer(input, true));
        frame(pointer(input, false));
        let key = |key, modifiers| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        };
        frame(vec![
            key(egui::Key::A, egui::Modifiers::COMMAND),
            egui::Event::Text("#D82040".into()),
        ]);
        frame(vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
        drop(frame);
        assert_eq!(shape.style.fill, Some([216, 32, 64]));
        assert_eq!(shape.style.fill_opacity, 35);
        assert_eq!(shape.style.border, [37, 99, 200, 127]);
        assert_eq!(changes, vec![StylePatch::Fill(Some([216, 32, 64]))]);
        assert_eq!(parse_hex("#aBcD09"), Some([171, 205, 9]));
        assert!(parse_hex("#xyz123").is_none());
    }
    #[test]
    fn real_menu_routes_style_with_mixed_values() {
        let ctx = egui::Context::default();
        let owner = uuid::Uuid::new_v4();
        let mut shapes: Vec<_> = [ShapeKind::Rectangle, ShapeKind::Ellipse, ShapeKind::Line]
            .into_iter()
            .map(|kind| {
                Shape::create(
                    owner,
                    crate::drawing::Surface::Canvas,
                    kind,
                    [0., 0.],
                    [100., 80.],
                    false,
                )
                .unwrap()
            })
            .collect();
        shapes[1].style.border_style = BorderStyle::Dashed;
        let mut patches = Vec::new();
        let mut render = |events| {
            super::super::settings::tests::render(&ctx, events, |ui| {
                ui.horizontal(|ui| {
                    for (patch, _) in controls(ui, &shapes) {
                        patches.push(patch);
                        for shape in &mut shapes {
                            patch.apply(shape);
                        }
                    }
                });
            })
        };
        render(vec![]);
        let labels = render(vec![]);
        let click = |render: &mut dyn FnMut(
            Vec<egui::Event>,
        )
            -> std::collections::HashMap<String, egui::Pos2>,
                     pos| {
            render(super::super::settings::tests::pointer(pos, true));
            render(super::super::settings::tests::pointer(pos, false))
        };
        click(&mut render, labels["Style"]);
        let labels = render(vec![]);
        assert!(labels.contains_key("Mixed values"), "{labels:?}");
        assert!(labels.contains_key("Dash length") && labels.contains_key("Gap length"));
        let pos = labels["Dotted"];
        click(&mut render, pos);
        let labels = render(vec![]);
        assert!(labels.contains_key("Dot spacing") && labels.contains_key("Dot size"));
        assert!(!labels.contains_key("Dash length") && !labels.contains_key("Gap length"));
        drop(render);
        assert_eq!(patches, vec![StylePatch::Pattern(BorderStyle::Dotted)]);
        assert!(
            shapes
                .iter()
                .all(|s| s.style.border_style == BorderStyle::Dotted)
        );
    }
}
