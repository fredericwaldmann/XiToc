//! Read-only source listing configuration and rendering.
use super::*;
use crate::{
    asset::{AssetId, BinaryAsset},
    attachment::Attachment,
    code_listing::{self as listing, Frame, Numbers, Style},
    object::{ObjectBounds, Placement},
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
    #[cfg(test)]
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
            line_count: listing::line_count(source),
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

#[derive(PartialEq)]
struct LayoutKey {
    asset: AssetId,
    name: String,
    style: Style,
    line_ranges: String,
    title: Option<String>,
    width: f32,
    scale: f32,
    pixels_per_point: f32,
}

#[derive(Default)]
pub(super) struct ListingLayout {
    key: Option<LayoutKey>,
    glyphs: Vec<(f32, f32, Arc<egui::Galley>)>,
    dimensions: [f64; 2],
    max_glyph_height: f32,
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
            SourceLine {
                #[cfg(test)]
                range,
                tokens,
            }
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
    scale: f32,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = width;
    job.wrap.break_anywhere = !style.break_at_space;
    let end = source.len();
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

pub(super) struct ListingDraft {
    id: Uuid,
    token: Uuid,
    model: listing::Listing,
    path: String,
    selected_file: Option<(String, Arc<BinaryAsset>, Option<String>)>,
    style_name: Option<String>,
    error: Option<String>,
    applying_path: bool,
}

#[cfg(not(target_arch = "wasm32"))]
type LinkResult = Result<
    Option<(
        crate::attachment_io::LinkStamp,
        crate::attachment_io::FileData,
    )>,
    String,
>;

#[cfg(not(target_arch = "wasm32"))]
type WatchResults = Vec<(String, LinkResult)>;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
pub(super) struct ListingWatch {
    next: f64,
    pending: Option<std::sync::mpsc::Receiver<WatchResults>>,
    reconciled: Option<u64>,
    files: HashMap<String, WatchedFile>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
struct WatchedFile {
    stamp: Option<crate::attachment_io::LinkStamp>,
    data: Option<crate::attachment_io::FileData>,
    unavailable: bool,
}

impl Canvas {
    pub(crate) fn choose_listing(&mut self, ctx: &egui::Context, replace: Option<Uuid>) {
        self.finish();
        let scope = if replace.is_some() {
            self.listing_config.as_ref().map(|draft| draft.token)
        } else {
            self.drawing.editing_group
        };
        self.attachment_io
            .pick(ctx.clone(), self.image_target(replace), scope, Some(false));
    }

    fn listing_preset(&self, name: &str) -> Option<Style> {
        listing::effective(
            name,
            &self.drawing.latex.listing_styles,
            &self.latex.listing_styles,
        )
    }

    fn preset_text(&self, name: &str) -> String {
        self.latex
            .listing_configurations
            .get(name)
            .cloned()
            .unwrap_or_else(|| listing::preset_configuration(name))
    }

    pub(super) fn default_listing(&self) -> listing::Listing {
        let preferred = &self.latex.listing_default;
        let name = if self.listing_preset(preferred).is_some() {
            preferred.as_str()
        } else {
            listing::LIGHT
        };
        listing::Listing {
            style_name: name.into(),
            configuration: self.preset_text(name),
            fallback: self.listing_preset(name).unwrap(),
            overrides: None,
            ..Default::default()
        }
    }

    pub(super) fn open_listing(&mut self, id: Uuid) {
        let Some(mut model) = self.attachment(id).and_then(|file| file.listing.clone()) else {
            return;
        };
        if model.configuration.is_empty() {
            if let Some(style) = model.overrides.take() {
                model.fallback = style;
            }
            model.configuration = r"\lstset{}".into();
        }
        self.listing_config = Some(ListingDraft {
            id,
            token: Uuid::new_v4(),
            path: model.source_path.clone().unwrap_or_default(),
            model,
            selected_file: None,
            style_name: None,
            error: None,
            applying_path: false,
        });
    }

    pub(super) fn listing_file_selected(
        &mut self,
        token: Option<Uuid>,
        result: Result<crate::attachment_io::FileData, String>,
        path: Option<String>,
    ) {
        let Some(draft) = self
            .listing_config
            .as_mut()
            .filter(|d| Some(d.token) == token)
        else {
            return;
        };
        match result {
            Ok((name, asset)) if asset.listing_text() => {
                draft.path = path.clone().unwrap_or_default();
                draft.selected_file = Some((name, asset, path));
                draft.error = None;
            }
            Ok(_) => {
                draft.error = Some("Choose a valid UTF-8 source file.".into());
                draft.applying_path = false;
            }
            Err(error) => {
                draft.error = Some(error);
                draft.applying_path = false;
            }
        }
    }

    fn draft_style(&self, draft: &ListingDraft) -> Result<Style, String> {
        // The selected preset is a snapshot. Reapplying an unchanged draft uses its
        // stored appearance even after a named preference is updated or removed.
        let base = draft.model.fallback.clone();
        let input = &draft.model.configuration;
        listing::parse(input, base.clone(), |name| {
            if name == draft.model.style_name {
                Some(base.clone())
            } else {
                self.listing_preset(name)
            }
        })
        .map(|(_, style)| style)
    }

    pub(super) fn listing_dialog(&mut self, ctx: &egui::Context) -> bool {
        self.listing_preview = None;
        let Some(mut draft) = self.listing_config.take() else {
            return false;
        };
        let Some(file) = self.attachment(draft.id) else {
            return false;
        };
        let locked = self.drawing.is_locked(file.id);
        let candidate = draft
            .selected_file
            .as_ref()
            .map(|(_, asset, _)| asset)
            .unwrap_or(&file.asset);
        let count = candidate.listing_line_count();
        let ranges = listing::parse_ranges(&draft.model.line_ranges, Some(count));
        let style = self.draft_style(&draft);
        let filename = draft
            .selected_file
            .as_ref()
            .map_or(file.original_name.as_str(), |value| value.0.as_str())
            .to_owned();
        let mut browse = false;
        let mut apply =
            draft.applying_path && !self.attachment_io.picker_open && draft.error.is_none();
        let mut cancel = false;
        let mut preset_change = None;
        let mut save = false;
        let mut set_default = false;
        let mut names: std::collections::BTreeSet<_> = listing::PRESETS.into_iter().collect();
        names.extend(self.latex.listing_styles.keys().map(String::as_str));
        names.extend(self.drawing.latex.listing_styles.keys().map(String::as_str));
        let response = crate::ui::dialog::configuration(
            ctx,
            egui::Id::unique(("code-listing", file.id)),
            "Code Listing",
            [660., 650.],
            |ui| {
                ui.set_width(
                    (ui.ctx().content_rect().width() - 48.)
                        .clamp(180., crate::ui::style::SETTINGS_WIDTH),
                );
                ui.horizontal(|ui| {
                    ui.heading("Code Listing");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.ctx().viewport_id() == egui::ViewportId::ROOT {
                            cancel |= ui.button("×").on_hover_text("Close").clicked();
                        }
                    });
                });
                egui::ScrollArea::vertical().max_height((ui.ctx().content_rect().height() - 160.).max(60.)).show(ui, |ui| {
                ui.label("Source File");
                ui.horizontal(|ui| {
                    #[cfg(not(target_arch = "wasm32"))]
                    ui.add(egui::TextEdit::singleline(&mut draft.path)
                        .hint_text(filename.clone()).desired_width((ui.available_width() - 72.).max(60.)));
                    #[cfg(target_arch = "wasm32")]
                    ui.label(&filename);
                    browse = ui.add_enabled(!locked && !self.attachment_io.picker_open, egui::Button::new("Browse…")).clicked();
                });
                #[cfg(target_arch = "wasm32")]
                ui.weak("Automatic synchronization is unavailable in this browser. Browse to refresh the embedded copy.");
                #[cfg(not(target_arch = "wasm32"))]
                if let Some(path) = file.listing.as_ref().and_then(|model| model.source_path.as_deref()) {
                    if self.listing_watch.files.get(path).is_some_and(|f| f.unavailable) {
                        ui.weak("Source file unavailable or invalid. Showing embedded copy.");
                    } else if !crate::attachment_io::approved_link(path) {
                        ui.weak("Browse to reconnect this source. Showing embedded copy.");
                    }
                }
                ui.label("Style Preset");
                egui::ComboBox::from_id_salt("listing-preset").selected_text(&draft.model.style_name)
                    .show_ui(ui, |ui| {
                        for name in &names {
                            if ui.selectable_label(draft.model.style_name == *name, *name).clicked() {
                                preset_change = Some((*name).to_owned());
                            }
                        }
                    });
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Save Custom Style").clicked() { draft.style_name.get_or_insert_with(String::new); }
                    set_default = ui.add_enabled((listing::PRESETS.contains(&draft.model.style_name.as_str()) || self.latex.listing_styles.contains_key(&draft.model.style_name)) && style.is_ok(), egui::Button::new("Set as Default")).clicked();
                });
                if let Some(name) = &mut draft.style_name {
                    ui.horizontal(|ui| {
                        ui.label("Style name");
                        ui.add(egui::TextEdit::singleline(name).char_limit(80));
                        save = ui.add_enabled(style.is_ok(), egui::Button::new("Save")).clicked();
                    });
                }
                ui.label("LaTeX Listings Configuration");
                ui.add(egui::TextEdit::multiline(&mut draft.model.configuration).code_editor().desired_rows(6).desired_width(f32::INFINITY));
                ui.label("Source Lines to Display");
                ui.add(egui::TextEdit::singleline(&mut draft.model.line_ranges).hint_text("All lines · e.g. 1-20, 25-32, 35, 37").desired_width(f32::INFINITY));
                ui.checkbox(&mut draft.model.show_title, "Show Title");
                if draft.model.show_title {
                    ui.label("Title");
                    ui.add(egui::TextEdit::singleline(&mut draft.model.custom_title).hint_text(&filename).char_limit(128).desired_width(f32::INFINITY));
                }
                if file.corners().iter().any(|&p| !self.drawing.contains(file.surface, p)) {
                    ui.weak("Listing exceeds the page. Reduce the line selection or adjust its width, font size, or placement.");
                }
                if let Some(error) = draft.error.as_ref().or_else(|| style.as_ref().err()).or_else(|| ranges.as_ref().err()) {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
                if locked { ui.weak("Unlock this listing to apply changes."); }
            });
                ui.horizontal(|ui| {
                    cancel |= ui.button("Cancel").clicked();
                    apply |= ui
                        .add_enabled(
                            !locked && !self.attachment_io.picker_open,
                            egui::Button::new("Apply"),
                        )
                        .clicked();
                });
                cancel |= ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
            },
        );
        if cancel || response {
            return true;
        }
        if let Some(name) = preset_change {
            draft.model.fallback = self.listing_preset(&name).unwrap();
            draft.model.overrides = None;
            draft.model.configuration = self.preset_text(&name);
            draft.model.style_name = name;
            draft.error = None;
        }
        if save
            && let (Some(name), Ok(style)) = (draft.style_name.clone(), self.draft_style(&draft))
        {
            let name = name.trim().to_owned();
            if !listing::valid_name(&name)
                || names.contains(name.as_str())
                || self.latex.listing_styles.len() >= 64
            {
                draft.error =
                    Some("Use an unused style name. Up to 64 custom styles are supported.".into());
            } else {
                self.latex
                    .listing_styles
                    .insert(name.clone(), style.clone());
                self.latex
                    .listing_configurations
                    .insert(name.clone(), draft.model.configuration.clone());
                draft.model.style_name = name;
                draft.model.fallback = style;
                draft.model.overrides = None;
                draft.style_name = None;
                draft.error = None;
            }
        }
        if set_default {
            self.latex.listing_default = draft.model.style_name.clone();
        }
        if apply && !locked {
            draft.applying_path = false;
            #[cfg(not(target_arch = "wasm32"))]
            let read_path = draft.path.trim()
                != draft
                    .selected_file
                    .as_ref()
                    .and_then(|f| f.2.as_deref())
                    .or_else(|| file.listing.as_ref().and_then(|f| f.source_path.as_deref()))
                    .unwrap_or_default();
            #[cfg(not(target_arch = "wasm32"))]
            if read_path && !draft.path.trim().is_empty() {
                self.attachment_io.listing_path(
                    ctx.clone(),
                    self.image_target(Some(file.id)),
                    draft.token,
                    draft.path.trim().to_owned(),
                );
                draft.applying_path = true;
                draft.error = None;
                self.listing_config = Some(draft);
                return true;
            }
            match self.configured_listing(&file, &draft, count) {
                Err(error) => draft.error = Some(error),
                Ok(next) => {
                    if next == *file || self.drawing.replace_attachment(next) {
                        #[cfg(not(target_arch = "wasm32"))]
                        if let Some((_, _, Some(path))) = &draft.selected_file {
                            self.listing_watch.files.remove(path);
                            self.listing_watch.pending = None;
                            self.listing_watch.next = 0.;
                        }
                        self.selection.refresh(&self.drawing);
                        ctx.request_repaint();
                        return true;
                    }
                    draft.error = Some("Could not apply listing changes. Check permissions, title, and asset limits.".into());
                }
            }
        }
        if let Ok(preview) = self.configured_listing(&file, &draft, count) {
            let mut preview = preview;
            let painter = ctx.layer_painter(egui::LayerId::background());
            let layout = self.listing_measurements.entry(preview.id).or_default();
            prepare_layout(&mut self.listing_cache, layout, &painter, &preview, 1.);
            preview.dimensions = layout.dimensions;
            layout.glyphs.clear();
            self.listing_preview = Some(preview);
        }
        self.listing_config = Some(draft);
        if browse {
            self.choose_listing(ctx, Some(file.id));
        }
        true
    }

    fn configured_listing(
        &self,
        file: &Attachment,
        draft: &ListingDraft,
        count: usize,
    ) -> Result<Attachment, String> {
        let style = self.draft_style(draft)?;
        let ranges = listing::parse_ranges(&draft.model.line_ranges, Some(count))?;
        let mut next = file.clone();
        let mut model = draft.model.clone();
        if let Some((name, asset, path)) = &draft.selected_file {
            next.original_name = name.clone();
            next.display_name = name.clone();
            next.asset = asset.clone();
            model.source_path = path.clone();
        }
        #[cfg(not(target_arch = "wasm32"))]
        if draft.path.trim().is_empty() {
            model.source_path = None;
        }
        model.line_ranges = listing::format_ranges(&ranges);
        model.overrides = Some(style);
        model.language = None;
        if !model.valid() {
            return Err("Invalid title, source path, or listing configuration.".into());
        }
        next.listing = Some(model);
        Ok(next)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn poll_listing_sources(&mut self, ui: &egui::Ui) {
        let watch = &mut self.listing_watch;
        if let Some(results) = watch.pending.as_ref().and_then(|r| r.try_recv().ok()) {
            watch.pending = None;
            watch.reconciled = None;
            for (path, value) in results {
                let file = watch.files.entry(path).or_default();
                match value {
                    Ok(Some((stamp, data))) => {
                        file.stamp = Some(stamp);
                        file.data = Some(data);
                        file.unavailable = false;
                    }
                    Ok(None) => file.unavailable = false,
                    Err(_) => {
                        file.unavailable = true;
                        file.stamp = None;
                    }
                }
            }
        }
        let now = ui.input(|i| i.time);
        let due = watch.pending.is_none() && now >= watch.next;
        let reconcile =
            self.selection_gesture.is_none() && watch.reconciled != Some(self.drawing.revision());
        if !due && !reconcile {
            return;
        }
        let links = self
            .drawing
            .items()
            .iter()
            .filter_map(|object| {
                let ObjectSnapshot::Attachment(file) = object else {
                    return None;
                };
                let path = file.listing.as_ref()?.source_path.as_deref()?;
                Some((file.clone(), path.to_owned()))
            })
            .collect::<Vec<_>>();
        if links.is_empty() {
            watch.next = now + 0.85;
            watch.files.clear();
            watch.reconciled = Some(self.drawing.revision());
            return;
        }
        if reconcile {
            for (file, path) in &links {
                if let Some((name, asset)) = watch.files.get(path).and_then(|f| f.data.as_ref())
                    && file.asset.id() != asset.id()
                {
                    let mut next = (**file).clone();
                    next.original_name = name.clone();
                    next.display_name = name.clone();
                    next.asset = asset.clone();
                    if !self.drawing.sync_listing(next)
                        && let Some(watched) = watch.files.get_mut(path)
                    {
                        watched.unavailable = true;
                    }
                }
            }
            watch.reconciled = Some(self.drawing.revision());
        }
        if due {
            watch.next = now + 0.85;
            let paths: HashSet<_> = links
                .into_iter()
                .map(|(_, path)| path)
                .filter(|path| crate::attachment_io::approved_link(path))
                .collect();
            watch.files.retain(|path, _| paths.contains(path));
            if !paths.is_empty() {
                let jobs = paths
                    .into_iter()
                    .map(|path| {
                        let stamp = watch.files.get(&path).and_then(|file| file.stamp.clone());
                        (path, stamp)
                    })
                    .collect::<Vec<_>>();
                let (tx, rx) = std::sync::mpsc::channel();
                watch.pending = Some(rx);
                let ctx = ui.ctx().clone();
                std::thread::spawn(move || {
                    let results = jobs
                        .into_iter()
                        .map(|(path, stamp)| {
                            let result = crate::attachment_io::poll_link(&path, stamp);
                            (path, result)
                        })
                        .collect();
                    let _ = tx.send(results);
                    ctx.request_repaint();
                });
            }
        }
        if !watch.files.is_empty() || watch.pending.is_some() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs_f64(
                    (watch.next - now).max(0.1),
                ));
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

impl Canvas {
    /// Extend the editor's scrollable margin when an intact listing exceeds its paper.
    pub(crate) fn page_listing_bounds(&self, surface: Surface, paper: [f64; 2]) -> ObjectBounds {
        let mut bounds = ObjectBounds {
            min: [0.; 2],
            max: paper,
        };
        for object in self.drawing.items() {
            let ObjectSnapshot::Attachment(file) = object else {
                continue;
            };
            if file.listing.is_none() || file.surface != surface {
                continue;
            }
            let file = self
                .listing_preview
                .as_ref()
                .filter(|f| f.id == file.id)
                .unwrap_or(file);
            if let Some(extents) = file.world_bounds() {
                bounds = bounds.union(extents);
            }
        }
        bounds
    }

    pub(super) fn measure_listings(&mut self, ui: &egui::Ui) {
        if self.selection_gesture.is_some() {
            return;
        }
        let dpi = ui.ctx().pixels_per_point();
        if self.listing_measurement_key == Some((self.drawing.revision(), dpi)) {
            return;
        }
        let files = self
            .drawing
            .items()
            .iter()
            .filter_map(|object| match object {
                ObjectSnapshot::Attachment(file) if file.listing.is_some() => Some(file.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        self.listing_measurements
            .retain(|id, _| self.drawing.snapshot(*id).is_some());
        for file in files {
            let layout = self.listing_measurements.entry(file.id).or_default();
            prepare_layout(&mut self.listing_cache, layout, ui.painter(), &file, 1.);
            layout.glyphs.clear();
            if layout.dimensions != file.dimensions {
                let mut next = (*file).clone();
                next.dimensions = layout.dimensions;
                self.drawing.sync_listing(next);
            }
        }
        self.listing_measurement_key = Some((self.drawing.revision(), dpi));
    }
}

pub(super) fn fit_resize(
    cache: &mut HashMap<(AssetId, String), SourcePreview>,
    layout: &mut ListingLayout,
    painter: &egui::Painter,
    file: &mut Attachment,
    moving_handle: usize,
) {
    if file.listing.is_none() {
        return;
    }
    let opposite = (moving_handle + 4) % 8;
    let fixed = file.handles()[opposite];
    prepare_layout(cache, layout, painter, file, 1.);
    file.dimensions = layout.dimensions;
    layout.glyphs.clear();
    let moved = file.handles()[opposite];
    for axis in 0..2 {
        file.placement.position[axis] += fixed[axis] - moved[axis];
    }
}

fn prepare_layout(
    cache: &mut HashMap<(AssetId, String), SourcePreview>,
    layout: &mut ListingLayout,
    painter: &egui::Painter,
    file: &Attachment,
    scale: f32,
) {
    let model = file.listing.as_ref().unwrap();
    let style = model.overrides.as_ref().unwrap_or(&model.fallback);
    let width = model.preferred_width.unwrap_or(342.) as f32;
    let key = LayoutKey {
        asset: file.asset.id(),
        name: file.original_name.clone(),
        style: style.clone(),
        line_ranges: model.line_ranges.clone(),
        title: model.show_title.then(|| model.custom_title.clone()),
        width,
        scale,
        pixels_per_point: painter.ctx().pixels_per_point(),
    };
    if layout.key.as_ref() == Some(&key) {
        return;
    }
    let Some(source) = file.asset.listing_source() else {
        return;
    };
    let language = listing::detect(&file.original_name);
    let preview = cache_source(
        cache,
        (file.asset.id(), language.to_owned()),
        &file.asset,
        source,
        language,
    );
    let count = preview.line_count;
    let ranges = listing::parse_ranges(&model.line_ranges, None).unwrap_or_default();
    let ranges = if ranges.is_empty() {
        vec![(1, count)]
    } else {
        ranges
    };
    let left = style.padding + style.left;
    let right = style.padding + style.right;
    let scale = scale.max(0.001);
    let gutter = if style.numbers == Numbers::None {
        0.
    } else {
        painter
            .layout_no_wrap(
                "9".repeat(count.max(1).ilog10() as usize + 1),
                font(style, style.number_size * scale),
                rgb(style.number_color),
            )
            .size()
            .x
            / scale
            + style.number_sep
    };
    let body_x = left
        + if style.numbers == Numbers::Left {
            gutter
        } else {
            0.
        };
    let line_height = (style.size * style.spacing).max(if style.numbers != Numbers::None {
        style.number_size
    } else {
        0.
    });
    let wrap_width = (width - left - right - gutter - 2.).max(style.size * 2.);
    let actual_width = if style.wrap {
        wrap_width * scale
    } else {
        f32::INFINITY
    };
    let mut widest: f32 = wrap_width;
    let mut y = style.padding;
    let mut glyphs = Vec::new();
    let mut number_glyphs = Vec::new();
    if model.show_title {
        let title = if !model.custom_title.trim().is_empty() {
            model.custom_title.as_str()
        } else if !style.caption.is_empty() {
            style.caption.as_str()
        } else {
            file.original_name.as_str()
        };
        let galley = painter.layout_job(line_job(title, &[], style, actual_width, scale));
        widest = widest.max(galley.size().x / scale);
        let height = (galley.size().y / scale).max(line_height);
        glyphs.push((left, y, galley));
        y += height + 3.;
    }
    for (start, end) in ranges {
        if start > count {
            break;
        }
        for (i, (line, tokens)) in preview
            .visible_from(source, start - 1)
            .take(end.min(count) - start + 1)
            .enumerate()
        {
            let number = start + i;
            let galley = painter.layout_job(line_job(line, tokens, style, actual_width, scale));
            widest = widest.max(galley.size().x / scale);
            let height = (galley.size().y / scale).max(line_height);
            if style.numbers != Numbers::None && (number - 1).is_multiple_of(style.step as usize) {
                let label = painter.layout_no_wrap(
                    number.to_string(),
                    font(style, style.number_size * scale),
                    rgb(style.number_color),
                );
                if style.numbers == Numbers::Left {
                    glyphs.push((body_x - style.number_sep - label.size().x / scale, y, label));
                } else {
                    number_glyphs.push((y, label));
                }
            }
            glyphs.push((body_x, y, galley));
            y += height;
        }
    }
    let width = (left + widest + gutter + right + 2.).max(width);
    for (y, galley) in number_glyphs {
        glyphs.push((width - right - galley.size().x / scale, y, galley));
    }
    glyphs.sort_by(|a, b| a.1.total_cmp(&b.1));
    layout.dimensions = [
        f64::from(width),
        f64::from((y + style.padding + 2.).max(16.)),
    ];
    layout.max_glyph_height = glyphs
        .iter()
        .map(|(_, _, galley)| galley.size().y / scale)
        .fold(0., f32::max);
    layout.glyphs = glyphs;
    layout.key = Some(key);
}

pub(super) fn paint(
    cache: &mut HashMap<(AssetId, String), SourcePreview>,
    layout: &mut ListingLayout,
    painter: &egui::Painter,
    view: Transform,
    file: &Attachment,
    delta: Placement,
) {
    let model = file.listing.as_ref().unwrap();
    let style = model.overrides.as_ref().unwrap_or(&model.fallback);
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
    let scale = (view.scale * placement.scale) as f32;
    let text_visible = (3. ..=768.).contains(&(scale * style.size));
    if text_visible {
        prepare_layout(cache, layout, painter, file, scale);
    }
    let [w, h] = if text_visible {
        layout.dimensions
    } else {
        [w, h]
    };
    let corners = [[0., 0.], [w, 0.], [w, h], [0., h]].map(to_screen);
    let [r, g, b] = style.background;
    let background = egui::Color32::from_rgba_unmultiplied(r, g, b, style.opacity);
    painter.add(egui::Shape::convex_polygon(
        corners.to_vec(),
        background,
        egui::Stroke::NONE,
    ));
    let rule = egui::Stroke::new(style.thickness * scale, rgb(style.rule));
    for (a, b) in match style.frame {
        Frame::None => &[][..],
        Frame::Single => &[(0, 1), (1, 2), (2, 3), (3, 0)],
        Frame::Lines => &[(0, 1), (2, 3)],
        Frame::Left => &[(3, 0)],
    } {
        painter.line_segment([corners[*a], corners[*b]], rule);
    }
    if !text_visible {
        return;
    }
    let local = |x, y| {
        to_screen([
            if placement.reflected[0] { w - x } else { x },
            if placement.reflected[1] { h - y } else { y },
        ])
    };
    let clip = painter.clip_rect();
    let min = [f64::from(clip.min.x), f64::from(clip.min.y)];
    let max = [f64::from(clip.max.x), f64::from(clip.max.y)];
    let visible = [min, [max[0], min[1]], max, [min[0], max[1]]].map(|p| {
        let local = placement.inverse(view.to_document(p));
        (if placement.reflected[1] {
            h - local[1]
        } else {
            local[1]
        }) as f32
    });
    let low = visible.into_iter().fold(f32::INFINITY, f32::min);
    let high = visible.into_iter().fold(f32::NEG_INFINITY, f32::max);
    let first = layout
        .glyphs
        .partition_point(|(_, y, _)| *y + layout.max_glyph_height < low);
    for (x, y, galley) in &layout.glyphs[first..] {
        if *y > high {
            break;
        }
        if *y + galley.size().y / scale < low {
            continue;
        }
        painter.add(
            egui::epaint::TextShape::new(
                local(f64::from(*x), f64::from(*y)),
                galley.clone(),
                rgb(style.foreground),
            )
            .with_angle(placement.rotation as f32),
        );
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
        assert_eq!(
            resized.listing.as_ref().unwrap().preferred_width,
            Some(resized.dimensions[0])
        );
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
        let bundle =
            ObjectBundle::capture_objects(std::iter::once(new_file.as_ref().clone().into()))
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
    fn selected_source_lines_keep_original_numbers_and_grow_to_fit() {
        let ctx = egui::Context::default();
        crate::text_layout::install(&ctx);
        let (mut canvas, surface) = setup(DocumentKind::Whiteboard);
        let source: String = (1..=50).map(|n| format!("code_{n}\n")).collect();
        let asset = BinaryAsset::new(source.as_bytes().into()).unwrap();
        let mut file = Attachment::new(
            canvas.drawing.owner(),
            surface,
            [0.; 2],
            "listing.py".into(),
            asset,
        );
        let mut model = canvas.default_listing();
        model.line_ranges = "1-20, 25-32, 35, 37".into();
        file.listing = Some(model);
        let mut layout = ListingLayout::default();
        crate::ui::settings::tests::render(&ctx, vec![], |ui| {
            prepare_layout(
                &mut canvas.listing_cache,
                &mut layout,
                ui.painter(),
                &file,
                1.,
            );
            let expected = (1..=20).chain(25..=32).chain([35, 37]).collect::<Vec<_>>();
            let glyphs = &layout.glyphs;
            assert_eq!(glyphs[0].2.job.text, "listing.py");
            assert_eq!(glyphs.len(), expected.len() * 2 + 1);
            for (pair, n) in glyphs[1..].as_chunks::<2>().0.iter().zip(expected) {
                assert_eq!(pair[0].2.job.text, n.to_string());
                assert_eq!(pair[1].2.job.text, format!("code_{n}"));
            }
            assert!(layout.dimensions[1] > 198.);
            let with_title = layout.dimensions[1];
            file.listing.as_mut().unwrap().show_title = false;
            prepare_layout(
                &mut canvas.listing_cache,
                &mut layout,
                ui.painter(),
                &file,
                1.,
            );
            assert!(layout.dimensions[1] < with_title);
            assert_eq!(layout.glyphs[0].2.job.text, "1");
        });
    }

    #[test]
    fn unwrapped_long_line_is_fully_laid_out_and_expands_width() {
        let ctx = egui::Context::default();
        crate::text_layout::install(&ctx);
        let (mut canvas, surface) = setup(DocumentKind::Whiteboard);
        let source = "x".repeat(10_000);
        let asset = BinaryAsset::new(source.as_bytes().into()).unwrap();
        let mut file = Attachment::new(
            canvas.drawing.owner(),
            surface,
            [0.; 2],
            "listing.txt".into(),
            asset,
        );
        let mut model = canvas.default_listing();
        model.fallback.wrap = false;
        model.show_title = false;
        file.listing = Some(model);
        let mut layout = ListingLayout::default();
        crate::ui::settings::tests::render(&ctx, vec![], |ui| {
            prepare_layout(
                &mut canvas.listing_cache,
                &mut layout,
                ui.painter(),
                &file,
                1.,
            );
            assert_eq!(layout.glyphs.last().unwrap().2.job.text, source);
            let expanded = layout.dimensions;
            assert!(expanded[0] > 342.);
            file.listing.as_mut().unwrap().fallback.wrap = true;
            prepare_layout(
                &mut canvas.listing_cache,
                &mut layout,
                ui.painter(),
                &file,
                1.,
            );
            assert_eq!(layout.glyphs.last().unwrap().2.job.text, source);
            assert!(layout.dimensions[0] < expanded[0]);
            assert!(layout.dimensions[1] > expanded[1]);
        });
    }
    #[test]
    fn dialog_cancel_and_apply_preserve_transaction_and_refreshed_source() {
        use crate::ui::settings::tests::{pointer, render};
        let ctx = egui::Context::default();
        crate::text_layout::install(&ctx);
        let (mut canvas, surface) = setup(DocumentKind::Whiteboard);
        let asset = BinaryAsset::new((&b"first\nsecond\n"[..]).into()).unwrap();
        canvas.insert_file_object(
            Target {
                surface,
                position: [400., 300.],
                replace: None,
            },
            "test.py".into(),
            asset,
            Some(false),
        );
        let id = canvas.selected_attachment().unwrap().id;
        let initial = canvas.attachment(id).unwrap();
        canvas.open_listing(id);
        canvas.listing_config.as_mut().unwrap().model.custom_title = "Draft".into();
        let mut frame = |events| {
            render(&ctx, events, |ui| {
                canvas.listing_dialog(ui.ctx());
            })
        };
        frame(vec![]);
        let button = frame(vec![])["Cancel"];
        frame(pointer(button, true));
        frame(pointer(button, false));
        assert!(canvas.listing_config.is_none());
        assert_eq!(canvas.attachment(id).unwrap(), initial);

        canvas.open_listing(id);
        let draft = canvas.listing_config.as_mut().unwrap();
        draft.model.custom_title = "Committed".into();
        draft.model.line_ranges = "2".into();
        let mut refreshed = initial.as_ref().clone();
        refreshed.asset = BinaryAsset::new((&b"updated\nsource\nthird\n"[..]).into()).unwrap();
        assert!(canvas.drawing.sync_listing(refreshed.clone()));
        let mut frame = |events| {
            render(&ctx, events, |ui| {
                canvas.listing_dialog(ui.ctx());
            })
        };
        frame(vec![]);
        let button = frame(vec![])["Apply"];
        frame(pointer(button, true));
        frame(pointer(button, false));
        assert!(canvas.listing_config.is_none());
        let applied = canvas.attachment(id).unwrap();
        assert_eq!(applied.asset.bytes(), refreshed.asset.bytes());
        let model = applied.listing.as_ref().unwrap();
        assert_eq!(model.custom_title, "Committed");
        assert_eq!(model.line_ranges, "2");
        canvas.undo();
        let restored = canvas.attachment(id).unwrap();
        assert_eq!(restored.asset.bytes(), refreshed.asset.bytes());
        assert!(restored.listing.as_ref().unwrap().custom_title.is_empty());
    }

    #[test]
    fn new_listings_use_persisted_default_without_changing_existing_style() {
        let (mut canvas, _) = setup(DocumentKind::Whiteboard);
        let original = canvas.default_listing();
        let text = r"\lstset{numbers=right,frame=leftline}".to_owned();
        let style = listing::parse(&text, Style::default(), listing::preset)
            .unwrap()
            .1;
        canvas
            .latex
            .listing_styles
            .insert("Paper".into(), style.clone());
        canvas
            .latex
            .listing_configurations
            .insert("Paper".into(), text.clone());
        canvas.latex.listing_default = "Paper".into();
        canvas.latex =
            serde_json::from_str(&serde_json::to_string(&canvas.latex).unwrap()).unwrap();
        assert!(canvas.latex.validate().is_ok());
        let new = canvas.default_listing();
        assert_eq!(new.style_name, "Paper");
        assert_eq!(new.fallback, style);
        assert_eq!(new.configuration, text);
        assert_eq!(original.style_name, listing::LIGHT);
        assert_eq!(original.fallback.numbers, Numbers::Left);
    }
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn file_refresh_remeasures_without_advancing_selection_or_history() {
        use crate::ui::settings::tests::render;
        let ctx = egui::Context::default();
        crate::text_layout::install(&ctx);
        let (mut canvas, surface) = setup(DocumentKind::Whiteboard);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("source.py");
        let path_text = path.to_str().unwrap();
        let original = BinaryAsset::new((&b"original\r\n"[..]).into()).unwrap();
        let mut file = Attachment::new(
            canvas.drawing.owner(),
            surface,
            [100., 150.],
            "source.py".into(),
            original,
        );
        let model = listing::Listing {
            source_path: Some(path_text.into()),
            ..canvas.default_listing()
        };
        file.listing = Some(model);
        let id = file.id;
        let placement = file.placement;
        assert!(canvas.drawing.insert_snapshots(vec![file.into()]));
        canvas
            .selection
            .change([id], crate::selection::Membership::Replace, &canvas.drawing);
        render(&ctx, vec![], |ui| canvas.measure_listings(ui));
        let initial = canvas.attachment(id).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        canvas.listing_watch.next = 10.;
        canvas.listing_watch.pending = Some(rx);
        let bytes = b"updated\r\nwith\r\nmore lines\r\n";
        std::fs::write(&path, bytes).unwrap();
        tx.send(vec![(
            path_text.into(),
            crate::attachment_io::poll_link(path_text, None),
        )])
        .unwrap();
        render(&ctx, vec![], |ui| {
            canvas.poll_listing_sources(ui);
            canvas.measure_listings(ui);
        });
        let refreshed = canvas.attachment(id).unwrap();
        assert_eq!(refreshed.asset.bytes(), bytes);
        assert_eq!(refreshed.placement, placement);
        assert_eq!(refreshed.listing, initial.listing);
        assert!(refreshed.dimensions[1] > initial.dimensions[1]);
        assert!(canvas.selection.ids.contains(&id));
        canvas.undo();
        assert!(canvas.attachment(id).is_none());
    }
}
