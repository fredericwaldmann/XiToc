//! Drawing and read-only inspection of source listings.
use super::*;
use crate::{
    asset::{AssetId, BinaryAsset},
    attachment::Attachment,
    code_listing::{self as listing, Frame, Numbers, Style},
    object::Placement,
};
use std::{ops::Range, sync::OnceLock};
use two_face::re_exports::syntect::parsing::{ParseState, Scope, ScopeStack, SyntaxSet};

#[derive(Clone, Copy, PartialEq)]
enum Token {
    Text,
    Keyword,
    Comment,
    String,
    Identifier,
    Number,
}

struct SourceLine {
    range: Range<usize>,
    tokens: Vec<(Range<usize>, Token)>,
}

pub(super) struct SourcePreview {
    asset: std::sync::Weak<BinaryAsset>,
    checkpoints: Vec<u32>,
    line_count: usize,
    lines: Vec<SourceLine>,
}
impl SourcePreview {
    const STRIDE: usize = 64;

    fn new(file: &Arc<BinaryAsset>, source: &str, language: &str) -> Self {
        let mut checkpoints = vec![0];
        let mut line_count = 1usize;
        for (index, byte) in source.bytes().enumerate() {
            if byte == b'\n' {
                if line_count.is_multiple_of(Self::STRIDE) {
                    checkpoints.push((index + 1) as u32);
                }
                line_count += 1;
            }
        }
        Self {
            asset: Arc::downgrade(file),
            checkpoints,
            line_count,
            lines: parse_preview(source, language),
        }
    }
    fn visible_from<'a>(
        &'a self,
        source: &'a str,
        first: usize,
    ) -> impl Iterator<Item = (&'a str, &'a [(Range<usize>, Token)])> + 'a {
        let start = self.checkpoints[first / Self::STRIDE] as usize;
        source[start..]
            .split('\n')
            .skip(first % Self::STRIDE)
            .enumerate()
            .map(move |(index, source)| {
                let tokens = self
                    .lines
                    .get(first + index)
                    .map_or(&[][..], |line| line.tokens.as_slice());
                (source, tokens)
            })
    }
}

type LayoutKey = (AssetId, String, Style, [f32; 2], f32, usize);

#[derive(Default)]
pub(super) struct ListingLayout {
    key: Option<LayoutKey>,
    glyphs: Vec<(f32, f32, Arc<egui::Galley>)>,
}

const MAX_HIGHLIGHT_BYTES: usize = 512 * 1024;
const MAX_LINE_BYTES: usize = 8192;

fn extension(language: &str) -> Option<&'static str> {
    listing::LANGUAGES
        .iter()
        .find(|(name, _)| *name == language)
        .map(|(_, ext)| ext[0])
}

fn syntax_set() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(two_face::syntax::extra_newlines)
}

fn category(scopes: &ScopeStack) -> Token {
    static PREFIXES: OnceLock<[(Scope, Token); 8]> = OnceLock::new();
    let prefixes = PREFIXES.get_or_init(|| {
        [
            ("comment", Token::Comment),
            ("string", Token::String),
            ("markup", Token::String),
            ("keyword", Token::Keyword),
            ("storage", Token::Keyword),
            ("constant", Token::Number),
            ("entity", Token::Identifier),
            ("support", Token::Identifier),
        ]
        .map(|(prefix, token)| (Scope::new(prefix).expect("static scope"), token))
    });
    scopes
        .as_slice()
        .iter()
        .rev()
        .find_map(|scope| {
            prefixes
                .iter()
                .find_map(|(prefix, token)| prefix.is_prefix_of(*scope).then_some(*token))
        })
        .unwrap_or(Token::Text)
}

fn parse_preview(source: &str, language: &str) -> Vec<SourceLine> {
    let syntax = (language != "Plain Text")
        .then(|| extension(language).and_then(|ext| syntax_set().find_syntax_by_extension(ext)))
        .flatten();
    let mut parser = syntax.map(ParseState::new);
    let mut scopes = ScopeStack::new();
    let mut at = 0;
    let bounded = if source.len() <= MAX_HIGHLIGHT_BYTES {
        source
    } else {
        let prefix = &source[..source.floor_char_boundary(MAX_HIGHLIGHT_BYTES)];
        prefix
            .rsplit_once('\n')
            .map_or("", |(complete, _)| &source[..complete.len() + 1])
    };
    bounded
        .split_inclusive('\n')
        .take(8192)
        .map(|line| {
            let end = at + line.trim_end_matches(['\r', '\n']).len();
            let range = at..end;
            let tokens = if end <= MAX_HIGHLIGHT_BYTES
                && line.len() <= MAX_LINE_BYTES
                && let Some(parser) = &mut parser
                && let Ok(ops) = parser.parse_line(line, syntax_set())
            {
                let mut start = 0;
                let mut spans = Vec::new();
                for (stop, op) in ops {
                    let stop = stop.min(range.len());
                    if stop > start {
                        spans.push((start..stop, category(&scopes)));
                    }
                    let _ = scopes.apply(&op);
                    start = stop;
                }
                if start < range.len() {
                    spans.push((start..range.len(), category(&scopes)));
                }
                spans
            } else {
                // Resume from the grammar root after an oversized or malformed line.
                parser = syntax.map(ParseState::new);
                scopes = ScopeStack::new();
                Vec::new()
            };
            at += line.len();
            SourceLine { range, tokens }
        })
        .collect()
}

fn rgb(value: [u8; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(value[0], value[1], value[2])
}

fn font(style: &Style, size: f32) -> egui::FontId {
    match style.typeface {
        listing::Typeface::DejaVu => egui::FontId::new(size, egui::FontFamily::Name("mono".into())),
        listing::Typeface::UiMonospace => egui::FontId::monospace(size),
    }
}

fn format(token: Token, style: &Style, scale: f32) -> egui::TextFormat {
    let color = match token {
        Token::Text => style.foreground,
        Token::Keyword => style.keyword,
        Token::Comment => style.comment,
        Token::String => style.string,
        Token::Identifier => style.identifier,
        Token::Number => style.numeric,
    };
    let mut format = egui::TextFormat::simple(font(style, style.size * scale), rgb(color));
    format.italics = token == Token::Comment && style.italic_comments;
    format.line_height = Some(style.size * style.spacing * scale);
    format
}

fn append_span(
    job: &mut egui::text::LayoutJob,
    span: &str,
    token: Token,
    style: &Style,
    scale: f32,
    column: &mut usize,
) {
    let mut text = String::with_capacity(span.len());
    for ch in span.chars() {
        match ch {
            '\t' => {
                let tabs = usize::from(style.tabs);
                let space = tabs - *column % tabs;
                text.push(if style.show_tabs { '→' } else { ' ' });
                text.extend(std::iter::repeat_n(' ', space - 1));
                *column += space;
            }
            ' ' if style.show_spaces || (style.show_string_spaces && token == Token::String) => {
                text.push('·');
                *column += 1;
            }
            _ => {
                text.push(ch);
                *column += 1;
            }
        }
    }
    job.append(&text, 0., format(token, style, scale));
}

fn line_job(
    source: &str,
    tokens: &[(Range<usize>, Token)],
    style: &Style,
    width: f32,
    rows: usize,
    scale: f32,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = width;
    job.wrap.max_rows = if style.wrap { rows } else { 1 };
    job.wrap.break_anywhere = !style.break_at_space;
    let end = source.floor_char_boundary(source.len().min(MAX_LINE_BYTES));
    let mut column = 0;
    if tokens.is_empty() {
        append_span(
            &mut job,
            &source[..end],
            Token::Text,
            style,
            scale,
            &mut column,
        );
    } else {
        for (range, token) in tokens.iter().take_while(|(r, _)| r.start < end) {
            append_span(
                &mut job,
                &source[range.start..range.end.min(end)],
                *token,
                style,
                scale,
                &mut column,
            );
        }
    }
    job
}

impl Canvas {
    pub(crate) fn choose_listing(&mut self, ctx: &egui::Context, replace: Option<Uuid>) {
        self.finish();
        self.attachment_io.pick(
            ctx.clone(),
            self.image_target(replace),
            self.drawing.editing_group,
            Some(ctx.theme() == egui::Theme::Dark),
        );
    }

    pub(super) fn open_listing(&mut self, id: Uuid) {
        self.listing_page = 0;
        self.listing_inspector_galley = None;
        self.listing_inspector = Some(id);
        self.listing_focus = true;
    }

    pub(super) fn listing_language<'a>(
        file: &'a Attachment,
        model: &'a listing::Listing,
        style: &'a Style,
    ) -> &'a str {
        model
            .language
            .as_deref()
            .or(style.language.as_deref())
            .unwrap_or_else(|| listing::detect(&file.original_name))
    }

    pub(super) fn effective_listing_for_clipboard(
        &self,
        object: &ObjectSnapshot,
    ) -> ObjectSnapshot {
        let ObjectSnapshot::Attachment(file) = object else {
            return object.clone();
        };
        let Some(listing) = &file.listing else {
            return object.clone();
        };
        let mut copy = (**file).clone();
        let style = listing.style(
            &self.drawing.latex.listing_styles,
            &self.latex.listing_styles,
        );
        let listing = copy.listing.as_mut().unwrap();
        listing.fallback = style.clone();
        // Freeze the copied appearance, including when the destination shadows its name.
        listing.overrides = Some(style);
        copy.into()
    }

    pub(crate) fn scroll_listing(&mut self, ui: &egui::Ui) {
        if self.listing_inspector.is_some() || !matches!(self.tool, Tool::Select | Tool::Lasso) {
            return;
        }
        let Some(file) = self
            .selected_attachment()
            .filter(|file| file.listing.is_some())
        else {
            return;
        };
        let Some((surface, view, bounds)) = self.selection_view else {
            return;
        };
        let Some(position) = ui.input(|i| i.pointer.hover_pos()) else {
            return;
        };
        if file.surface != surface
            || !bounds.contains(position)
            || !ui.clip_rect().contains(position)
            || ui.ctx().layer_id_at(position) != Some(ui.layer_id())
            || !file.hit_test(
                view.to_document([f64::from(position.x), f64::from(position.y)]),
                0.,
            )
        {
            return;
        }
        ui.input_mut(|i| {
            if i.modifiers.is_none() && i.smooth_scroll_delta.y != 0. {
                let line = self.listing_scroll.entry(file.id).or_default();
                *line = (*line - i.smooth_scroll_delta.y / 16.).max(0.);
                if let Some(max) = self
                    .listing_cache
                    .values()
                    .find(|preview| {
                        preview
                            .asset
                            .upgrade()
                            .is_some_and(|asset| asset.id() == file.asset.id())
                    })
                    .map(|p| p.line_count.saturating_sub(1))
                {
                    *line = (*line).min(max as f32);
                }
                i.smooth_scroll_delta.y = 0.;
            }
        });
    }

    pub(super) fn listing_dialog(&mut self, ctx: &egui::Context) -> bool {
        let Some(id) = self.listing_inspector else {
            return false;
        };
        let Some(file) = self.attachment(id) else {
            self.listing_inspector = None;
            return false;
        };
        let Some(model) = &file.listing else {
            self.listing_inspector = None;
            return false;
        };
        let Some(source) = file.asset.listing_source() else {
            return false;
        };
        let style = model.style(
            &self.drawing.latex.listing_styles,
            &self.latex.listing_styles,
        );
        let language = Self::listing_language(&file, model, &style);
        let width = (ctx.content_rect().width() - 56.).clamp(200., 880.);
        let mut close = false;
        const WINDOW_BYTES: usize = 256 * 1024;
        let start = source.floor_char_boundary(
            self.listing_page
                .saturating_mul(WINDOW_BYTES)
                .min(source.len()),
        );
        let end = source.floor_char_boundary(
            (self.listing_page + 1)
                .saturating_mul(WINDOW_BYTES)
                .min(source.len()),
        );
        let window = &source[start..end];
        let key = (file.asset.id(), language.to_owned());
        let preview = cache_source(&mut self.listing_cache, key, &file.asset, source, language);
        let response =
            egui::Modal::new(egui::Id::unique(("listing-inspector", id))).show(ctx, |ui| {
                ui.set_width(width);
                ui.heading(&file.original_name);
                ui.weak(format!("{language} · Read-only source"));
                if source.len() > WINDOW_BYTES {
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(start > 0, egui::Button::new("Previous section"))
                            .clicked()
                        {
                            self.listing_page -= 1;
                            self.listing_focus = true;
                        }
                        ui.label(format!(
                            "Section {} of {}",
                            self.listing_page + 1,
                            source.len().div_ceil(WINDOW_BYTES)
                        ));
                        if ui
                            .add_enabled(end < source.len(), egui::Button::new("Next section"))
                            .clicked()
                        {
                            self.listing_page += 1;
                            self.listing_focus = true;
                        }
                    });
                }
                egui::ScrollArea::new([!style.wrap, true])
                    .id_salt(("inspection-scroll", start))
                    .max_height((ctx.content_rect().height() - 160.).max(70.))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let background = ui.painter().add(egui::Shape::Noop);
                        let mut buffer = window;
                        let mut layouter =
                            |ui: &egui::Ui, _buffer: &dyn egui::TextBuffer, wrap_width: f32| {
                                let wrap_width = if style.wrap {
                                    wrap_width
                                } else {
                                    f32::INFINITY
                                };
                                let key = (
                                    file.asset.id(),
                                    language.to_owned(),
                                    style.clone(),
                                    start,
                                    wrap_width,
                                );
                                if self
                                    .listing_inspector_galley
                                    .as_ref()
                                    .is_none_or(|entry| entry.0 != key)
                                {
                                    let mut job = egui::text::LayoutJob::default();
                                    job.wrap.max_width = wrap_width;
                                    job.wrap.break_anywhere = !style.break_at_space;
                                    let mut cursor = start;
                                    for line in preview
                                        .lines
                                        .iter()
                                        .skip_while(|line| line.range.end <= start)
                                        .take_while(|line| line.range.start < end)
                                    {
                                        for (range, token) in &line.tokens {
                                            let begin = (line.range.start + range.start).max(start);
                                            let stop = (line.range.start + range.end).min(end);
                                            if begin >= stop {
                                                continue;
                                            }
                                            if begin > cursor {
                                                job.append(
                                                    &source[cursor..begin],
                                                    0.,
                                                    format(Token::Text, &style, 1.),
                                                );
                                            }
                                            job.append(
                                                &source[begin..stop],
                                                0.,
                                                format(*token, &style, 1.),
                                            );
                                            cursor = stop;
                                        }
                                    }
                                    job.append(
                                        &source[cursor..end],
                                        0.,
                                        format(Token::Text, &style, 1.),
                                    );
                                    let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
                                    self.listing_inspector_galley = Some((key, galley));
                                }
                                self.listing_inspector_galley.as_ref().unwrap().1.clone()
                            };
                        let response = ui.add(
                            egui::TextEdit::multiline(&mut buffer)
                                .id(egui::Id::unique(("listing-source", id, start)))
                                .desired_width(if style.wrap { width } else { f32::INFINITY })
                                .font(font(&style, style.size))
                                .layouter(&mut layouter)
                                .frame(egui::Frame::NONE),
                        );
                        ui.painter().set(
                            background,
                            egui::Shape::rect_filled(response.rect, 0., rgb(style.background)),
                        );
                        if self.listing_focus {
                            response.request_focus();
                            self.listing_focus = false;
                        }
                    });
                ui.horizontal(|ui| {
                    if ui.button("Copy Entire Source").clicked() {
                        ctx.copy_text(source.into());
                    }
                    close = ui.button("Close").clicked();
                });
                close |= ui
                    .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
            });
        if close || response.backdrop_response.clicked() {
            self.listing_inspector = None;
            self.listing_inspector_galley = None;
        }
        true
    }

    pub(super) fn listing_controls(&mut self, ui: &mut egui::Ui, file: &Attachment) {
        let mut next = file.clone();
        let model = next.listing.as_mut().unwrap();
        let document = &self.drawing.latex.listing_styles;
        let global = &self.latex.listing_styles;
        let mut style = model.style(document, global);
        ui.label(&file.original_name);
        ui.weak("Read-only source");
        ui.add_enabled_ui(!self.drawing.is_locked(file.id), |ui| {
            let lang = Self::listing_language(file, model, &style).to_owned();
            crate::ui::listing_controls::choice(ui, ("listing-language", file.id), lang, |ui| {
                ui.selectable_value(&mut model.language, None, "Automatic / style");
                for &(language, _) in listing::LANGUAGES {
                    ui.selectable_value(&mut model.language, Some(language.into()), language);
                }
            });
            let names: std::collections::BTreeSet<_> = listing::PRESETS
                .into_iter()
                .chain(global.keys().map(String::as_str))
                .chain(document.keys().map(String::as_str))
                .collect();
            let caption = if model.overrides.is_some() {
                format!("{} · Overrides", model.style_name)
            } else {
                model.style_name.clone()
            };
            crate::ui::listing_controls::choice(ui, ("listing-style", file.id), caption, |ui| {
                for name in names {
                    let scope = if document.contains_key(name) {
                        " · Document"
                    } else if global.contains_key(name) {
                        " · Global"
                    } else {
                        ""
                    };
                    if ui
                        .selectable_label(
                            model.style_name == name && model.overrides.is_none(),
                            format!("{name}{scope}"),
                        )
                        .clicked()
                    {
                        model.style_name = name.into();
                        model.fallback = listing::effective(name, document, global).unwrap();
                        model.overrides = None;
                        style = model.fallback.clone();
                    }
                }
            });
            let before = style.clone();
            egui::ScrollArea::vertical()
                .max_height(300.)
                .show(ui, |ui| {
                    crate::ui::listing_controls::style_fields(ui, &mut style);
                });
            if style != before && style.valid() {
                model.overrides = Some(style.clone());
            }
            if model.overrides.is_some() && ui.button("Reset Individual Overrides").clicked() {
                model.overrides = None;
            }
        });
        if next != *file && self.drawing.replace_attachment(next) {
            self.selection.refresh(&self.drawing);
            ui.ctx().request_repaint();
        }
    }
}

fn cache_source<'a>(
    cache: &'a mut HashMap<(AssetId, String), SourcePreview>,
    key: (AssetId, String),
    asset: &Arc<BinaryAsset>,
    source: &str,
    language: &str,
) -> &'a SourcePreview {
    if cache.len() >= 24 && !cache.contains_key(&key) {
        cache.retain(|_, entry| entry.asset.strong_count() != 0);
        if cache.len() >= 24 {
            cache.clear();
        }
    }
    cache
        .entry(key)
        .or_insert_with(|| SourcePreview::new(asset, source, language))
}

pub(super) type InspectorGalley = ((AssetId, String, Style, usize, f32), Arc<egui::Galley>);

#[allow(clippy::too_many_arguments)]
pub(super) fn paint(
    cache: &mut HashMap<(AssetId, String), SourcePreview>,
    layout: &mut ListingLayout,
    painter: &egui::Painter,
    view: Transform,
    file: &Attachment,
    delta: Placement,
    style: &Style,
    language: &str,
    scroll: f32,
) {
    let placement = file.placement.then(delta, [0.; 2]);
    let to_screen = |point| {
        let xy = view.to_screen(placement.apply(point));
        Pos2::new(xy[0] as f32, xy[1] as f32)
    };
    let [w, h] = file.dimensions;
    let corners = [[0., 0.], [w, 0.], [w, h], [0., h]].map(to_screen);
    if !painter.clip_rect().intersects(Rect::from_points(&corners)) {
        return;
    }
    let [r, g, b] = style.background;
    let background = egui::Color32::from_rgba_unmultiplied(r, g, b, style.opacity);
    painter.add(egui::Shape::convex_polygon(
        corners.to_vec(),
        background,
        egui::Stroke::NONE,
    ));
    let scale = (view.scale * placement.scale) as f32;
    let rule = egui::Stroke::new(style.thickness * scale, rgb(style.rule));
    for (a, b) in match style.frame {
        Frame::None => &[][..],
        Frame::Single => &[(0, 1), (1, 2), (2, 3), (3, 0)],
        Frame::Lines => &[(0, 1), (2, 3)],
        Frame::Left => &[(3, 0)],
    } {
        painter.line_segment([corners[*a], corners[*b]], rule);
    }
    if scale * style.size < 3. || scale * style.size > 768. {
        return;
    }
    let key = (file.asset.id(), language.to_owned());
    let Some(source) = file.asset.listing_source() else {
        return;
    };
    let preview = cache_source(cache, key, &file.asset, source, language);
    let local = |x, y| {
        to_screen([
            if placement.reflected[0] { w - x } else { x },
            if placement.reflected[1] { h - y } else { y },
        ])
    };
    let draw = |x: f32, y: f32, galley: Arc<egui::Galley>| {
        painter.add(
            egui::epaint::TextShape::new(
                local(f64::from(x), f64::from(y)),
                galley,
                rgb(style.foreground),
            )
            .with_angle(placement.rotation as f32),
        );
    };
    let first = (scroll.max(0.) as usize).min(preview.line_count.saturating_sub(1));
    let key = (
        file.asset.id(),
        format!("{}\0{language}", file.original_name),
        style.clone(),
        [w as f32, h as f32],
        scale,
        first,
    );
    if layout.key.as_ref() != Some(&key) {
        layout.key = Some(key);
        layout.glyphs.clear();
        populate_layout(
            layout, painter, file, style, language, preview, source, first, scale,
        );
    }
    for (x, y, galley) in &layout.glyphs {
        draw(*x, *y, galley.clone());
    }
}

#[allow(clippy::too_many_arguments)]
fn populate_layout(
    layout: &mut ListingLayout,
    painter: &egui::Painter,
    file: &Attachment,
    style: &Style,
    language: &str,
    preview: &SourcePreview,
    source: &str,
    first: usize,
    scale: f32,
) {
    let [w, h] = file.dimensions;
    let left = style.padding + style.left;
    let right = w as f32 - style.padding - style.right;
    let bottom = h as f32 - style.padding;
    let mut y = style.padding;
    let line_height = style.size * style.spacing;
    if right <= left || bottom < y + line_height {
        return;
    }
    let label = if style.header {
        format!("{}  ·  {language}", file.original_name)
    } else {
        String::new()
    };
    for text in [&label, &style.caption]
        .into_iter()
        .filter(|s| !s.is_empty())
    {
        if bottom < y + line_height {
            return;
        }
        let mut job = line_job(text, &[], style, (right - left) * scale, 1, scale);
        job.wrap.max_rows = 1;
        layout.glyphs.push((left, y, painter.layout_job(job)));
        y += line_height + 3.;
    }
    let gutter = if style.numbers == Numbers::None {
        0.
    } else {
        let digits = style
            .first
            .saturating_add(preview.line_count as u32)
            .max(1)
            .ilog10()
            .saturating_add(1);
        digits as f32 * style.number_size * 0.65 + style.number_sep
    };
    let body_x = if style.numbers == Numbers::Left {
        left + gutter
    } else {
        left
    };
    let body_width = (right - left - gutter) * scale;
    if body_width <= 1. {
        return;
    }
    for (offset, (line, tokens)) in preview.visible_from(source, first).take(512).enumerate() {
        let index = first + offset;
        let rows = ((bottom - y) / line_height).floor().clamp(0., 512.) as usize;
        if rows == 0 {
            break;
        }
        let galley = painter.layout_job(line_job(line, tokens, style, body_width, rows, scale));
        let height = (galley.size().y / scale).max(line_height);
        if y + height > bottom + 0.5 {
            break;
        }
        let number = style.first.saturating_add(index as u32);
        if style.numbers != Numbers::None
            && y + style.number_size <= bottom
            && (index as u32).is_multiple_of(style.step)
        {
            let galley = painter.layout_no_wrap(
                number.to_string(),
                font(style, style.number_size * scale),
                rgb(style.number_color),
            );
            let x = if style.numbers == Numbers::Left {
                body_x - style.number_sep - galley.size().x / scale
            } else {
                right - galley.size().x / scale
            };
            layout.glyphs.push((x, y, galley));
        }
        layout.glyphs.push((body_x, y, galley));
        y += height;
    }
}

pub(crate) fn listing_style_preview(ui: &mut egui::Ui, style: &Style) {
    const SAMPLE: &str = "def square(x):\n    return x ** 2  # Area\n";
    let background = egui::Color32::from_rgba_unmultiplied(
        style.background[0],
        style.background[1],
        style.background[2],
        style.opacity,
    );
    let (response, painter) = ui.allocate_painter(
        egui::vec2(
            ui.available_width(),
            (style.size * style.spacing * 3. + style.padding * 2.).clamp(52., 190.),
        ),
        egui::Sense::hover(),
    );
    let rect = response.rect;
    painter.rect_filled(rect, 0., background);
    let rule = egui::Stroke::new(style.thickness, rgb(style.rule));
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ];
    for (a, b) in match style.frame {
        Frame::None => &[][..],
        Frame::Single => &[(0, 1), (1, 2), (2, 3), (3, 0)],
        Frame::Lines => &[(0, 1), (2, 3)],
        Frame::Left => &[(3, 0)],
    } {
        painter.line_segment([corners[*a], corners[*b]], rule);
    }
    static LINES: OnceLock<Vec<SourceLine>> = OnceLock::new();
    let lines = LINES.get_or_init(|| parse_preview(SAMPLE, "Python"));
    let inset = style.padding.min(rect.width() * 0.25);
    let gutter = if style.numbers == Numbers::None {
        0.
    } else {
        style.number_size + style.number_sep
    };
    let area = rect.shrink(inset);
    let painter = painter.with_clip_rect(painter.clip_rect().intersect(area));
    let mut y = area.top();
    for (i, line) in lines.iter().enumerate() {
        let x = area.left()
            + if style.numbers == Numbers::Left {
                gutter
            } else {
                0.
            };
        let galley = painter.layout_job(line_job(
            &SAMPLE[line.range.clone()],
            &line.tokens,
            style,
            (area.width() - gutter).max(1.),
            4,
            1.,
        ));
        if style.numbers != Numbers::None && (i as u32).is_multiple_of(style.step) {
            let x = if style.numbers == Numbers::Left {
                area.left()
            } else {
                area.right() - gutter
            };
            painter.text(
                egui::pos2(x, y),
                egui::Align2::LEFT_TOP,
                style.first.saturating_add(i as u32),
                font(style, style.number_size),
                rgb(style.number_color),
            );
        }
        let height = galley.size().y.max(style.size * style.spacing);
        painter.galley(egui::pos2(x, y), galley, rgb(style.foreground));
        y += height;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        canvas::selection_ui::tests::setup,
        document::DocumentKind,
        image_import::Target,
        object::{CanvasObject, ObjectType},
        object_clipboard::ObjectBundle,
    };

    #[test]
    fn highlighting_uses_offline_grammars_without_changing_text() {
        for (language, text) in [
            ("Python", "def f(x): return 'value' # note\n"),
            ("Rust", "fn main() { let x = 42; } // note\n"),
            ("C++", "int main() { return 0; }\n"),
            ("JSON", "{\"key\": 42, \"enabled\": true}\n"),
            ("LaTeX", r"\textbf{Hello} % note"),
            ("Markdown", "# Heading\n**strong**\n"),
            ("TypeScript", "const value: number = 42;\n"),
        ] {
            let lines = parse_preview(text, language);
            assert!(
                lines
                    .iter()
                    .flat_map(|line| &line.tokens)
                    .any(|(_, token)| *token != Token::Text),
                "{language}"
            );
            for line in lines {
                let displayed = &text[line.range];
                for (span, _) in line.tokens {
                    assert!(displayed.get(span).is_some());
                }
            }
        }
        assert!(
            parse_preview("fn main()", "Plain Text")[0]
                .tokens
                .is_empty()
        );
        let _ = parse_preview("let \"unterminated λ", "Rust");
    }

    #[test]
    fn listing_history_resize_and_cross_document_clipboard_preserve_asset() {
        let (mut board, surface) = setup(DocumentKind::Whiteboard);
        let target = Target {
            surface,
            position: [230., 250.],
            replace: None,
        };
        let original = BinaryAsset::new((&b"  let x = 1;\r\n"[..]).into()).unwrap();
        board.insert_file_object(target, "first.rs".into(), original.clone(), Some(false));
        let file = board.selected_attachment().unwrap();
        assert_eq!(file.object_type(), ObjectType::CodeListing);
        let resized = file.resized(4, file.handles()[4].map(|v| v + 20.));
        assert_ne!(resized.dimensions, file.dimensions);
        assert_eq!(resized.listing, file.listing);
        assert!(board.drawing.replace_attachment(resized.clone()));
        let replacement = BinaryAsset::new((&b"print('replacement')\r\n"[..]).into()).unwrap();
        board.insert_file_object(
            Target {
                replace: Some(file.id),
                ..target
            },
            "second.py".into(),
            replacement.clone(),
            Some(false),
        );
        let new_file = board.attachment(file.id).unwrap();
        assert_eq!(new_file.dimensions, resized.dimensions);
        assert_eq!(new_file.placement, resized.placement);
        assert_eq!(new_file.asset.bytes(), replacement.bytes());
        assert_eq!(listing::detect(&new_file.original_name), "Python");
        board.undo();
        assert_eq!(
            board.attachment(file.id).unwrap().asset.bytes(),
            original.bytes()
        );
        board.redo();
        assert_eq!(
            board.attachment(file.id).unwrap().asset.bytes(),
            replacement.bytes()
        );
        let invalid = BinaryAsset::new((&b"\0bad"[..]).into()).unwrap();
        board.insert_file_object(
            Target {
                replace: Some(file.id),
                ..target
            },
            "invalid.dat".into(),
            invalid,
            Some(false),
        );
        assert_eq!(
            board.attachment(file.id).unwrap().asset.bytes(),
            replacement.bytes()
        );

        let (mut page, page_surface) = setup(DocumentKind::Page);
        let bundle = ObjectBundle::capture_objects(std::iter::once(
            board.effective_listing_for_clipboard(&new_file.as_ref().clone().into()),
        ))
        .unwrap();
        let pasted = ObjectBundle::decode(&bundle.encode().unwrap())
            .unwrap()
            .paste(&mut page.drawing, page_surface, [260., 300.]);
        assert_eq!(pasted.len(), 1);
        let copy = page.attachment(pasted[0]).unwrap();
        assert_ne!(copy.id, file.id);
        assert_eq!(copy.original_name, "second.py");
        assert_eq!(copy.asset.bytes(), replacement.bytes());
    }

    #[test]
    fn large_preview_keeps_lines_beyond_the_highlight_budget_accessible() {
        let bytes = format!("{}\n// final line\n", "x\n".repeat(12_000));
        let asset = BinaryAsset::new(bytes.as_bytes().into()).unwrap();
        let source = asset.listing_source().unwrap();
        let preview = SourcePreview::new(&asset, source, "Rust");
        assert!(preview.lines.len() <= 8192);
        assert_eq!(
            preview.visible_from(source, 12_001).next().unwrap().0,
            "// final line"
        );
        assert_eq!(preview.visible_from(source, 12_002).next().unwrap().0, "");
    }

    #[test]
    fn inspector_text_buffer_cannot_accept_insertions_or_deletions() {
        use egui::TextBuffer;
        let mut source = "let π = 3;\n";
        assert!(!source.is_mutable());
        assert_eq!(
            source.insert_text("replacement", egui::text::CharIndex(0)),
            0
        );
        source.delete_char_range(egui::text::CharIndex(0)..egui::text::CharIndex(3));
        assert_eq!(source, "let π = 3;\n");
    }
}
