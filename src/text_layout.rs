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
    ctx.set_fonts(fonts);
}
pub fn font(content: &TextContent) -> FontId {
    let family = match content.style.family {
        TextFamily::Sans => "sans",
        TextFamily::Monospace => "mono",
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
        Alignment::Left => egui::Align::LEFT,
        Alignment::Center => egui::Align::Center,
        Alignment::Right => egui::Align::RIGHT,
    }
}
pub fn job(content: &TextContent, width: f64) -> egui::text::LayoutJob {
    let c = content.style.color;
    let color = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
    let mut job = egui::text::LayoutJob::default();
    job.append(
        &content.text,
        0.,
        egui::TextFormat {
            font_id: font(content),
            color,
            underline: if content.style.underline {
                Stroke::new(1., color)
            } else {
                Stroke::NONE
            },
            ..Default::default()
        },
    );
    job.keep_trailing_whitespace = true;
    job.round_output_to_gui = false;
    job.wrap.max_width = width as f32;
    job.halign = alignment(content);
    job
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
                fonts: egui::epaint::text::Fonts::new(options, definitions),
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
    service
        .fonts
        .with_pixels_per_point(1.)
        .layout_job(job(content, width))
}
pub fn measure(ctx: &egui::Context, text: &mut TextObject) {
    let mut galley = layout(ctx, &text.content, text.width);
    // A single wide glyph cannot wrap. Keep the box at least as wide as that
    // glyph rather than clipping it or distorting the font.
    if f64::from(galley.size().x) > text.width + 0.001 {
        text.width = f64::from(galley.size().x);
        galley = layout(ctx, &text.content, text.width);
    }
    text.height = f64::from(galley.size().y).max(text.content.style.size * 1.3);
}

/// Reusable plain Unicode editing core: egui owns selection, IME and its local
/// undoer; canvas objects only store source/style. Hosts provide focus/placement.
pub fn edit(
    ui: &mut egui::Ui,
    id: egui::Id,
    content: &mut TextContent,
    width: f64,
    scale: f64,
) -> egui::text_edit::TextEditOutput {
    let mut template = content.clone();
    template.style.size = (template.style.size * scale).clamp(1., 1024.);
    let content_style = content.style.clone();
    let mut layouter = |ui: &egui::Ui, buffer: &dyn egui::TextBuffer, _: f32| {
        layout_scaled(
            ui.ctx(),
            &TextContent {
                text: buffer.as_str().to_owned(),
                style: content_style.clone(),
            },
            width,
            scale,
        )
    };
    let atomic = ui.input(|i| {
        i.events.iter().any(|e| {
            matches!(
                e,
                egui::Event::Cut
                    | egui::Event::Paste(_)
                    | egui::Event::Ime(egui::ImeEvent::Commit(_))
            )
        })
    });
    if atomic && let Some(mut state) = egui::text_edit::TextEditState::load(ui.ctx(), id) {
        let mut undoer = state.undoer();
        undoer.add_undo(&(
            state.cursor.char_range().unwrap_or_default(),
            content.text.clone(),
        ));
        state.set_undoer(undoer);
        state.store(ui.ctx(), id);
    }
    let mut output = egui::TextEdit::multiline(&mut content.text)
        .id(id)
        .font(font(&template))
        .align(egui::Align2([alignment(&template), egui::Align::TOP]))
        .desired_width((width * scale) as f32)
        .desired_rows(1)
        .margin(egui::Margin::ZERO)
        .frame(egui::Frame::NONE)
        .layouter(&mut layouter)
        .show(ui);
    if atomic && output.response.changed() {
        let mut undoer = output.state.undoer();
        undoer.add_undo(&(
            output.state.cursor.char_range().unwrap_or_default(),
            content.text.clone(),
        ));
        output.state.set_undoer(undoer);
        output.state.clone().store(ui.ctx(), id);
    }
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
    screen.style.size = (content.style.size * scale).clamp(1., 1024.);
    let mut parts = Vec::new();
    for canonical in &logical.rows {
        let mut line = screen.clone();
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
                    v.pos.x *= row.size.x / old_width.max(0.001);
                    v.pos.y *= row.size.y / old_height.max(0.001);
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
