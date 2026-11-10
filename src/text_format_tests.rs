//! Real Canvas + egui controls: retained selection through focus and layout changes.
use super::{
    rich_tests::{key, selected, start},
    *,
};
use crate::{
    canvas::selection_ui::tests::setup,
    document::DocumentKind,
    text::{Alignment, TextStylePatch},
};
struct Harness {
    ctx: egui::Context,
    canvas: Canvas,
    surface: Surface,
    id: Uuid,
    labels: HashMap<String, Pos2>,
    galleys: Vec<Arc<egui::Galley>>,
}
impl Harness {
    fn new(text: &str) -> Self {
        let (mut canvas, surface) = setup(DocumentKind::Whiteboard);
        let ctx = egui::Context::default();
        text_layout::install(&ctx);
        let mut t = TextObject::new(canvas.drawing.owner(), surface, [220., 240.], 340.);
        t.height = 130.;
        t.content_mut().text = text.into();
        let id = t.id;
        canvas.drawing.replace_texts(vec![t.clone()], false);
        start(&ctx, &mut canvas, t);
        let mut h = Self {
            ctx,
            canvas,
            surface,
            id,
            labels: HashMap::new(),
            galleys: Vec::new(),
        };
        h.frame(vec![]);
        h.frame(vec![]);
        h
    }
    fn with_shape_text() -> Self {
        let (mut canvas, surface) = setup(DocumentKind::Whiteboard);
        let ctx = egui::Context::default();
        text_layout::install(&ctx);
        let shape = crate::shape::Shape::create(
            canvas.drawing.owner(),
            surface,
            crate::shape::ShapeKind::Rectangle,
            [220., 220.],
            [540., 420.],
            false,
        )
        .unwrap();
        let id = shape.id;
        canvas.drawing.add_shape(shape.clone());
        canvas.select(Tool::Select);
        canvas
            .selection
            .change([id], Membership::Replace, &canvas.drawing);
        let mut output = ctx.run_ui(Default::default(), |ui| canvas.start_shape_text(ui, &shape));
        output.textures_delta.clear();
        let mut harness = Self {
            ctx,
            canvas,
            surface,
            id,
            labels: HashMap::new(),
            galleys: Vec::new(),
        };
        harness.frame(vec![]);
        harness
    }
    fn value(&self) -> &TextObject {
        if let Some(session) = self
            .canvas
            .text_session
            .as_ref()
            .filter(|s| s.target == TextTarget::Shape)
        {
            return &session.value;
        }
        let Some(ObjectSnapshot::Text(t)) = self.canvas.drawing.snapshot(self.id) else {
            panic!("text disappeared")
        };
        t
    }
    fn frame(&mut self, events: Vec<Event>) {
        let mut output = self.ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1000., 760.))),
                events,
                focused: true,
                ..Default::default()
            },
            |ui| {
                self.canvas.begin_frame(ui);
                self.canvas.surface(
                    ui,
                    Rect::from_min_size(Pos2::ZERO, egui::vec2(1000., 760.)),
                    self.surface,
                    crate::canvas::selection_ui::tests::t(),
                );
                self.canvas.end_frame(ui);
            },
        );
        self.labels.clear();
        self.galleys.clear();
        fn collect(
            shape: &egui::Shape,
            labels: &mut HashMap<String, Pos2>,
            galleys: &mut Vec<Arc<egui::Galley>>,
        ) {
            match shape {
                egui::Shape::Text(t) => {
                    labels.insert(
                        t.galley.text().into(),
                        t.pos + t.galley.rect.center().to_vec2(),
                    );
                    galleys.push(t.galley.clone());
                }
                egui::Shape::Vec(shapes) => {
                    for s in shapes {
                        collect(s, labels, galleys);
                    }
                }
                _ => (),
            }
        }
        for shape in &output.shapes {
            collect(&shape.shape, &mut self.labels, &mut self.galleys);
        }
        output.textures_delta.clear();
    }
    fn select(&mut self, start: usize, end: usize) -> egui::text::CCursorRange {
        selected(&self.ctx, &mut self.canvas, start, end);
        self.frame(vec![]);
        self.canvas.text_session.as_ref().unwrap().cursor.unwrap()
    }
    fn check(&self, expected: egui::text::CCursorRange) {
        assert_eq!(
            self.canvas
                .text_session
                .as_ref()
                .expect("editing must stay active")
                .cursor,
            Some(expected)
        );
        let id = egui::Id::unique(("text-editor", self.id));
        assert_eq!(
            egui::text_edit::TextEditState::load(&self.ctx, id)
                .unwrap()
                .cursor
                .char_range(),
            Some(expected)
        );
    }
    fn highlight(&self) {
        let inactive = self
            .ctx
            .style_of(self.ctx.theme())
            .visuals
            .selection
            .bg_fill
            .gamma_multiply(crate::ui::style::INACTIVE_TEXT_SELECTION_OPACITY);
        let active = self
            .ctx
            .style_of(self.ctx.theme())
            .visuals
            .selection
            .bg_fill;
        let galley = self
            .galleys
            .iter()
            .find(|g| g.text() == self.value().content.text)
            .expect("editable text was painted");
        assert!(
            galley.rows.iter().any(|r| r
                .visuals
                .mesh
                .vertices
                .iter()
                .any(|v| v.color == inactive || v.color == active)),
            "selected characters need a visible selection background"
        );
    }
    fn click(&mut self, label: &str, expected: egui::text::CCursorRange) {
        let p = *self
            .labels
            .get(label)
            .unwrap_or_else(|| panic!("missing {label}: {:?}", self.labels.keys()));
        for pressed in [true, false] {
            self.frame(vec![
                Event::PointerMoved(p),
                Event::PointerButton {
                    pos: p,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ]);
            self.check(expected);
        }
        self.frame(vec![]);
        self.check(expected);
    }
}

#[test]
fn shape_text_is_one_shape_and_reuses_the_rich_editor_and_formatting_scope() {
    let mut h = Harness::with_shape_text();
    let session = h.canvas.text_session.as_ref().unwrap();
    assert_eq!(session.target, TextTarget::Shape);
    assert_eq!(session.value.id, h.id);
    assert!(h.canvas.text_editing());

    h.frame(vec![Event::Text("Hello beautiful world".into())]);
    let Some(ObjectSnapshot::Shape(shape)) = h.canvas.drawing.snapshot(h.id) else {
        panic!("editing must keep the canonical shape object")
    };
    assert_eq!(h.canvas.drawing.items().len(), 1);
    let text = shape
        .text
        .as_ref()
        .expect("typed text is embedded in the shape");
    assert_eq!(text.content.text, "Hello beautiful world");
    assert_eq!(text.content.style.alignment, Alignment::Center);

    let selection = h.select(6, 15);
    h.click("Text", selection);
    h.click("+", selection);
    h.highlight();
    let Some(ObjectSnapshot::Shape(shape)) = h.canvas.drawing.snapshot(h.id) else {
        panic!("shape identity must remain stable")
    };
    let content = &shape.text.as_ref().unwrap().content;
    assert_eq!(content.style_at(0).size, 18.);
    assert_eq!(content.style_at(8).size, 19.);
    assert_eq!(content.style_at(18).size, 18.);
    assert_eq!(h.canvas.text_session.as_ref().unwrap().value.id, h.id);
}
#[test]
fn font_preview_real_canvas_freezes_direction_and_highlights_cancel_and_commit() {
    let mut h = Harness::new("Hello beautiful world");
    let expected = h.select(15, 6);
    let original = h.value().clone();
    let revision = h.canvas.drawing.revision();
    h.click("Text", expected);
    h.highlight();
    h.click("Font: DejaVu Sans", expected);
    h.highlight();
    assert!(h.canvas.font_browser.is_some());
    assert_eq!(h.canvas.drawing.revision(), revision);
    for _ in 0..3 {
        h.frame(vec![key(egui::Key::ArrowDown, false)]);
        h.check(expected);
        h.frame(vec![]);
        h.highlight();
    }
    assert_eq!(h.value(), &original);
    h.frame(vec![key(egui::Key::Escape, false)]);
    h.frame(vec![]);
    h.check(expected);
    h.highlight();
    assert!(h.canvas.font_browser.is_none());
    assert_eq!(h.value(), &original);
    assert_eq!(h.canvas.drawing.revision(), revision);
    h.click("Text", expected);
    h.click("Font: DejaVu Sans", expected);
    h.frame(vec![key(egui::Key::ArrowDown, false)]);
    h.frame(vec![]);
    h.check(expected);
    let preview = h.canvas.font_browser.as_ref().unwrap().previews[0]
        .content
        .clone();
    h.frame(vec![key(egui::Key::Enter, false)]);
    h.frame(vec![]);
    h.check(expected);
    h.highlight();
    assert_eq!(h.canvas.drawing.revision(), revision + 1);
    assert_eq!(h.value().content, preview);
    assert_eq!(h.value().content.style_at(0), original.content.style_at(0));
    assert_eq!(
        h.value().content.style_at(18),
        original.content.style_at(18)
    );
}
#[test]
fn font_size_bold_color_and_tab_switches_keep_the_same_word_selected() {
    let mut h = Harness::new("Hello beautiful world");
    let expected = h.select(15, 6);
    let original = h.value().clone();
    h.click("Text", expected);
    h.click("Font: DejaVu Sans", expected);
    h.frame(vec![key(egui::Key::ArrowDown, false)]);
    h.frame(vec![key(egui::Key::Enter, false)]);
    h.frame(vec![]);
    h.check(expected);
    h.click("Text", expected);
    h.click("+", expected);
    h.highlight();
    assert_eq!(h.value().content.style_at(8).size, 19.);
    h.click("Bold", expected);
    h.highlight();
    assert!(h.value().content.style_at(8).bold);
    h.click("#202731", expected);
    h.frame(vec![
        key(egui::Key::A, true),
        Event::Text("#C06020".into()),
        key(egui::Key::Enter, false),
    ]);
    h.check(expected);
    h.highlight();
    h.click("Paragraph", expected);
    h.highlight();
    assert_eq!(&h.value().content.style_at(8).color[..3], &[192, 96, 32]);
    assert_eq!(h.value().content.style_at(0), original.content.style_at(0));
    assert_eq!(
        h.value().content.style_at(18),
        original.content.style_at(18)
    );
    assert!(!h.labels.contains_key("Box") && !h.labels.contains_key("Block"));
    let runs = h.value().content.runs.clone();
    let style = h.value().content.style.clone();
    h.click("Justify", expected);
    h.check(expected);
    h.highlight();
    assert_eq!(h.value().content.paragraph(0).alignment, Alignment::Justify);
    assert_eq!(h.value().content.runs, runs);
    assert_eq!(h.value().content.style, style);
    h.click("Text", expected);
    h.highlight();
    assert!(h.labels.contains_key("Bold"));
}

#[test]
fn text_tab_is_read_only_for_mixed_sizes_and_selection_toolbar_scales_runs() {
    use crate::{
        selection::SelectionCommand,
        text::{TextFamily, TextStylePatch},
    };
    let (mut canvas, surface) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    text_layout::install(&ctx);
    let mut text = TextObject::new(canvas.drawing.owner(), surface, [120., 120.], 300.);
    text.content_mut().text = "Small\nMedium\nLARGE".into();
    for (range, size, family) in [
        (0..5, 12., TextFamily::Sans),
        (6..12, 18., TextFamily::Monospace),
        (13..18, 30., TextFamily::Sans),
    ] {
        text.content_mut().format(
            range,
            &TextStylePatch {
                size: Some(size),
                family: Some(family),
                ..Default::default()
            },
        );
    }
    let id = text.id;
    canvas.drawing.replace_texts(vec![text.clone()], false);
    canvas.select(Tool::Select);
    canvas
        .selection
        .change([id], Membership::Replace, &canvas.drawing);

    let before_revision = canvas.drawing.revision();
    let before_history = canvas.history_available();
    use crate::ui::settings::tests::{pointer as ui_pointer, render};
    let labels = render(&ctx, vec![], |ui| canvas.text_format_controls(ui));
    let text_button = labels["Text"];
    render(&ctx, ui_pointer(text_button, true), |ui| {
        canvas.text_format_controls(ui)
    });
    render(&ctx, ui_pointer(text_button, false), |ui| {
        canvas.text_format_controls(ui)
    });
    let labels = render(&ctx, vec![], |ui| canvas.text_format_controls(ui));
    assert!(
        labels.keys().any(|label| label.contains("Mixed")),
        "{labels:?}"
    );
    let paragraph_button = labels["Paragraph"];
    render(&ctx, ui_pointer(paragraph_button, true), |ui| {
        canvas.text_format_controls(ui)
    });
    render(&ctx, ui_pointer(paragraph_button, false), |ui| {
        canvas.text_format_controls(ui)
    });
    let labels = render(&ctx, vec![], |ui| canvas.text_format_controls(ui));
    let text_button = labels["Text"];
    render(&ctx, ui_pointer(text_button, true), |ui| {
        canvas.text_format_controls(ui)
    });
    render(&ctx, ui_pointer(text_button, false), |ui| {
        canvas.text_format_controls(ui)
    });
    assert_eq!(canvas.drawing.revision(), before_revision);
    assert_eq!(canvas.history_available(), before_history);
    let Some(ObjectSnapshot::Text(current)) = canvas.drawing.snapshot(id) else {
        panic!("text object missing")
    };
    assert_eq!(current.content.style_at(0).size, 12.);
    assert_eq!(current.content.style_at(6).size, 18.);
    assert_eq!(current.content.style_at(13).size, 30.);

    canvas.selection_action(SelectionCommand::Larger, &ctx);
    let Some(ObjectSnapshot::Text(scaled)) = canvas.drawing.snapshot(id) else {
        panic!("text object missing after scale")
    };
    assert_eq!(scaled.id, id);
    for (index, expected) in [(0, 13.2), (6, 19.8), (13, 33.)] {
        assert!((scaled.content.style_at(index).size - expected).abs() < 1e-12);
    }
    assert_eq!(scaled.placement.scale, text.placement.scale);
    canvas.selection_action(SelectionCommand::Smaller, &ctx);
    let Some(ObjectSnapshot::Text(restored)) = canvas.drawing.snapshot(id) else {
        panic!("text object missing after inverse scale")
    };
    for (index, expected) in [(0, 12.0), (6, 18.0), (13, 30.0)] {
        assert!((restored.content.style_at(index).size - expected).abs() < 1e-12);
    }
    canvas.undo();
    let Some(ObjectSnapshot::Text(undone)) = canvas.drawing.snapshot(id) else {
        panic!("text object missing after undo")
    };
    assert!((undone.content.style_at(0).size - 13.2).abs() < 1e-12);
    assert!((undone.content.style_at(6).size - 19.8).abs() < 1e-12);
    assert!((undone.content.style_at(13).size - 33.0).abs() < 1e-12);
    canvas.redo();
    let Some(ObjectSnapshot::Text(redone)) = canvas.drawing.snapshot(id) else {
        panic!("text object missing after redo")
    };
    assert!((redone.content.style_at(6).size - 18.0).abs() < 1e-12);
}
#[test]
fn paragraph_menu_scope_spacing_and_undo_preserve_unicode_selection_and_inline_styles() {
    let mut h = Harness::new("First café\nSecond Ω\nThird 中文");
    let mut t = h.value().clone();
    t.content_mut().format(
        2..16,
        &TextStylePatch {
            bold: Some(true),
            size: Some(29.),
            ..Default::default()
        },
    );
    h.canvas.drawing.replace_texts(vec![t], false);
    h.frame(vec![]);
    let expected = h.select(18, 3);
    let original = h.value().content.clone();
    h.click("Paragraph", expected);
    h.highlight();
    h.click("Justify", expected);
    assert_eq!(h.value().content.paragraph(0).alignment, Alignment::Justify);
    assert_eq!(h.value().content.paragraph(1).alignment, Alignment::Justify);
    assert_eq!(h.value().content.paragraph(2).alignment, Alignment::Left);
    assert_eq!(h.value().content.runs, original.runs);
    assert_eq!(h.value().content.style, original.style);
    // Paragraph and Text are the only formatting sections; resize remains a
    // layout choice in Paragraph rather than decorative styling.
    assert!(h.labels.contains_key("Reflow Text") && h.labels.contains_key("Scale Text (corners)"));
    for removed in ["Box", "Fill", "Border", "Padding", "Container opacity"] {
        assert!(!h.labels.contains_key(removed));
    }
    let justified = h.value().content.clone();
    let number = h
        .labels
        .iter()
        .find(|(label, _)| label.parse::<f64>().ok() == Some(1.))
        .map(|(_, p)| *p)
        .expect("line spacing numeric entry");
    for _ in 0..2 {
        for pressed in [true, false] {
            h.frame(vec![
                Event::PointerMoved(number),
                Event::PointerButton {
                    pos: number,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ]);
            h.check(expected);
        }
    }
    h.frame(vec![
        key(egui::Key::A, true),
        Event::Text("1.75".into()),
        key(egui::Key::Enter, false),
    ]);
    h.frame(vec![]);
    h.check(expected);
    h.highlight();
    assert_eq!(h.value().content.paragraph(0).line_spacing, 1.75);
    assert_eq!(h.value().content.paragraph(1).line_spacing, 1.75);
    assert_eq!(h.value().content.paragraph(2).line_spacing, 1.);
    h.click("Paragraph", expected);
    h.canvas
        .selection_action(crate::selection::SelectionCommand::Undo, &h.ctx);
    h.frame(vec![]);
    h.check(expected);
    assert_eq!(h.value().content, justified);
    h.canvas
        .selection_action(crate::selection::SelectionCommand::Undo, &h.ctx);
    h.frame(vec![]);
    h.check(expected);
    assert_eq!(h.value().content, original);
    h.canvas
        .selection_action(crate::selection::SelectionCommand::Redo, &h.ctx);
    h.frame(vec![]);
    h.check(expected);
    assert_eq!(h.value().content.paragraph(0).alignment, Alignment::Justify);
}

#[test]
fn unicode_grapheme_selection_stays_selected_through_character_toggles() {
    let mut h = Harness::new("Hello e\u{301} 🌍 world");
    let expected = h.select(8, 6);
    let source = h.value().content.text.clone();
    h.click("Text", expected);
    for label in ["Bold", "Italic", "Underline"] {
        h.click(label, expected);
        h.highlight();
    }
    assert_eq!(h.value().content.text, source);
    let style = h.value().content.style_at(6);
    assert!(style.bold && style.italic && style.underline);
    assert_eq!(style, h.value().content.style_at(7));
    assert!(!h.value().content.style_at(9).bold);
    assert!(!h.value().content.style_at(0).bold);
}
