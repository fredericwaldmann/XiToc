//! One portable font/layout service shared by display and egui TextEdit.
use crate::text::{Alignment, TextContent, TextFamily, TextObject};
use egui::{Color32, FontFamily, FontId, Stroke};
use std::sync::Arc;
pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let data: [(&str, &[u8]); 8] = [
        ("sans", include_bytes!("../assets/fonts/DejaVuSans.ttf")),
        (
            "sans-b",
            include_bytes!("../assets/fonts/DejaVuSans-Bold.ttf"),
        ),
        (
            "sans-i",
            include_bytes!("../assets/fonts/DejaVuSans-Oblique.ttf"),
        ),
        (
            "sans-bi",
            include_bytes!("../assets/fonts/DejaVuSans-BoldOblique.ttf"),
        ),
        ("mono", include_bytes!("../assets/fonts/DejaVuSansMono.ttf")),
        (
            "mono-b",
            include_bytes!("../assets/fonts/DejaVuSansMono-Bold.ttf"),
        ),
        (
            "mono-i",
            include_bytes!("../assets/fonts/DejaVuSansMono-Oblique.ttf"),
        ),
        (
            "mono-bi",
            include_bytes!("../assets/fonts/DejaVuSansMono-BoldOblique.ttf"),
        ),
    ];
    fonts.font_data.insert(
        "droid-cjk".into(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/DroidSansFallbackFull.ttf"
        ))),
    );
    for (name, bytes) in data {
        fonts
            .font_data
            .insert(name.into(), Arc::new(egui::FontData::from_static(bytes)));
        let mut fallback = fonts.families[&FontFamily::Proportional].clone();
        fallback.insert(0, name.into());
        fallback.push("droid-cjk".into());
        fonts
            .families
            .insert(FontFamily::Name(name.into()), fallback);
    }
    for provider in crate::text_fonts::providers(&fonts) {
        ctx.add_font_provider(provider);
    }
    ctx.set_fonts(fonts);
}
pub fn font(content: &TextContent) -> FontId {
    let family = match &content.style.family {
        TextFamily::Sans => "sans",
        TextFamily::Monospace => "mono",
        TextFamily::Installed(name) => {
            return FontId::new(
                content.style.size as f32,
                FontFamily::Name(
                    format!(
                        "system:{name}:{}:{}",
                        content.style.bold, content.style.italic
                    )
                    .into(),
                ),
            );
        }
    };
    let suffix = match (content.style.bold, content.style.italic) {
        (false, false) => "",
        (true, false) => "-b",
        (false, true) => "-i",
        (true, true) => "-bi",
    };
    FontId::new(
        content.style.size as f32,
        FontFamily::Name(format!("{family}{suffix}").into()),
    )
}
pub fn alignment(content: &TextContent) -> egui::Align {
    match content.style.alignment {
        Alignment::Left | Alignment::Justify => egui::Align::LEFT,
        Alignment::Center => egui::Align::Center,
        Alignment::Right => egui::Align::RIGHT,
    }
}
pub fn job(content: &TextContent, width: f64) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let mut bytes: Vec<_> = content.text.char_indices().map(|(i, _)| i).collect();
    bytes.push(content.text.len());
    let n = bytes.len() - 1;
    for run in content.segments() {
        let style = &run.style;
        let c = style.color;
        let color = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
        let template = TextContent {
            style: style.clone(),
            ..Default::default()
        };
        job.append(
            &content.text[bytes[run.range.start]..bytes[run.range.end]],
            0.,
            egui::TextFormat {
                font_id: font(&template),
                color,
                underline: if style.underline {
                    Stroke::new(1., color)
                } else {
                    Stroke::NONE
                },
                ..Default::default()
            },
        );
    }
    if n == 0 {
        job.append(
            "",
            0.,
            egui::TextFormat {
                font_id: font(content),
                ..Default::default()
            },
        );
    }
    job.keep_trailing_whitespace = true;
    job.round_output_to_gui = false;
    job.wrap.max_width = width as f32;
    job.halign = alignment(content);
    job
}
/// Expand breakable word separators in eligible wrapped rows. Glyph advances
/// within words never change. Paragraph-final and single-word rows stay natural.
fn justify_words(galley: &mut egui::Galley, width: f32) {
    let count = galley.rows.len();
    for placed in galley.rows.iter_mut().take(count.saturating_sub(1)) {
        if placed.ends_with_newline {
            continue;
        }
        let row = Arc::make_mut(&mut placed.row);
        let Some(last) = row.glyphs.iter().rposition(|g| !g.chr.is_whitespace()) else {
            continue;
        };
        let first = row
            .glyphs
            .iter()
            .position(|g| !g.chr.is_whitespace())
            .unwrap();
        let gaps: Vec<_> = (first..last)
            .filter(|i| {
                let c = row.glyphs[*i].chr;
                c.is_whitespace() && !matches!(c, '\t' | '\u{a0}' | '\u{202f}')
            })
            .collect();
        if gaps.is_empty() {
            continue;
        }
        let extra = (width - row.glyphs[last].max_x()) / gaps.len() as f32;
        if extra <= 0. || !extra.is_finite() {
            continue;
        }
        let mut shift = 0.;
        for (index, glyph) in row.glyphs.iter_mut().enumerate() {
            glyph.pos.x += shift;
            if gaps.binary_search(&index).is_ok() {
                glyph.advance_width += extra;
                shift += extra;
            }
            if index > last {
                glyph.pos.x = width;
                glyph.advance_width = 0.;
            }
        }
        row.size.x = width;
    }
    galley.rect.max.x = galley
        .rows
        .iter()
        .map(|r| r.pos.x + r.size.x)
        .fold(0., f32::max);
}

struct Metrics {
    fonts: egui::epaint::text::Fonts,
    frame: Option<u64>,
}
/// A normalized (unhinted, 1-pixel-per-point) metrics context makes document
/// wrapping independent of monitor DPI. It uses the same engine and bundled
/// definitions as display, but its glyph atlas is never painted.
pub fn layout(ctx: &egui::Context, content: &TextContent, width: f64) -> Arc<egui::Galley> {
    let id = egui::Id::unique("XiToc-normalized-text-metrics");
    let service = ctx
        .data(|d| d.get_temp::<Arc<std::sync::Mutex<Metrics>>>(id))
        .unwrap_or_else(|| {
            let definitions = ctx.fonts(|f| f.definitions().clone());
            let options = egui::epaint::text::TextOptions {
                font_hinting: false,
                ..Default::default()
            };
            let service = Arc::new(std::sync::Mutex::new(Metrics {
                fonts: egui::epaint::text::Fonts::new(options, definitions.clone())
                    .with_font_providers(crate::text_fonts::providers(&definitions)),
                frame: None,
            }));
            ctx.data_mut(|d| d.insert_temp(id, service.clone()));
            service
        });
    let mut service = service.lock().unwrap();
    let frame = ctx.cumulative_frame_nr();
    if service.frame != Some(frame) {
        let options = *service.fonts.options();
        service.fonts.begin_pass(options, Default::default());
        service.frame = Some(frame);
    }
    let mut parts = Vec::new();
    let mut offset = 0;
    let mut spacings = Vec::new();
    for (index, paragraph) in content.text.split('\n').enumerate() {
        let count = paragraph.chars().count();
        let mut part = content.slice(offset..offset + count);
        part.text = paragraph.to_owned();
        let style = content.paragraph(index);
        part.style.alignment = style.alignment;
        let mut job = job(&part, width);
        for section in &mut job.sections {
            section.format.line_height =
                Some(section.format.font_id.size * 1.3 * style.line_spacing as f32);
        }
        let mut galley = (*service.fonts.with_pixels_per_point(1.).layout_job(job)).clone();
        if style.alignment == Alignment::Justify {
            justify_words(&mut galley, width as f32);
        }
        if index < content.text.matches('\n').count()
            && let Some(row) = galley.rows.last_mut()
        {
            row.ends_with_newline = true;
        }
        galley.rect.min.x = 0.;
        galley.rect.max.x = 0.;
        galley.mesh_bounds = egui::Rect::NOTHING;
        for row in &mut galley.rows {
            row.pos.x = match style.alignment {
                Alignment::Left | Alignment::Justify => 0.,
                Alignment::Center => (width as f32 - row.size.x) * 0.5,
                Alignment::Right => width as f32 - row.size.x,
            };
            galley.rect.min.x = galley.rect.min.x.min(row.pos.x);
            galley.rect.max.x = galley.rect.max.x.max(row.pos.x + row.size.x);
            galley.mesh_bounds |= row.visuals.mesh_bounds.translate(row.pos.to_vec2());
        }
        spacings.push(style.spacing as f32);
        parts.push(Arc::new(galley));
        offset += count + 1;
    }
    let mut merged = egui::Galley::concat(Arc::new(job(content, width)), &parts, 1.);
    let mut y = 0.;
    let mut index = 0;
    merged.rect = egui::Rect::ZERO;
    merged.mesh_bounds = egui::Rect::NOTHING;
    for (part, spacing) in parts.iter().zip(spacings) {
        for source in &part.rows {
            let row = &mut merged.rows[index];
            row.pos = source.pos + egui::vec2(0., y);
            merged.rect |= egui::Rect::from_min_size(row.pos, row.size);
            merged.mesh_bounds |= row.visuals.mesh_bounds.translate(row.pos.to_vec2());
            index += 1;
        }
        y += part.rect.height() + spacing;
    }
    merged.rect.max.y = y;
    Arc::new(merged)
}
pub fn measure(ctx: &egui::Context, text: &mut TextObject) {
    let mut galley = layout(ctx, &text.content, text.width);
    // A single wide glyph cannot wrap. Keep the box at least as wide as that
    // glyph rather than clipping it or distorting the font.
    if f64::from(galley.size().x) > text.width + 0.001 {
        text.width = f64::from(galley.size().x);
        galley = layout(ctx, &text.content, text.width);
    }
    text.height = text
        .height
        .max(f64::from(galley.size().y).max(text.content.style.size * 1.3));
}

/// Rich semantic buffer and mixed layout; egui owns caret/selection and IME.
/// Hosts route undo/redo through the rich object journal before calling this.
pub fn edit(
    ui: &mut egui::Ui,
    id: egui::Id,
    content: &mut TextContent,
    width: f64,
    scale: f64,
    selection: Option<egui::text::CCursorRange>,
    interactive: bool,
) -> egui::text_edit::TextEditOutput {
    let mut template = content.clone();
    template.style.size = (template.style.size * scale).clamp(1., 1024.);
    let mirror = std::rc::Rc::new(std::cell::RefCell::new(content.clone()));
    let layout_content = mirror.clone();
    // The session is the sole selection owner; widget state is its adapter.
    if let Some(selection) = selection {
        let mut state = egui::text_edit::TextEditState::load(ui.ctx(), id).unwrap_or_default();
        state.cursor.set_char_range(Some(selection));
        state.store(ui.ctx(), id);
    }
    let mut layouter = move |ui: &egui::Ui, _: &dyn egui::TextBuffer, _: f32| {
        let mut galley = layout_scaled(ui.ctx(), &layout_content.borrow(), width, scale);
        if (!interactive || !ui.memory(|m| m.has_focus(id)))
            && let Some(selection) = selection
        {
            let mut visuals = ui.visuals().clone();
            visuals.selection.bg_fill = visuals
                .selection
                .bg_fill
                .gamma_multiply(crate::ui::style::INACTIVE_TEXT_SELECTION_OPACITY);
            egui::text_selection::visuals::paint_text_selection(
                &mut galley,
                &visuals,
                &selection,
                None,
            );
        }
        galley
    };
    let typing = ui
        .ctx()
        .data(|d| d.get_temp::<crate::text::TextStyle>(id.with("typing")))
        .unwrap_or_else(|| content.style.clone());
    let paste = ui
        .ctx()
        .data(|d| d.get_temp::<TextContent>(egui::Id::unique("XiToc-rich-clipboard")));
    let mut buffer = RichBuffer {
        content,
        mirror,
        typing,
        paste,
    };
    let mut output = egui::TextEdit::multiline(&mut buffer)
        .id(id)
        .interactive(interactive)
        .font(font(&template))
        .align(egui::Align2::LEFT_TOP)
        .desired_width((width * scale) as f32)
        .desired_rows(1)
        .margin(egui::Margin::ZERO)
        .frame(egui::Frame::NONE)
        .layouter(&mut layouter)
        .show(ui);
    // The framework undoer stores plain strings. Never retain a second history
    // capable of restoring source without its formatting/paragraphs.
    // egui deliberately collapses an unfocused editor's widget selection.
    // Rehydrate that cache from the session; the painted inactive selection
    // above and the next focused editing pass must use the very same range.
    if (!interactive || !output.response.has_focus())
        && let Some(selection) = selection
    {
        output.state.cursor.set_char_range(Some(selection));
        output.cursor_range = Some(selection);
    }
    output.state.clear_undoer();
    output.state.clone().store(ui.ctx(), id);
    output
}

/// Canonical document line breaks with scale-specific glyph rasterization.
/// Galley rows/character indices remain identical at every view scale; egui still
/// owns caret/navigation/selection and uses high-resolution atlas glyphs.
pub fn layout_scaled(
    ctx: &egui::Context,
    content: &TextContent,
    width: f64,
    scale: f64,
) -> Arc<egui::Galley> {
    let logical = layout(ctx, content, width);
    let factor = scale as f32;
    let mut screen = content.clone();
    screen.style.size = (content.style.size * scale).clamp(1., 4096.);
    for run in &mut screen.runs {
        run.style.size = (run.style.size * scale).clamp(1., 4096.);
    }
    let mut parts = Vec::new();
    let mut offset = 0;
    for canonical in &logical.rows {
        let count = canonical.text().chars().count();
        let mut line = screen.slice(offset..offset + count);
        offset += count + usize::from(canonical.ends_with_newline);
        // Slicing must preserve empty rows as well as explicit line breaks.
        line.text = canonical.text();
        let mut rendered = (*ctx.fonts_mut(|f| f.layout_job(job(&line, f64::INFINITY)))).clone();
        for placed in &mut rendered.rows {
            let row = Arc::make_mut(&mut placed.row);
            let old_width = row.size.x;
            let old_height = row.size.y;
            let old_glyphs = row.glyphs.clone();
            for (index, (glyph, source)) in row.glyphs.iter_mut().zip(&canonical.glyphs).enumerate()
            {
                let delta = source.pos.to_vec2() * factor - glyph.pos.to_vec2();
                let start = old_glyphs[index].first_vertex as usize;
                let end = old_glyphs
                    .get(index + 1)
                    .map(|g| g.first_vertex as usize)
                    .unwrap_or(row.visuals.glyph_vertex_range.end);
                for v in &mut row.visuals.mesh.vertices[start..end] {
                    v.pos += delta;
                }
                glyph.pos = egui::pos2(source.pos.x * factor, source.pos.y * factor);
                glyph.advance_width = source.advance_width * factor;
                glyph.line_height = source.line_height * factor;
                glyph.font_ascent = source.font_ascent * factor;
                glyph.font_height = source.font_height * factor;
                glyph.font_face_ascent = source.font_face_ascent * factor;
                glyph.font_face_height = source.font_face_height * factor;
            }
            row.size = canonical.row.size * factor;
            for (i, v) in row.visuals.mesh.vertices.iter_mut().enumerate() {
                if !row.visuals.glyph_vertex_range.contains(&i) {
                    let glyph = old_glyphs
                        .partition_point(|g| g.pos.x <= v.pos.x)
                        .saturating_sub(1);
                    if let (Some(old), Some(new)) =
                        (old_glyphs.get(glyph), canonical.glyphs.get(glyph))
                    {
                        let fraction = (v.pos.x - old.pos.x) / old.advance_width.max(0.001);
                        v.pos.x = (new.pos.x + fraction * new.advance_width) * factor;
                        v.pos.y += new.pos.y * factor - old.pos.y;
                    } else {
                        v.pos.x *= row.size.x / old_width.max(0.001);
                        v.pos.y *= row.size.y / old_height.max(0.001);
                    }
                }
            }
            row.visuals.mesh_bounds = egui::Rect::NOTHING;
            for vertex in &row.visuals.mesh.vertices {
                row.visuals.mesh_bounds.extend_with(vertex.pos);
            }
            placed.pos = egui::pos2(canonical.pos.x * factor, 0.);
        }
        rendered.rect = egui::Rect::from_min_size(
            egui::pos2(canonical.pos.x * factor, 0.),
            canonical.row.size * factor,
        );
        parts.push(Arc::new(rendered));
    }
    let mut galley = egui::Galley::concat(
        Arc::new(job(&screen, width * scale)),
        &parts,
        ctx.pixels_per_point(),
    );
    galley.mesh_bounds = egui::Rect::NOTHING;
    for (row, source) in galley.rows.iter_mut().zip(&logical.rows) {
        row.ends_with_newline = source.ends_with_newline;
        row.pos = egui::pos2(source.pos.x * factor, source.pos.y * factor);
        galley.mesh_bounds |= row.visuals.mesh_bounds.translate(row.pos.to_vec2());
    }
    galley.rect = egui::Rect::from_min_max(
        egui::pos2(logical.rect.min.x * factor, logical.rect.min.y * factor),
        egui::pos2(logical.rect.max.x * factor, logical.rect.max.y * factor),
    );
    Arc::new(galley)
}

/// TextEdit edits this semantic buffer directly; every IME/paste/delete operation
/// updates runs before the widget asks the layouter for its next galley.
struct RichBuffer<'a> {
    content: &'a mut TextContent,
    mirror: std::rc::Rc<std::cell::RefCell<TextContent>>,
    typing: crate::text::TextStyle,
    paste: Option<TextContent>,
}
impl egui::TextBuffer for RichBuffer<'_> {
    fn is_mutable(&self) -> bool {
        true
    }
    fn as_str(&self) -> &str {
        &self.content.text
    }
    fn insert_text(&mut self, text: &str, index: egui::text::CharIndex) -> usize {
        if self.content.text.len() + text.len() > crate::text::MAX_TEXT_BYTES || text.contains('\0')
        {
            return 0;
        }
        let value = self
            .paste
            .as_ref()
            .filter(|p| p.text == text)
            .cloned()
            .unwrap_or(TextContent {
                text: text.into(),
                style: self.typing.clone(),
                ..Default::default()
            });
        self.content.replace(index.0..index.0, &value);
        *self.mirror.borrow_mut() = self.content.clone();
        text.chars().count()
    }
    fn delete_char_range(&mut self, range: std::ops::Range<egui::text::CharIndex>) {
        self.content
            .replace(range.start.0..range.end.0, &TextContent::default());
        *self.mirror.borrow_mut() = self.content.clone();
    }
    fn type_id(&self) -> std::any::TypeId {
        std::any::TypeId::of::<TextContent>()
    }
}
