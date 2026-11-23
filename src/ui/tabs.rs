use super::{icons, style};
use crate::document::DocumentKind;
use egui::{Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2};

pub struct Tab<'a> {
    pub path: &'a str,
    pub kind: DocumentKind,
    pub read_only: bool,
    pub dirty: bool,
}
const MIN_TAB: f32 = 96.;
const MAX_TAB: f32 = 210.;
const OVERFLOW: f32 = 34.;
const GAP: f32 = 2.;

pub enum TabAction {
    Activate(usize),
    Close(usize),
    Reorder { from: usize, insertion: usize },
}

#[derive(Default)]
pub struct Tabs {
    dragging: Option<usize>,
}

fn layout(available: f32, count: usize, active: Option<usize>) -> (Vec<usize>, f32) {
    let overflow = count as f32 * (MIN_TAB + GAP) > available;
    let space = (available - if overflow { OVERFLOW } else { 0. }).max(0.);
    let capacity = ((space / (MIN_TAB + GAP)).floor() as usize).min(count);
    let mut indices: Vec<_> = (0..capacity).collect();
    if let Some(active) = active
        && active < count
        && active >= capacity
        && let Some(last) = indices.last_mut()
    {
        *last = active;
    }
    let width = if capacity == 0 {
        0.
    } else {
        (space / capacity as f32 - GAP).min(MAX_TAB)
    };
    (indices, width)
}

impl Tabs {
    pub fn required_width(count: usize) -> f32 {
        count as f32 * (MAX_TAB + GAP)
    }

    pub fn show<'a>(
        &mut self,
        ui: &mut egui::Ui,
        documents: impl Iterator<Item = Tab<'a>>,
        active: Option<usize>,
    ) -> Option<TabAction> {
        let p = style::palette(ui);
        let documents: Vec<_> = documents.collect();
        let (indices, width) = layout(ui.available_width(), documents.len(), active);
        let mut action = None;
        let mut boundaries = Vec::with_capacity(indices.len());
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            for &index in &indices {
                let tab = &documents[index];
                let filename = tab.path.rsplit('/').next().unwrap_or(tab.path);
                let title = if tab.dirty {
                    format!("● {filename}")
                } else {
                    filename.to_owned()
                };
                let selected = active == Some(index);
                let (_, rect) = ui.allocate_space(Vec2::new(width, 32.));
                boundaries.push((index, rect));
                let body =
                    Rect::from_min_max(rect.min, Pos2::new(rect.right() - 25., rect.bottom()));
                let response = ui.interact(
                    body,
                    ui.make_persistent_id(("tab", tab.path)),
                    Sense::click_and_drag(),
                );
                let dragging = response.drag_started();
                if dragging {
                    self.dragging = Some(index);
                }
                response.widget_info(|| {
                    egui::WidgetInfo::selected(egui::Role::Tab, ui.is_enabled(), selected, &title)
                });
                let keyboard = response.has_focus()
                    && ui.input(|i| {
                        i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Space)
                    });
                if response.clicked() || keyboard {
                    action = Some(TabAction::Activate(index));
                }
                if selected || response.hovered() || self.dragging == Some(index) {
                    ui.painter().rect_filled(
                        rect,
                        egui::CornerRadius {
                            nw: 7,
                            ne: 7,
                            sw: 0,
                            se: 0,
                        },
                        if selected { p.surface } else { p.hover },
                    );
                } else {
                    ui.painter().vline(
                        rect.right(),
                        rect.y_range().shrink(9.),
                        Stroke::new(1., p.border),
                    );
                }
                if response.has_focus() {
                    ui.painter().rect_stroke(
                        rect.shrink(1.),
                        6.,
                        Stroke::new(1., p.accent),
                        StrokeKind::Inside,
                    );
                }
                icons::paint(
                    ui.painter(),
                    Pos2::new(rect.left() + 15., rect.center().y),
                    tab.kind.into(),
                    if selected { p.accent } else { p.muted },
                );
                ui.put(
                    Rect::from_min_max(Pos2::new(rect.left() + 28., rect.top()), body.max),
                    egui::Label::new(RichText::new(&title).color(if selected {
                        p.text
                    } else {
                        p.muted
                    }))
                    .selectable(false)
                    .truncate(),
                );
                response.on_hover_text(format!(
                    "{}
{}{}",
                    tab.path,
                    tab.kind.label(),
                    if tab.dirty {
                        " · Unsaved drawing"
                    } else if tab.read_only {
                        " · Read-only"
                    } else {
                        ""
                    }
                ));
                let close_rect = Rect::from_center_size(
                    Pos2::new(rect.right() - 12., rect.center().y),
                    Vec2::splat(22.),
                );
                let close = ui.put(
                    close_rect,
                    egui::Button::new(" ").frame_when_inactive(false),
                );
                icons::paint(
                    ui.painter(),
                    close.rect.center(),
                    icons::Icon::Close,
                    p.muted,
                );
                let close_label = format!("Close {filename}");
                close.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), &close_label)
                });
                if close.on_hover_text(close_label).clicked() && self.dragging.is_none() {
                    action = Some(TabAction::Close(index));
                }
            }
            if indices.len() < documents.len() {
                ui.menu_button("▾", |ui| {
                    ui.set_max_width(320.);
                    egui::ScrollArea::vertical()
                        .max_height(340.)
                        .show(ui, |ui| {
                            for (index, tab) in documents
                                .iter()
                                .enumerate()
                                .filter(|(index, _)| !indices.contains(index))
                            {
                                let name = tab.path.rsplit('/').next().unwrap_or(tab.path);
                                ui.horizontal(|ui| {
                                    let label = if tab.dirty {
                                        format!("● {name}")
                                    } else {
                                        name.to_owned()
                                    };
                                    if ui
                                        .add(
                                            egui::Button::new(label)
                                                .selected(active == Some(index))
                                                .truncate(),
                                        )
                                        .on_hover_text(tab.path)
                                        .clicked()
                                    {
                                        action = Some(TabAction::Activate(index));
                                        ui.close();
                                    }
                                    if icons::button(
                                        ui,
                                        icons::Icon::Close,
                                        "Close document",
                                        false,
                                    )
                                    .clicked()
                                    {
                                        action = Some(TabAction::Close(index));
                                        ui.close();
                                    }
                                });
                            }
                        });
                })
                .response
                .on_hover_text("More documents");
            }
        });
        if let Some(from) = self.dragging {
            let (pointer, down) =
                ui.input(|i| (i.pointer.interact_pos(), i.pointer.primary_down()));
            if let Some(pointer) = pointer
                && let (Some((_, first)), Some((last_index, last))) =
                    (boundaries.first(), boundaries.last())
                && first.union(*last).expand(10.).contains(pointer)
            {
                let (insertion, x) = boundaries
                    .iter()
                    .find(|(_, rect)| pointer.x < rect.center().x)
                    .map_or((last_index + 1, last.right()), |(index, rect)| {
                        (*index, rect.left())
                    });
                ui.painter()
                    .vline(x, first.y_range().shrink(3.), Stroke::new(2., p.accent));
                if !down {
                    action = Some(TabAction::Reorder { from, insertion });
                }
            }
            if !down || !ui.is_enabled() {
                self.dragging = None;
            }
        }
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overflow_keeps_the_active_tab_and_reserves_menu_space() {
        let (indices, width) = layout(330., 12, Some(11));
        assert_eq!(indices, [0, 1, 11]);
        assert!(width >= MIN_TAB);
        assert!(indices.len() as f32 * (width + GAP) + OVERFLOW <= 330.);
        assert_eq!(layout(600., 2, Some(0)).1, MAX_TAB);
    }

    #[test]
    fn dragging_a_tab_emits_an_insertion_on_release() {
        let ctx = egui::Context::default();
        let mut tabs = Tabs::default();
        let first = egui::pos2(60., 16.);
        let second = egui::pos2(340., 16.);
        let button = |pressed, pos| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let mut action = None;
        for events in [
            vec![],
            vec![egui::Event::PointerMoved(first), button(true, first)],
            vec![egui::Event::PointerMoved(second)],
            vec![button(false, second)],
        ] {
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(600., 200.))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    action = tabs.show(
                        ui,
                        ["a/Notes.pdf", "b/Notes.pdf"].into_iter().map(|path| Tab {
                            path,
                            kind: DocumentKind::Page,
                            read_only: false,
                            dirty: true,
                        }),
                        Some(0),
                    );
                },
            )
            .textures_delta
            .clear();
        }
        assert!(matches!(
            action,
            Some(TabAction::Reorder {
                from: 0,
                insertion: 2
            })
        ));
    }

    #[test]
    fn clicking_the_filename_activates_the_tab() {
        let ctx = egui::Context::default();
        let mut tabs = Tabs::default();
        let pointer = egui::pos2(60.0, 16.0);
        let frames = [
            vec![],
            vec![
                egui::Event::PointerMoved(pointer),
                egui::Event::PointerButton {
                    pos: pointer,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            vec![egui::Event::PointerButton {
                pos: pointer,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        ];
        let mut action = None;
        for events in frames {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(600.0, 200.0))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    action = tabs.show(
                        ui,
                        [
                            Tab {
                                path: "Page.pdf",
                                kind: DocumentKind::Page,
                                read_only: false,
                                dirty: false,
                            },
                            Tab {
                                path: "Board.pdf",
                                kind: DocumentKind::Whiteboard,
                                read_only: false,
                                dirty: false,
                            },
                        ]
                        .into_iter(),
                        Some(1),
                    );
                },
            );
            output.textures_delta.clear();
        }
        assert!(matches!(action, Some(TabAction::Activate(0))));
    }
}
