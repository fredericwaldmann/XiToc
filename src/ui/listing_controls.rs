//! Listing settings share XiToc's LaTeX settings dialogs and color controls.
use crate::code_listing::{self as listing, Frame, Numbers, Style, Styles, Typeface};

#[derive(Default)]
pub(super) struct StyleDraft {
    selected: String,
    name: String,
    directive: String,
    error: Option<String>,
}

fn color(ui: &mut egui::Ui, label: &str, value: &mut [u8; 3]) {
    ui.horizontal(|ui| {
        ui.label(label);
        super::shape_controls::color_control(ui, value, 255);
    });
}
fn number<T: egui::emath::Numeric>(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut T,
    limit: std::ops::RangeInclusive<T>,
) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(value).range(limit).speed(0.25));
    });
}

pub(crate) fn style_fields(ui: &mut egui::Ui, style: &mut Style) {
    egui::ComboBox::from_id_salt("listing-typeface")
        .selected_text(match style.typeface {
            Typeface::DejaVu => "DejaVu Sans Mono",
            Typeface::UiMonospace => "Interface Monospace",
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut style.typeface, Typeface::DejaVu, "DejaVu Sans Mono");
            ui.selectable_value(
                &mut style.typeface,
                Typeface::UiMonospace,
                "Interface Monospace",
            );
        });
    number(ui, "Font size", &mut style.size, 6. ..=72.);
    number(ui, "Line spacing", &mut style.spacing, 1. ..=3.);
    color(ui, "Background", &mut style.background);
    number(ui, "Background opacity", &mut style.opacity, 0..=255);
    egui::ComboBox::from_id_salt("listing-frame")
        .selected_text(format!("{:?}", style.frame))
        .show_ui(ui, |ui| {
            for (frame, name) in [
                (Frame::None, "No frame"),
                (Frame::Single, "Single"),
                (Frame::Lines, "Top and bottom"),
                (Frame::Left, "Left line"),
            ] {
                ui.selectable_value(&mut style.frame, frame, name);
            }
        });
    if style.frame != Frame::None {
        color(ui, "Frame color", &mut style.rule);
        number(ui, "Frame thickness", &mut style.thickness, 0. ..=12.);
    }
    egui::ComboBox::from_id_salt("listing-numbers")
        .selected_text(format!("Line numbers · {:?}", style.numbers))
        .show_ui(ui, |ui| {
            for mode in [Numbers::None, Numbers::Left, Numbers::Right] {
                ui.selectable_value(&mut style.numbers, mode, format!("{mode:?}"));
            }
        });
    if style.numbers != Numbers::None {
        color(ui, "Number color", &mut style.number_color);
        number(ui, "Number size", &mut style.number_size, 6. ..=72.);
        number(ui, "Number separation", &mut style.number_sep, 0. ..=96.);
        number(ui, "First number", &mut style.first, 0..=u32::MAX);
        number(ui, "Number interval", &mut style.step, 1..=u32::MAX);
    }
    ui.checkbox(&mut style.wrap, "Word wrap");
    if style.wrap {
        ui.checkbox(&mut style.break_at_space, "Wrap at whitespace");
    }
    number(ui, "Tab width", &mut style.tabs, 1..=16);
    ui.collapsing("Whitespace and margins", |ui| {
        ui.checkbox(&mut style.show_spaces, "Show spaces");
        ui.checkbox(&mut style.show_tabs, "Show tabs");
        ui.checkbox(&mut style.show_string_spaces, "Show spaces in strings");
        number(ui, "Padding", &mut style.padding, 0. ..=96.);
        number(ui, "Left margin", &mut style.left, 0. ..=96.);
        number(ui, "Right margin", &mut style.right, 0. ..=96.);
    });
    ui.checkbox(&mut style.header, "Show filename and language");
    ui.label("Caption");
    ui.add(egui::TextEdit::singleline(&mut style.caption).char_limit(128));
    ui.collapsing("Syntax colors", |ui| {
        color(ui, "Base text", &mut style.foreground);
        color(ui, "Keywords", &mut style.keyword);
        color(ui, "Comments", &mut style.comment);
        color(ui, "Strings", &mut style.string);
        color(ui, "Identifiers", &mut style.identifier);
        color(ui, "Constants and numbers", &mut style.numeric);
        ui.checkbox(&mut style.italic_comments, "Italic comments");
    });
}

impl StyleDraft {
    pub(super) fn show(&mut self, ui: &mut egui::Ui, styles: &mut Styles, inherited: &Styles) {
        ui.separator();
        ui.strong("Code Listing Styles");
        let names: std::collections::BTreeSet<_> = listing::PRESETS
            .into_iter()
            .chain(inherited.keys().map(String::as_str))
            .chain(styles.keys().map(String::as_str))
            .map(str::to_owned)
            .collect();
        if !names.contains(&self.selected) {
            self.selected = listing::LIGHT.into();
        }
        egui::ComboBox::from_id_salt("named-listing-styles")
            .selected_text(&self.selected)
            .show_ui(ui, |ui| {
                for name in names {
                    let suffix = if styles.contains_key(&name) {
                        if inherited.contains_key(&name) || listing::preset(&name).is_some() {
                            " · Local override"
                        } else {
                            " · Local"
                        }
                    } else if inherited.contains_key(&name) {
                        " · Global"
                    } else {
                        " · Built-in"
                    };
                    ui.selectable_value(
                        &mut self.selected,
                        name.clone(),
                        format!("{name}{suffix}"),
                    );
                }
            });
        let mut style = listing::effective(&self.selected, styles, inherited).unwrap_or_default();
        ui.horizontal_wrapped(|ui| {
            ui.label("Style name");
            ui.text_edit_singleline(&mut self.name);
            if ui.button("Duplicate as New Style").clicked() {
                if listing::valid_name(&self.name)
                    && !styles.contains_key(&self.name)
                    && styles.len() < 64
                {
                    styles.insert(self.name.clone(), style.clone());
                    self.selected = self.name.clone();
                    self.error = None;
                } else {
                    self.error = Some(
                        "Enter a valid, unused local style name. Up to 64 styles are supported."
                            .into(),
                    );
                }
            }
            if ui
                .add_enabled(
                    styles.contains_key(&self.selected),
                    egui::Button::new("Delete Local Style"),
                )
                .clicked()
            {
                styles.remove(&self.selected);
                style = listing::effective(&self.selected, styles, inherited).unwrap_or_default();
            }
        });
        ui.collapsing("Style appearance", |ui| {
            let before = style.clone();
            style_fields(ui, &mut style);
            if style != before
                && style.valid()
                && (styles.len() < 64 || styles.contains_key(&self.selected))
            {
                styles.insert(self.selected.clone(), style.clone());
            }
        });
        ui.collapsing("Listings configuration", |ui| {
            ui.weak(r"Use \lstset{...} or \lstdefinestyle{Name}{...}. This configuration is separate from mathematical LaTeX.");
            ui.add(egui::TextEdit::multiline(&mut self.directive).code_editor().desired_rows(4).desired_width(f32::INFINITY));
            if ui.button("Apply Listing Directive").clicked() {
                self.error = match listing::parse(&self.directive, style.clone(), |name| listing::effective(name, styles, inherited)) {
                    Ok((name, parsed)) => {
                        let name = name.unwrap_or_else(|| self.selected.clone());
                        if styles.contains_key(&name) || styles.len() < 64 {
                            styles.insert(name.clone(), parsed.clone());
                            self.selected = name;
                            style = parsed;
                            None
                        } else { Some("Up to 64 local listing styles are supported.".into()) }
                    }
                    Err(error) => Some(error),
                };
            }
        });
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        ui.label("Preview");
        crate::canvas::listing_style_preview(ui, &style);
    }
}
