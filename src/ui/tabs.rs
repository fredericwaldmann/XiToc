use super::{icons, style};
use crate::document::DocumentKind;
use egui::{Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2};

pub struct Tab<'a> {
    pub path: &'a str,
    pub kind: DocumentKind,
    pub read_only: bool,
    pub dirty: bool,
}
pub enum TabAction {
    Activate(usize),
    Close(usize),
}

#[derive(Default)]
pub struct Tabs {
    last_active: String,
    last_width: f32,
}

impl Tabs {
    pub fn show<'a>(
        &mut self,
        ui: &mut egui::Ui,
        documents: impl Iterator<Item = Tab<'a>>,
        active: Option<usize>,
    ) -> Option<TabAction> {
        let p = style::palette(ui);
        let mut action = None;
        let resized = (self.last_width - ui.available_width()).abs() > 1.0;
        self.last_width = ui.available_width();
        egui::ScrollArea::horizontal()
            .id_salt("document-tabs-scroll")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for (index, tab) in documents.enumerate() {
                        let filename = tab.path.rsplit('/').next().unwrap_or(tab.path);
                        let title = if tab.dirty {
                            format!("● {filename}")
                        } else {
                            filename.to_owned()
                        };
                        let width = (title.chars().count() as f32 * 7.0 + 62.0).clamp(128.0, 224.0);
                        let (rect, response) =
                            ui.allocate_exact_size(Vec2::new(width, 32.0), Sense::click());
                        let selected = active == Some(index);
                        let fill = if selected {
                            p.surface
                        } else if response.hovered() {
                            p.hover
                        } else {
                            p.shell
                        };
                        ui.painter().rect_filled(rect, 3.0, fill);
                        if selected {
                            ui.painter().line_segment(
                                [rect.left_bottom(), rect.right_bottom()],
                                Stroke::new(2.0, p.accent),
                            );
                        }
                        if response.has_focus() {
                            ui.painter().rect_stroke(
                                rect.shrink(1.0),
                                3.0,
                                Stroke::new(1.0, p.accent),
                                StrokeKind::Inside,
                            );
                        }
                        icons::paint(
                            ui.painter(),
                            Pos2::new(rect.left() + 16.0, rect.center().y),
                            tab.kind.into(),
                            if selected { p.accent } else { p.muted },
                        );
                        let text_rect = Rect::from_min_max(
                            Pos2::new(rect.left() + 30.0, rect.top()),
                            Pos2::new(rect.right() - 28.0, rect.bottom()),
                        );
                        ui.put(
                            text_rect,
                            egui::Label::new(RichText::new(&title).color(if selected {
                                p.text
                            } else {
                                p.muted
                            }))
                            .selectable(false)
                            .truncate(),
                        );
                        response.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::Role::Tab,
                                ui.is_enabled(),
                                selected,
                                &title,
                            )
                        });
                        let keyboard = response.has_focus()
                            && ui.input(|i| {
                                i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Space)
                            });
                        if response.clicked() || keyboard {
                            action = Some(TabAction::Activate(index));
                        }
                        if selected && (self.last_active != tab.path || resized) {
                            response.scroll_to_me(Some(egui::Align::Center));
                            self.last_active = tab.path.into();
                        }
                        response.on_hover_text(format!(
                            "{}\n{}{}",
                            tab.path,
                            tab.kind.label(),
                            if tab.dirty {
                                " · Unsaved drawing (memory only)"
                            } else if tab.read_only {
                                " · Read-only"
                            } else {
                                ""
                            }
                        ));
                        let close_rect = Rect::from_center_size(
                            Pos2::new(rect.right() - 14.0, rect.center().y),
                            Vec2::splat(22.0),
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
                        close.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::Role::Button,
                                ui.is_enabled(),
                                format!("Close {title}"),
                            )
                        });
                        if close.on_hover_text(format!("Close {title}")).clicked() {
                            action = Some(TabAction::Close(index));
                        }
                    }
                });
            });
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
