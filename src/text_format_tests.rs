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
    fn value(&self) -> &TextObject {
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
